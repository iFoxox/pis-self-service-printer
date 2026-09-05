//! 视图层：根 Render 实现 + 壳层（渐变背景 / 顶栏时钟 / 底栏长按 Logo）/ 弹窗
//!
//! 各页面 render 方法分文件实现（home / search / reports / settings / admin）。

pub(crate) mod admin;
pub(crate) mod logs;
mod home;
mod reports;
mod search;
mod settings;

use std::sync::Arc;

use chrono::Datelike;
use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, Context, FontWeight, Image, ImageFormat, ImageSource, KeyDownEvent,
    MouseButton, ObjectFit, Render, Window, div, px,
};

use crate::state::{AdminMatch, KioskState};
use crate::theme::{self, c, s, ts};
use crate::widgets;
use gpui_component::WindowExt as _;
use gpui_component::button::ButtonVariants as _;

/// 内置默认院徽（resources/assets 同款资源）
const DEFAULT_HOSPITAL_LOGO: &[u8] = include_bytes!("../../resources/assets/hospital-logo.png");
/// 底栏运营方 Logo（内置默认图，可经配置 footerLogo 替换）
const OPERATOR_LOGO: &[u8] = include_bytes!("../../resources/assets/operator-logo.png");

const WEEKDAYS: [&str; 7] = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

impl Render for KioskState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 按当前窗口视口更新自适应缩放系数（所有尺寸经 theme::s 换算），
        // 并同步 rem 基准（抵消 gpui 0.2.2 Windows 文字渲染的 DPI 放大）
        theme::update_ui_scale(window);
        window.set_rem_size(theme::rem_base());
        // gpui 0.2.2 Windows 布局视口是物理像素，渲染又乘 DPI——
        // 根容器显式使用"可见逻辑视口"尺寸，把内容约束在可见区域内
        let (view_w, view_h) = theme::viewport_logical();


        // 键盘输入依赖焦点：非设置页强制保证根焦点存在（设置页焦点归各输入框所有）
        if self.page != crate::state::Page::Settings && !self.focus_handle.is_focused(window) {
            window.focus(&self.focus_handle);
        }

        // 错误 / 日志 / 管理员弹窗已改为 gpui-component Dialog（经窗口句柄
        // 打开，由 Root 的 Dialog 层渲染），内容区始终是当前页面
        let content: gpui::AnyElement = match self.page {
            crate::state::Page::Home => self.render_home(cx),
            crate::state::Page::Search => self.render_search(cx),
            crate::state::Page::Reports => self.render_reports(cx),
            crate::state::Page::Settings => self.render_settings(cx),
        };

        // 注：曾用 with_animation 包装做页面淡入，但 gpui 0.2.2 Windows 下
        // 容器 opacity 动画与半透明内容叠加会渲染出大片黑块，已移除。
        let content = content;

        div()
            .id("root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event, window, cx| {
                    if this.page != crate::state::Page::Settings {
                        // 点击任意处重新接管根焦点（键盘输入依赖焦点）
                        window.focus(&this.focus_handle);
                    }
                    // 设置页焦点由 gpui-component 的 Input 自行管理
                    cx.notify();
                }),
            )
            .w(gpui::px(view_w))
            .h(gpui::px(view_h))
            .flex()
            .flex_col()
            .font_family("Microsoft YaHei")
            .text_color(c(theme::INK))
            // 渐变底 + 两枚氛围圆（对应 .kiosk-shell / .ambient-*）
            .bg(gpui::linear_gradient(
                145.,
                gpui::linear_color_stop(c(0xFBFDFF), 0.),
                gpui::linear_color_stop(c(0xE9F3FF), 1.),
            ))
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .top(s(-320.))
                    .right(s(190.))
                    .size(s(520.))
                    .rounded_full()
                    .bg(gpui::rgba(0x409EFF14)),
            )
            .child(
                div()
                    .absolute()
                    .bottom(s(-260.))
                    .left(s(-90.))
                    .size(s(360.))
                    .rounded_full()
                    .bg(gpui::rgba(0xF4A34C14)),
            )
            .child(self.render_header(cx))
            .child(content)
            .child(self.render_footer(cx))
            // gpui-component Dialog/通知渲染层（错误 / 日志 / 管理员弹窗）
            .children(gpui_component::Root::render_dialog_layer(window, cx))
    }
}

impl KioskState {
    /// 顶栏：左侧院徽（配置 logo > 内置默认），右侧实时时钟
    pub fn render_header(&mut self, _cx: &mut Context<Self>) -> impl IntoElement {
        let cfg = self.cfg();
        let now = self.now;

        let logo_source: ImageSource = {
            let preset = crate::domain::logo::find(
                crate::domain::logo::HOSPITAL_PRESETS,
                &cfg.hospital_logo_preset,
            );
            match preset {
                Some(p) => ImageSource::Image(Arc::new(Image::from_bytes(
                    ImageFormat::Png,
                    p.bytes.to_vec(),
                ))),
                None => match paths_join_logo(&cfg.hospital_logo) {
                    Some(path) => ImageSource::Resource(gpui::Resource::Path(path.into())),
                    None => ImageSource::Image(Arc::new(Image::from_bytes(
                        ImageFormat::Png,
                        DEFAULT_HOSPITAL_LOGO.to_vec(),
                    ))),
                },
            }
        };

        let time = now.format("%H:%M:%S").to_string();
        let date = format!(
            "{} {}",
            now.format("%Y/%m/%d"),
            WEEKDAYS[now.weekday().num_days_from_monday() as usize]
        );

        div()
            .flex()
            .items_center()
            .justify_between()
            .h(s(88.))
            .px(s(64.))
            .border_b_1()
            .border_color(gpui::rgba(0xB0CAE88C))
            .bg(gpui::rgba(0xFBFDFFF0))
            .child(
                div().h(s(64.)).flex().items_center().child(
                    gpui::img(logo_source)
                        .w(s(320.))
                        .h(s(64.))
                        .object_fit(ObjectFit::Contain),
                ),
            )
            .child(
                div()
                    .min_w(s(190.))
                    .flex()
                    .flex_col()
                    .items_end()
                    .child(
                        div()
                            .text_size(ts(34.))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(c(theme::HEADER_TEXT))
                            .child(time),
                    )
                    .child(
                        div()
                            .mt_1()
                            .text_size(ts(16.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(theme::CLOCK_DATE))
                            .child(date),
                    ),
            )
    }

    /// 底栏：运营方 Logo（可配置，默认内置图）长按 2.5 秒打开管理员验证（按住期间黄色进度条）
    pub fn render_footer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let holding = self.logo_holding;
        let cfg = self.cfg();

        // 底部 Logo：内置预设 > 自定义文件 > 内置占位图
        let logo_source: ImageSource = {
            let preset = crate::domain::logo::find(
                crate::domain::logo::OPERATOR_PRESETS,
                &cfg.footer_logo_preset,
            );
            match preset {
                Some(p) => ImageSource::Image(Arc::new(Image::from_bytes(
                    ImageFormat::Png,
                    p.bytes.to_vec(),
                ))),
                None => {
                    let name = cfg.footer_logo.trim();
                    let custom = (!name.is_empty())
                        .then(|| crate::paths::app_data_dir().join("logo").join(name))
                        .filter(|p| p.exists());
                    match custom {
                        Some(path) => ImageSource::Resource(gpui::Resource::Path(path.into())),
                        None => ImageSource::Image(Arc::new(Image::from_bytes(
                            ImageFormat::Png,
                            OPERATOR_LOGO.to_vec(),
                        ))),
                    }
                }
            }
        };

        let mut logo = div()
            .id("hy-kang-logo")
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .px(s(10.))
            .py(s(4.))
            .child(
                gpui::img(logo_source)
                    .h(s(40.))
                    .object_fit(ObjectFit::Contain),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _event: &gpui::MouseDownEvent, _window, cx| {
                    this.logo_holding = true;
                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(2500))
                            .await;
                        let _ = this.update(cx, |state, cx| {
                            if state.logo_holding {
                                state.open_admin(cx);
                                cx.notify();
                            }
                            state.logo_holding = false;
                        });
                    })
                    .detach();
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _event: &gpui::MouseUpEvent, _window, cx| {
                    this.logo_holding = false;
                    cx.notify();
                }),
            )
            .on_hover(cx.listener(|this, hovered: &bool, _window, cx| {
                // 按住期间移出按钮区域取消（对应 pointerleave）
                if !*hovered && this.logo_holding {
                    this.logo_holding = false;
                    cx.notify();
                }
            }));

        if holding {
            logo = logo.child(
                div()
                    .absolute()
                    .left(s(5.))
                    .right(s(5.))
                    .bottom(s(1.))
                    .h(s(2.))
                    .overflow_hidden()
                    .child(
                        div()
                            .h_full()
                            .rounded(s(2.))
                            .bg(c(0xFFCA28))
                            .with_animation(
                                "quit-hold",
                                Animation::new(std::time::Duration::from_millis(2500)),
                                |el, delta| el.w(gpui::relative(delta)),
                            ),
                    ),
            );
        }

        div()
            .flex()
            .items_center()
            .justify_center()
            .h(s(50.))
            .child(logo)
    }

        /// 全局键盘处理：快捷键 / 扫码枪与物理键盘输入
    pub fn handle_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ks = &event.keystroke;
        let key = ks.key.clone();
        let ctrl_alt = (ks.modifiers.control || ks.modifiers.platform) && ks.modifiers.alt;

        if ctrl_alt && key == "s" {
            self.open_settings(window, cx);
            cx.notify();
            return;
        }
        if ctrl_alt && key == "q" {
            crate::domain::log::info("main", "Ctrl+Alt+Q 退出应用");
            cx.quit();
            return;
        }
        if ctrl_alt && key == "f" {
            // 走 Win32 无边框全屏（gpui 真全屏在高 DPI 下渲染错乱）
            crate::native_window::set_borderless_fullscreen();
            return;
        }

        // 调试构建专用：快捷键直达各页面，便于布局审查
        if cfg!(debug_assertions) && ctrl_alt {
            if key == "l" {
                self.open_logs(cx);
                cx.notify();
                return;
            }
            let target = match key.as_str() {
                "1" => Some(crate::state::Page::Home),
                "2" => Some(crate::state::Page::Search),
                "3" => Some(crate::state::Page::Reports),
                "4" => Some(crate::state::Page::Settings),
                _ => None,
            };
            if let Some(page) = target {
                if page == crate::state::Page::Settings {
                    self.open_settings(window, cx);
                } else {
                    self.page = page;
                }
                cx.notify();
                return;
            }
        }

        if self.admin_open {
            match key.as_str() {
                "escape" => {
                    self.close_admin();
                    window.close_dialog(cx);
                }
                "backspace" => {
                    self.admin_password.pop();
                }
                "enter" => {
                    if let Some(matched) = self.confirm_admin() {
                        // 密码匹配：先关框架 Dialog，再执行密码对应的动作
                        window.close_dialog(cx);
                        match matched {
                            AdminMatch::Minimize => window.minimize_window(),
                            AdminMatch::Settings => self.open_settings(window, cx),
                            AdminMatch::Logs => self.open_logs(cx),
                        }
                    }
                }
                _ => {
                    if let Some(ch) = &ks.key_char {
                        let ch = ch.to_string();
                        if ch.chars().all(|c| c.is_ascii_digit()) && self.admin_password.len() < 4 {
                            self.admin_password.push_str(&ch);
                        }
                    }
                }
            }
            cx.notify();
            return;
        }

        // 设置页按键由 gpui-component 的 Input 自行处理（各自持有焦点与光标），
        // 根级只保留快捷键 / 管理员弹窗 / 查询页扫码枪输入路由。

        if self.page == crate::state::Page::Search {
            let is_modifier_combo = ks.modifiers.control || ks.modifiers.platform;
            if is_modifier_combo {
                match key.as_str() {
                    "a" => {
                        self.select_all_keyword();
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    "c" => {
                        self.copy_keyword(cx);
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    "x" => {
                        self.cut_keyword(cx);
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    "v" => {
                        self.paste_keyword(cx);
                        cx.stop_propagation();
                        cx.notify();
                        return;
                    }
                    _ => {}
                }
            }
            match key.as_str() {
                "enter" => {
                    self.submit_query(cx);
                    cx.notify();
                    return;
                }
                "backspace" => {
                    if self.keyword_selected {
                        self.keyword.clear();
                        self.keyword_selected = false;
                    } else {
                        self.keyword.pop();
                    }
                    cx.notify();
                    return;
                }
                _ => {
                    if let Some(ch) = &ks.key_char {
                        let upper = ch.to_ascii_uppercase();
                        let ok = upper
                            .chars()
                            .next()
                            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-');
                        if ok {
                            if self.keyword_selected {
                                self.keyword = upper;
                                self.keyword_selected = false;
                            } else if self.keyword.len() < crate::state::MAX_KEYWORD {
                                self.keyword.push_str(&upper);
                            }
                            cx.notify();
                            return;
                        }
                    }
                }
            }
        }

        cx.notify();
    }
}

/// 按估算内容高度计算 Dialog 垂直居中的 margin_top（框架默认只留 1/10 视口）
/// est_height 是设计稿像素，须经 s() 换算为实际布局像素再与视口比较
pub(crate) fn dialog_center_margin(window: &Window, est_height: f32) -> gpui::Pixels {
    let vh = f32::from(window.viewport_size().height);
    px(((vh - f32::from(s(est_height))) / 2.).max(24.))
}

/// 在窗口上打开错误 Dialog（由 KioskState::show_error 调用）
pub(crate) fn open_error_dialog(
    window: &mut Window,
    cx: &mut gpui::App,
    kiosk: &gpui::WeakEntity<KioskState>,
    _message: String,
) {
    // 内容为实时视图：倒计时数字随 KioskState 每秒通知跳动
    let view = cx.new(|cx| ErrorDialogView::new(kiosk.clone(), cx));
    let kiosk_weak = kiosk.clone();
    let margin_top = dialog_center_margin(window, 460.);
    window.open_dialog(cx, {
        let view = view.clone();
        let margin_top = margin_top;
        move |dialog, _window, _cx| {
            let kiosk_weak = kiosk_weak.clone();
            dialog
                .title("出错了")
                .w(s(720.))
                .margin_top(margin_top)
                // 关闭只经「知道了」按钮与 10 秒倒计时，状态单一来源
                .overlay_closable(false)
                .close_button(false)
                .keyboard(false)
                .on_close(move |_, _, cx| {
                    if let Some(kiosk) = kiosk_weak.upgrade() {
                        kiosk.update(cx, |state, _| state.error = None);
                    }
                })
                .child(view.clone())
        }
    });
}

/// 错误弹窗内容视图：倒计时随 KioskState 每秒通知实时刷新
struct ErrorDialogView {
    kiosk: gpui::WeakEntity<KioskState>,
}

impl ErrorDialogView {
    fn new(kiosk: gpui::WeakEntity<KioskState>, cx: &mut Context<Self>) -> Self {
        if let Some(entity) = kiosk.upgrade() {
            cx.observe(&entity, |_, _, cx| cx.notify()).detach();
        }
        Self { kiosk }
    }
}

impl Render for ErrorDialogView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(kiosk) = self.kiosk.upgrade() else {
            return div().into_any_element();
        };
        let state = kiosk.read(cx);
        let message = state.error.clone().unwrap_or_default();
        let countdown = state.error_countdown;
        let kiosk = kiosk.clone();

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            // 统一弹窗尺寸：内容高 460 + 标题栏 ≈ 自绘弹窗 520
            .h(s(460.))
            .gap(s(22.))
            .py(s(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(s(96.))
                    .rounded_full()
                    .bg(gpui::linear_gradient(
                        135.,
                        gpui::linear_color_stop(c(0xF0886E), 0.),
                        gpui::linear_color_stop(c(theme::DANGER_DARK), 1.),
                    ))
                    .shadow_md()
                    .text_color(c(0xFFFFFF))
                    .text_size(ts(46.))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .child("!"),
            )
            .child(
                div()
                    .px_2()
                    .text_center()
                    .text_size(ts(30.))
                    .line_height(s(46.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(theme::HEADER_TEXT))
                    .child(message),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(s(6.))
                    // 数字与文字同字号，保证对齐
                    .text_size(ts(20.))
                    .line_height(s(28.))
                    .child(
                        div()
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(c(theme::TEAL))
                            .child(countdown.to_string()),
                    )
                    .child(
                        div()
                            .text_color(c(theme::MUTED))
                            .child("秒后自动关闭"),
                    ),
            )
            .child(
                gpui_component::button::Button::new("error-ok")
                    .primary()
                    .min_w(s(240.))
                    .min_h(s(56.))
                    .child(widgets::kiosk_label("知道了", 26.))
                    .on_click(move |_event, window, cx| {
                        kiosk.update(cx, |state, cx| {
                            state.error = None;
                            state.reset_countdown();
                            cx.notify();
                        });
                        window.close_dialog(cx);
                    }),
            )
            .into_any_element()
    }
}

/// 解析自定义院徽路径：配置非空且文件存在时返回 Some
fn paths_join_logo(name: &str) -> Option<std::path::PathBuf> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = crate::paths::app_data_dir().join("logo").join(trimmed);
    path.exists().then_some(path)
}
