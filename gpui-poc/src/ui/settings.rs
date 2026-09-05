//! 设置页（对应 SettingsDialog.vue：完整配置表单，分区双列布局）
//!
//! gpui-0.2：控件全部换用 GPUI Component——
//! 文本字段 = Input（真实光标/选择/剪贴板），开关 = Switch，
//! 滑动条 = Slider，页面滚动 = overflow_y_scrollbar（自带可拖动滚动条）。

use gpui::prelude::*;
use gpui::{
    Context, FontWeight, Image, ImageFormat, ImageSource, IntoElement, ObjectFit, ParentElement,
    Styled, div,
};
use gpui_component::button::Button;
use gpui_component::select::Select;
use gpui_component::slider::Slider;
use gpui_component::switch::Switch;
use gpui_component::{Disableable as _, Sizable as _, button::ButtonVariants as _, input::Input, scroll::ScrollableElement as _};

use crate::state::KioskState;
use crate::theme::{self, c, s, ts};
use crate::widgets;

const VOICE_EXT: &[&str] = &["mp3", "wav", "ogg", "m4a", "aac"];
const LOGO_EXT: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp"];

impl KioskState {
    /// 分区标题（对应 .settings-section-title）
    fn section_title(title: &str, subtitle: &str) -> gpui::AnyElement {
        div()
            .flex()
            .items_baseline()
            .gap_3()
            .mt(s(10.))
            .mb(s(16.))
            .pl(s(15.))
            .py(s(12.))
            .border_l_4()
            .border_color(c(theme::TEAL))
            .rounded(s(10.))
            .bg(c(0xF2F8FF))
            .child(
                div()
                    .text_size(ts(16.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(theme::INK))
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(c(theme::MUTED))
                    .child(subtitle.to_string()),
            )
            .into_any_element()
    }

    /// 表单标签
    fn form_label(label: &str) -> gpui::AnyElement {
        div()
            .mb(s(6.))
            .text_sm()
            .font_weight(FontWeight::BOLD)
            .text_color(c(theme::CLOCK_DATE))
            .child(label.to_string())
            .into_any_element()
    }

    fn field_label(key: &str) -> &'static str {
        match key {
            "hospital_name" => "终端显示名称",
            "terminal_code" => "终端编号",
            "report_notice" => "患者报告查询提示",
            "input_hint" => "查询输入提示",
            "base_url" => "PIS 接口地址",
            "org_id" => "机构 ID",
            "api_key" => "API Key",
            "secret_key" => "Secret Key",
            "exit_password" => "设置密码（选填）",
            "minimize_password" => "最小化密码（选填）",
            "log_password" => "查看日志密码（选填）",
            _ => "字段",
        }
    }

    /// 文本输入框：绑定框架 Input 实体（键入 / 光标 / 粘贴由框架接管，
    /// 变更经订阅实时写回 draft.config，见 state.rs open_settings）
    fn text_field(&self, key: &'static str) -> gpui::AnyElement {
        match self.settings_inputs.get(key) {
            Some(state) => div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(Self::form_label(Self::field_label(key)))
                .child(
                    Input::new(state)
                        .cleanable(false)
                        .h(s(64.))
                        .w_full()
                        .text_size(s(20.)),
                )
                .into_any_element(),
            None => div().into_any_element(),
        }
    }

    /// 跨两列的文本输入框
    fn text_field_wide(&self, key: &'static str) -> gpui::AnyElement {
        div()
            .col_span(2)
            .flex()
            .flex_col()
            .child(self.text_field(key))
            .into_any_element()
    }

    /// 开关行：标签 + 框架 Switch（checked 回调带新值，直接写配置）
    fn toggle_row(
        &self,
        cx: &Context<Self>,
        id: &'static str,
        label: &'static str,
        checked: bool,
        on_toggle: impl Fn(&mut Self, bool) + 'static,
    ) -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .child(Self::form_label(label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        Switch::new(id).checked(checked).on_click(cx.listener(
                            move |this, checked: &bool, _window, cx| {
                                this.play_click(cx);
                                on_toggle(this, *checked);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        div()
                            .ml(s(10.))
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(if checked {
                                theme::TEAL_DARK
                            } else {
                                theme::MUTED
                            }))
                            .child(if checked { "开启" } else { "关闭" }.to_string()),
                    ),
            )
            .into_any_element()
    }

    /// 滑动条设置行（label + 当前值右对齐；拖动经 SliderEvent 实时写回草稿）
    fn slider_row(
        &self,
        key: &'static str,
        label: &'static str,
        value_text: String,
    ) -> gpui::AnyElement {
        match self.settings_sliders.get(key) {
            Some(state) => div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(Self::form_label(label))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .text_color(c(theme::TEAL_DARK))
                                .child(value_text),
                        ),
                )
                .child(Slider::new(state).w_full())
                .into_any_element(),
            None => div().into_any_element(),
        }
    }

    pub fn render_settings(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let draft_cfg = self.draft.config.clone();
        let terminal = &draft_cfg.terminal;

        // ==== 终端信息 ====
        let info_grid = div()
            .grid()
            .grid_cols(2)
            .gap_x(s(24.))
            .gap_y(s(14.))
            .child(self.text_field("hospital_name"))
            .child(self.text_field("terminal_code"))
            .child(self.render_logo_row(
                cx,
                "hospital_logo",
                &draft_cfg.hospital_logo,
                &draft_cfg.hospital_logo_preset,
            ))
            .child(self.render_logo_row(
                cx,
                "footer_logo",
                &draft_cfg.footer_logo,
                &draft_cfg.footer_logo_preset,
            ))
            .child(self.text_field_wide("report_notice"))
            .child(self.text_field_wide("input_hint"));

        // ==== PIS 接口参数 ====
        let pis_grid = div()
            .grid()
            .grid_cols(2)
            .gap_x(s(24.))
            .gap_y(s(14.))
            .child(self.text_field_wide("base_url"))
            .child(self.text_field("org_id"))
            .child(self.slider_row(
                "request_timeout",
                "请求超时（秒）",
                format!("{} s", draft_cfg.service.request_timeout_seconds),
            ))
            .child(self.text_field("api_key"))
            .child(self.text_field("secret_key"));

        // ==== 打印与选择 ====
        let print_grid = div()
            .grid()
            .grid_cols(2)
            .gap_x(s(24.))
            .gap_y(s(14.))
            .child(self.render_printer_row(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(Self::form_label("纸张"))
                    .child(widgets::segmented(
                        &["A4", "A5"],
                        if draft_cfg.print.paper == "A5" { 1 } else { 0 },
                        cx.listener(|this, index: &usize, _w, cx| {
                            this.play_click(cx);
                            this.draft.config.print.paper = if *index == 1 {
                                "A5".into()
                            } else {
                                "A4".into()
                            };
                            cx.notify();
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(Self::form_label("纸张方向"))
                    .child(widgets::segmented(
                        &["纵向", "横向"],
                        if draft_cfg.print.orientation == "landscape" {
                            1
                        } else {
                            0
                        },
                        cx.listener(|this, index: &usize, _w, cx| {
                            this.play_click(cx);
                            this.draft.config.print.orientation = if *index == 1 {
                                "landscape".into()
                            } else {
                                "portrait".into()
                            };
                            cx.notify();
                        }),
                    )),
            )
            .child(self.toggle_row(
                cx,
                "allow_reprint",
                "允许已打印报告再次打印",
                draft_cfg.print.allow_reprint,
                |this, v| this.draft.config.print.allow_reprint = v,
            ))
            .child(self.toggle_row(
                cx,
                "auto_select_reports",
                "查询后默认勾选可打印报告",
                draft_cfg.terminal.auto_select_reports,
                |this, v| this.draft.config.terminal.auto_select_reports = v,
            ));

        // ==== 声音 ====
        let audio_grid = div()
            .grid()
            .grid_cols(2)
            .gap_x(s(24.))
            .gap_y(s(14.))
            .child(self.toggle_row(
                cx,
                "voice_enabled",
                "患者语音提示",
                draft_cfg.terminal.voice_enabled,
                |this, v| this.draft.config.terminal.voice_enabled = v,
            ))
            .child(self.slider_row(
                "voice_volume",
                "语音音量",
                format!("{}%", draft_cfg.terminal.voice_volume),
            ))
            .child(self.slider_row(
                "voice_rate",
                "语音速度",
                format!("{:.1}×", draft_cfg.terminal.voice_rate),
            ))
            .child(self.toggle_row(
                cx,
                "click_enabled",
                "按键音效",
                draft_cfg.terminal.click_enabled,
                |this, v| this.draft.config.terminal.click_enabled = v,
            ))
            .child(self.slider_row(
                "click_volume",
                "按键音效音量",
                format!("{}%", draft_cfg.terminal.click_volume),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .justify_end()
                    .child(
                        Button::new("preview-click")
                            .small()
                            .outline()
                            .label("试听按键音")
                            .on_click(cx.listener(|this, _event, _window, _cx| {
                                let vol = this.draft.config.terminal.click_volume;
                                crate::audio::play_click(vol);
                            })),
                    ),
            );

        // ==== 语音文件 ====
        let voice_grid = div()
            .col_span(2)
            .flex()
            .flex_col()
            .gap_3()
            .child(self.render_voice_row(cx, "输入提示音", "voice_input", &terminal.voice_input))
            .child(self.render_voice_row(
                cx,
                "选择报告提示音",
                "voice_reports_found",
                &terminal.voice_reports_found,
            ))
            .child(self.render_voice_row(
                cx,
                "取报告提示音",
                "voice_print_complete",
                &terminal.voice_print_complete,
            ));

        // ==== 运行与安全 ====
        let security_grid = div()
            .grid()
            .grid_cols(2)
            .gap_x(s(24.))
            .gap_y(s(14.))
            .child(self.toggle_row(
                cx,
                "fullscreen",
                "启动后自动全屏（下次启动生效）",
                draft_cfg.terminal.fullscreen,
                |this, v| this.draft.config.terminal.fullscreen = v,
            ))
            .child(self.slider_row(
                "idle_timeout",
                "无操作自动返回（秒）",
                format!("{} s", draft_cfg.terminal.idle_timeout_seconds),
            ))
            .child(self.slider_row(
                "log_retention",
                "日志保留天数（最长 30 天）",
                format!("{} 天", draft_cfg.terminal.log_retention_days),
            ))
            .child(self.render_log_dir_row(cx, &draft_cfg.terminal.log_dir))
            .child(self.render_backup_row(cx))
            .child(self.text_field("exit_password"))
            .child(self.text_field("minimize_password"))
            .child(self.text_field("log_password"));

        // ==== 页面骨架 ====
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .px(s(64.))
            .py(s(20.))
            // 保存成功提示（2 秒自动消失）
            .children(self.save_notice.as_ref().map(|_msg| {
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .mb(s(12.))
                    .w_full()
                    .h(s(40.))
                    .rounded(s(12.))
                    .border_1()
                    .border_color(c(0xB5E3C4))
                    .bg(c(0xEAF7EE))
                    .text_size(ts(15.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(0x2E7D4F))
                    .child("✓ 配置已保存")
            }))
            .child(
                widgets::card()
                    .flex_1()
                    .min_h_0()
                    .rounded(s(24.))
                    .overflow_hidden()
                    .shadow_md()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(s(28.))
                            .py(s(16.))
                            .border_b_1()
                            .border_color(c(theme::LINE))
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .text_color(c(theme::HEADER_TEXT))
                                    .child("终端设置"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(c(theme::MUTED))
                                    .child("点击文本框后用键盘输入，滚动条可拖动"),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-body")
                            .flex_1()
                            .min_h_0()
                            // 框架滚动条：滚轮 + 可拖动拇指，常显（kiosk 触屏）
                            .overflow_y_scrollbar()
                            .px(s(28.))
                            .py(s(12.))
                            .child(Self::section_title("终端信息", "用于页面展示和设备识别"))
                            .child(info_grid)
                            .child(Self::section_title(
                                "PIS 接口参数",
                                "以下参数由病理全流程系统管理员提供",
                            ))
                            .child(pis_grid)
                            .child(Self::section_title(
                                "打印与选择",
                                "设置打印机、纸张和报告默认选择方式",
                            ))
                            .child(print_grid)
                            .child(Self::section_title("声音", "语音与按键音效设置"))
                            .child(audio_grid)
                            .child(Self::section_title(
                                "语音文件",
                                "三段提示音可替换为自定义音频，不配置则使用内置语音",
                            ))
                            .child(voice_grid)
                            .child(Self::section_title(
                                "运行与安全",
                                "全屏设置下次启动生效，三个密码不能相同",
                            ))
                            .child(security_grid),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(s(28.))
                            .py(s(14.))
                            .border_t_1()
                            .border_color(c(theme::LINE))
                            .child(
                                div()
                                    .flex()
                                    .gap_3()
                                    .child(
                                        Button::new("open-logs")
                                            .outline()
                                            .min_w(s(150.))
                                            .h(s(52.))
                                            .label("查看日志")
                                            .on_click(cx.listener(|this, _event, _window, cx| {
                                                this.play_click(cx);
                                                this.open_logs(cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("quit-app")
                                            .danger()
                                            .min_w(s(150.))
                                            .h(s(52.))
                                            .label("关闭软件")
                                            .on_click(|_event, _window, cx| {
                                                crate::domain::log::info("main", "设置页关闭软件");
                                                cx.quit();
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        Button::new("cancel-settings")
                                            .outline()
                                            .min_w(s(150.))
                                            .h(s(52.))
                                            .label("取消")
                                            .on_click(cx.listener(|this, _event, _window, cx| {
                                                this.play_click(cx);
                                                this.go_home();
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("save-settings")
                                            .primary()
                                            .min_w(s(150.))
                                            .h(s(52.))
                                            .label("保存配置")
                                            .on_click(cx.listener(|this, _event, _window, cx| {
                                                this.play_click(cx);
                                                this.save_draft(cx);
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// Logo 行（医院 Logo / 底部运营方 Logo 共用）：
    /// 预览图 + 来源状态 + 内置预设下拉 + 外部文件选择
    fn render_logo_row(
        &mut self,
        cx: &mut Context<Self>,
        key: &'static str,
        name: &str,
        preset_id: &str,
    ) -> gpui::AnyElement {
        let is_hospital = key == "hospital_logo";
        let (label, presets, select) = if is_hospital {
            (
                "左上角医院 Logo",
                crate::domain::logo::HOSPITAL_PRESETS,
                self.hospital_logo_select.clone(),
            )
        } else {
            (
                "底部运营方 Logo",
                crate::domain::logo::OPERATOR_PRESETS,
                self.footer_logo_select.clone(),
            )
        };
        // 来源状态与预览图源：内置预设 > 自定义文件 > 占位图
        let (status, logo_source) = if let Some(p) = crate::domain::logo::find(presets, preset_id) {
            (
                format!("内置：{}", p.label),
                ImageSource::Image(std::sync::Arc::new(Image::from_bytes(
                    ImageFormat::Png,
                    p.bytes.to_vec(),
                ))),
            )
        } else {
            let custom_path = crate::paths::app_data_dir().join("logo").join(name);
            let custom = (!name.trim().is_empty() && custom_path.exists()).then(|| {
                ImageSource::Resource(gpui::Resource::Path(custom_path.into()))
            });
            match custom {
                Some(source) => (format!("自定义：{name}"), source),
                None => (
                    "使用内置占位图".to_string(),
                    ImageSource::Image(std::sync::Arc::new(Image::from_bytes(
                        ImageFormat::Png,
                        if is_hospital {
                            include_bytes!("../../resources/assets/hospital-logo.png").to_vec()
                        } else {
                            include_bytes!("../../resources/assets/operator-logo.png").to_vec()
                        },
                    ))),
                ),
            }
        };

        div()
            .col_span(2)
            .flex()
            .flex_col()
            .child(Self::form_label(label))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        gpui::img(logo_source)
                            .w(s(130.))
                            .h(s(42.))
                            .flex_none()
                            .rounded(s(6.))
                            .border_1()
                            .border_color(c(theme::LINE))
                            .bg(c(0xFFFFFF))
                            .object_fit(ObjectFit::Contain),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(c(theme::MUTED))
                            .child(status),
                    )
                    .child(match select {
                        Some(select) => Select::new(&select)
                            .w(s(240.))
                            .into_any_element(),
                        None => div().into_any_element(),
                    })
                    .child(
                        Button::new(gpui::ElementId::Name(format!("select-{key}").into()))
                            .small()
                            .outline()
                            .label("选择文件")
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.play_click(cx);
                                this.select_app_file(key, LOGO_EXT, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    /// 默认打印机行：框架 Select 下拉（不指定 = 走系统默认）+ 刷新 + 检测状态
    fn render_printer_row(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let count = self.printers.len();
        let loading = self.printers_loading;

        div()
            .col_span(2)
            .flex()
            .flex_col()
            .child(Self::form_label("默认打印机"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(match self.printer_select.clone() {
                        Some(select) => Select::new(&select)
                            .cleanable(true)
                            .placeholder("请选择报告打印机")
                            .w_full()
                            .into_any_element(),
                        None => div().into_any_element(),
                    })
                    .child(
                        Button::new("refresh-printers")
                            .small()
                            .outline()
                            .label(if loading { "刷新中..." } else { "刷新打印机" })
                            .disabled(loading)
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.play_click(cx);
                                this.refresh_printers(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .mt(s(5.))
                    .text_xs()
                    .text_color(c(theme::MUTED))
                    .child(if count > 0 {
                        format!("已检测到 {count} 台打印机")
                    } else {
                        "请确认打印机已连接并在系统中安装驱动".to_string()
                    }),
            )
            .into_any_element()
    }

    /// 语音文件行：文件名 + 选择 / 试听 / 恢复默认
    fn render_voice_row(
        &mut self,
        cx: &mut Context<Self>,
        label: &str,
        field: &'static str,
        value: &str,
    ) -> gpui::AnyElement {
        let voice_key = match field {
            "voice_input" => Some(crate::audio::VoiceKey::Input),
            "voice_reports_found" => Some(crate::audio::VoiceKey::ReportsFound),
            "voice_print_complete" => Some(crate::audio::VoiceKey::PrintComplete),
            _ => None,
        };
        div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .w(s(120.))
                    .flex_none()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(theme::CLOCK_DATE))
                    .child(label.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(c(theme::MUTED))
                    .child(if value.is_empty() {
                        "使用内置语音".to_string()
                    } else {
                        format!("自定义：{value}")
                    }),
            )
            .child(
                Button::new(gpui::ElementId::Name(format!("select-{field}").into()))
                    .small()
                    .outline()
                    .label("选择文件")
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.play_click(cx);
                        this.pending_voice_field = Some(field);
                        this.select_app_file("voice", VOICE_EXT, cx);
                    })),
            )
            .children(voice_key.map(|key| {
                Button::new(gpui::ElementId::Name(format!("preview-{field}").into()))
                    .small()
                    .outline()
                    .label("试听")
                    .on_click(cx.listener(move |this, _event, _window, _cx| {
                        let cfg = this.draft.config.clone();
                        crate::audio::speak(key, &cfg);
                    }))
            }))
            .children((!value.is_empty()).then(|| {
                Button::new(gpui::ElementId::Name(format!("reset-{field}").into()))
                    .small()
                    .outline()
                    .label("恢复默认")
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.play_click(cx);
                        let old = this.draft_text(field);
                        this.remove_app_file(&old, "voice");
                        this.clear_settings_field(field, window, cx);
                    }))
            }))
            .into_any_element()
    }

    /// 配置备份行：显示备份目录 + 打开按钮
    fn render_backup_row(&self, _cx: &Context<Self>) -> gpui::AnyElement {
        let dir = crate::paths::app_data_dir().join("config-backups");
        let dir_text = dir.display().to_string();
        let open_dir = dir.clone();
        div()
            .col_span(2)
            .flex()
            .flex_col()
            .child(Self::form_label("配置备份"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(c(theme::MUTED))
                            .child(format!(
                                "{}（配置变更时自动备份，最多保留 30 份）",
                                dir_text
                            )),
                    )
                    .child(
                        Button::new("open-backup")
                            .small()
                            .outline()
                            .label("打开备份目录")
                            .on_click(move |_event, _window, _cx| {
                                // 目录尚不存在时先创建（首次备份前点击也能打开）
                                let _ = std::fs::create_dir_all(&open_dir);
                                let _ = std::process::Command::new("explorer")
                                    .arg(&open_dir)
                                    .spawn();
                            }),
                    ),
            )
            .into_any_element()
    }

    /// 日志目录行
    fn render_log_dir_row(&mut self, cx: &mut Context<Self>, dir: &str) -> gpui::AnyElement {
        div()
            .col_span(2)
            .flex()
            .flex_col()
            .child(Self::form_label("日志输出目录"))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .text_color(c(theme::MUTED))
                            .child(if dir.is_empty() {
                                "默认：应用安装目录 logs 下".to_string()
                            } else {
                                dir.to_string()
                            }),
                    )
                    .child(
                        Button::new("select-log-dir")
                            .small()
                            .outline()
                            .label("选择目录")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.play_click(cx);
                                let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
                                    files: false,
                                    directories: true,
                                    multiple: false,
                                    prompt: None,
                                });
                                cx.spawn(async move |this, cx| {
                                    let picked = match receiver.await {
                                        Ok(Ok(Some(paths))) => Some(paths),
                                        _ => None,
                                    };
                                    let _ = this.update(cx, |state, cx| {
                                        if let Some(paths) = picked {
                                            if let Some(path) = paths.first() {
                                                state.draft.config.terminal.log_dir =
                                                    path.to_string_lossy().to_string();
                                            }
                                        }
                                        cx.notify();
                                    });
                                })
                                .detach();
                            })),
                    )
                    .children((!dir.is_empty()).then(|| {
                        Button::new("reset-log-dir")
                            .small()
                            .outline()
                            .label("恢复默认")
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.play_click(cx);
                                this.draft.config.terminal.log_dir.clear();
                                cx.notify();
                            }))
                    })),
            )
            .into_any_element()
    }
}
