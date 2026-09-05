//! 首页（对应 HomeView.vue：左文案 + 右操作卡 + 打印机插画）

use gpui::{
    Context, FontWeight, IntoElement, ParentElement, Styled, div, linear_color_stop,
    linear_gradient,
};

use crate::{icons, widgets};
use crate::state::{KioskState, Page};
use crate::theme::{self, c, s, ts};

impl KioskState {
    pub fn render_home(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let cfg = self.cfg();
        let notice = if cfg.terminal.report_notice.is_empty() {
            crate::domain::config::DEFAULT_REPORT_NOTICE.to_string()
        } else {
            cfg.terminal.report_notice.clone()
        };

        // ==== 左栏文案 ====
        let copy = div()
            .flex()
            .flex_col()
            .justify_between()
            .min_h(s(700.))
            .max_w(s(760.))
            // eyebrow：橙色短杠 + SELF-SERVICE TERMINAL
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .mb(s(26.))
                    .child(div().w(s(34.)).h(s(4.)).rounded(s(4.)).bg(c(theme::WARM)))
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(theme::TEAL))
                            .child("SELF-SERVICE TERMINAL"),
                    ),
            )
            // 大标题
            .child(
                div()
                    .text_size(ts(96.))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .line_height(gpui::relative(1.1))
                    .text_color(c(theme::TITLE))
                    .child("病理报告")
                    .child(div().text_color(c(theme::TEAL)).child("自助打印")),
            )
            // 操作流程（渐变标签牌 + 胶囊步骤条：渐变圆形序号徽章 + 图标 + 箭头连接）
            .child(
                div()
                    .mt(s(40.))
                    .flex()
                    .flex_col()
                    .items_start()
                    .child(
                        div().child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .px(s(20.))
                                .py(s(10.))
                                .rounded_tl(s(16.))
                                .rounded_tr(s(16.))
                                .rounded_br(s(16.))
                                .rounded_bl(s(4.))
                                .bg(linear_gradient(
                                    135.,
                                    linear_color_stop(c(theme::BUTTON_TOP), 0.),
                                    linear_color_stop(c(theme::BUTTON_BOTTOM), 1.),
                                ))
                                .shadow_sm()
                                .child(Self::flow_label_glyph())
                                .child(
                                    div()
                                        .text_size(ts(20.))
                                        .font_weight(FontWeight::EXTRA_BOLD)
                                        .text_color(c(0xFFFFFF))
                                        .child("操作流程"),
                                ),
                        ),
                    )
                    .child(
                        div()
                            .mt(s(16.))
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(Self::flow_step("1", "输入查询信息", icons::SEARCH))
                            .child(home_step_arrow())
                            .child(Self::flow_step("2", "选择报告", icons::DOCUMENT))
                            .child(home_step_arrow())
                            .child(Self::flow_step("3", "打印报告", icons::PRINTER)),
                    ),
            )
            // 报告时限提示（重要提醒：薄荷绿浅底 + 左侧粗色条 + 青绿图标，轻盈醒目）
            .child(
                div()
                    .mt(s(18.))
                    .flex()
                    .items_center()
                    .gap_3()
                    .pl(s(16.))
                    .pr(s(24.))
                    .py(s(16.))
                    .rounded(s(14.))
                    .border_1()
                    .border_color(c(0xA8DFD6))
                    .bg(linear_gradient(
                        135.,
                        linear_color_stop(c(0xF0FBF8), 0.),
                        linear_color_stop(c(0xDDF5EF), 1.),
                    ))
                    .shadow_sm()
                    .child(
                        div()
                            .flex_none()
                            .size(s(34.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            // 淡黄渐变 + 深琥珀字（浅底白字对比度不足）
                            .bg(linear_gradient(
                                135.,
                                linear_color_stop(c(0xFFE9A0), 0.),
                                linear_color_stop(c(0xFFD050), 1.),
                            ))
                            .shadow_sm()
                            .text_color(c(0x8A6100))
                            .text_size(ts(20.))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .child("!"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(ts(22.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(0x0E7566))
                            .child(notice),
                    ),
            );

        // ==== 右栏操作卡 ====
        let action_card = div()
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .flex_1()
            .max_w(s(780.))
            .min_h(s(700.))
            .px(s(48.))
            .py(s(44.))
            .border_2()
            .border_color(gpui::rgba(0xFFF2F2))
            .rounded_tl(s(42.))
            .rounded_tr(s(14.))
            .rounded_br(s(42.))
            .rounded_bl(s(42.))
            .bg(gpui::rgba(0xFFFFFFDB))
            .shadow_lg()
            // 内描边装饰（对应 ::before）
            .child(
                div()
                    .absolute()
                    .inset(s(18.))
                    .border_1()
                    .border_color(gpui::rgba(0x409EFF1A))
                    .rounded_tl(s(27.))
                    .rounded_tr(s(6.))
                    .rounded_br(s(27.))
                    .rounded_bl(s(27.)),
            )
            .child(self.render_printer_illustration())
            .child(
                div()
                    .mt(s(20.))
                    .text_size(ts(48.))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .text_color(c(theme::HEADER_TEXT))
                    .child("病理报告打印"),
            )
            .child(
                div()
                    .mt(s(14.))
                    .text_size(ts(22.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(theme::CLOCK_DATE))
                    .child("请根据页面提示输入查询信息并选择需要打印的报告"),
            )
            .child(
                div().mt(s(34.)).w_full().flex().justify_center().child(
                    widgets::fw_primary("start-print", "开始打印")
                        .child(icons::icon(icons::ARROW_RIGHT, 32., 0xFFFFFF))
                        .rounded_tl(s(22.))
                        .rounded_tr(s(22.))
                        .rounded_br(s(22.))
                        .rounded_bl(s(8.))
                        .min_h(s(88.))
                        .min_w(s(560.))
                        .text_size(ts(32.))
                        .shadow_lg()
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.play_click(cx);
                            this.keyword.clear();
                            this.reports.clear();
                            this.selected.clear();
                            this.page = Page::Search;
                            this.reset_countdown();
                            let cfg = this.cfg();
                            crate::audio::speak(crate::audio::VoiceKey::Input, &cfg);
                            cx.notify();
                        })),
                ),
            )
            .child(
                div()
                    .mt(s(18.))
                    .text_base()
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(theme::SUB_TEXT))
                    .child("请在打印完成后及时取走并妥善保管报告"),
            );

        div()
            .flex_1()
            .flex()
            .justify_between()
            .gap(s(72.))
            .px(s(88.))
            .py(s(32.))
            // 左右两栏均等高且垂直居中：顶部对顶部、底部对底部，视觉平行
            .child(div().flex().items_center().child(copy))
            .child(div().flex().items_center().child(action_card))
            .into_any_element()
    }

    /// 打印机插画（分层细节：报告纸张 + 高光机身 + 指示灯 + 出纸 + 底部投影）
    fn render_printer_illustration(&self) -> gpui::AnyElement {
        div()
            .relative()
            .w(s(290.))
            .h(s(302.))
            // 上方纸张（报告样式：角标 + 标题行 + 正文行）
            .child(
                div()
                    .absolute()
                    .left(s(62.))
                    .right(s(62.))
                    .top(s(6.))
                    .h(s(108.))
                    .rounded_tl(s(11.))
                    .rounded_tr(s(11.))
                    .bg(c(0xFFFFFF))
                    .shadow_sm()
                    .flex()
                    .flex_col()
                    .pt(s(20.))
                    .px(s(23.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .mb(s(12.))
                            .child(div().w(s(12.)).h(s(12.)).rounded(s(3.)).bg(c(0x8FC4F6)))
                            .child(
                                div()
                                    .w(gpui::relative(0.4))
                                    .h(s(9.))
                                    .rounded(s(3.))
                                    .bg(c(0xB9CDE4)),
                            ),
                    )
                    .child(div().h(s(6.)).rounded(s(6.)).bg(c(0xDCE8E6)).mb(s(11.)))
                    .child(
                        div()
                            .w(gpui::relative(0.7))
                            .h(s(6.))
                            .rounded(s(6.))
                            .bg(c(0xDCE8E6))
                            .mb(s(11.)),
                    )
                    .child(div().h(s(6.)).rounded(s(6.)).bg(c(0xDCE8E6))),
            )
            // 机身（渐变 + 顶部高光 + 面板接缝）
            .child(
                div()
                    .absolute()
                    .left(s(29.))
                    .right(s(29.))
                    .top(s(94.))
                    .h(s(113.))
                    .rounded_tl(s(31.))
                    .rounded_tr(s(31.))
                    .rounded_bl(s(25.))
                    .rounded_br(s(25.))
                    .bg(linear_gradient(
                        145.,
                        linear_color_stop(c(theme::PRINTER_TOP), 0.),
                        linear_color_stop(c(theme::PRINTER_BOTTOM), 1.),
                    ))
                    .shadow_md()
                    .overflow_hidden()
                    // 顶部玻璃高光
                    .child(
                        div()
                            .absolute()
                            .top(s(0.))
                            .left(s(0.))
                            .right(s(0.))
                            .h(s(34.))
                            .bg(gpui::linear_gradient(
                                180.,
                                gpui::linear_color_stop(gpui::rgba(0xFFFFFF40), 0.),
                                gpui::linear_color_stop(gpui::rgba(0xFFFFFF00), 1.),
                            )),
                    )
                    // 状态指示灯（柔光晕 + 灯芯）
                    .child(
                        div()
                            .absolute()
                            .right(s(22.))
                            .top(s(22.))
                            .size(s(21.))
                            .rounded_full()
                            .bg(gpui::rgba(0x86E7B540)),
                    )
                    .child(
                        div()
                            .absolute()
                            .right(s(26.))
                            .top(s(26.))
                            .size(s(13.))
                            .rounded_full()
                            .bg(c(0x86E7B5)),
                    )
                    // 前面板接缝线
                    .child(
                        div()
                            .absolute()
                            .left(s(30.))
                            .right(s(30.))
                            .top(s(52.))
                            .h(s(2.))
                            .rounded(s(2.))
                            .bg(gpui::rgba(0xFFFFFF33)),
                    )
                    // 出纸槽
                    .child(
                        div()
                            .absolute()
                            .left(s(37.))
                            .right(s(37.))
                            .bottom(s(21.))
                            .h(s(16.))
                            .rounded(s(10.))
                            .bg(c(0x075F5D)),
                    ),
            )
            // 出纸口纸张
            .child(
                div()
                    .absolute()
                    .left(s(72.))
                    .right(s(72.))
                    .top(s(189.))
                    .h(s(68.))
                    .rounded_bl(s(9.))
                    .rounded_br(s(9.))
                    .bg(c(0xFFFFFF))
                    .shadow_sm()
                    .flex()
                    .flex_col()
                    .pt(s(21.))
                    .px(s(21.))
                    .child(div().h(s(6.)).rounded(s(6.)).bg(c(0xDCE8E6)).mb(s(11.)))
                    .child(
                        div()
                            .w(gpui::relative(0.7))
                            .h(s(6.))
                            .rounded(s(6.))
                            .bg(c(0xDCE8E6)),
                    ),
            )
            // 底部投影（悬浮感）
            .child(
                div()
                    .absolute()
                    .left(s(62.))
                    .right(s(62.))
                    .bottom(s(2.))
                    .h(s(14.))
                    .rounded_full()
                    .bg(gpui::rgba(0x337ECC2B)),
            )
            .into_any_element()
    }

    /// 操作流程步骤胶囊（渐变圆形序号徽章 + 图标 + 步骤名，白色胶囊底）
    fn flow_step(num: &'static str, label: &'static str, icon: &'static str) -> gpui::AnyElement {
        div()
            .flex()
            .items_center()
            .gap_3()
            .pl(s(8.))
            .pr(s(26.))
            .py(s(8.))
            .rounded_full()
            .bg(c(0xFFFFFF))
            .border_1()
            .border_color(c(theme::CARD_LINE))
            .shadow_sm()
            .child(
                div()
                    .flex_none()
                    .size(s(44.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(linear_gradient(
                        135.,
                        linear_color_stop(c(theme::BUTTON_TOP), 0.),
                        linear_color_stop(c(theme::BUTTON_BOTTOM), 1.),
                    ))
                    .shadow_sm()
                    .text_size(ts(20.))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .text_color(c(0xFFFFFF))
                    .child(num.to_string()),
            )
            .child(div().flex_none().child(icons::icon(icon, 26., theme::TEAL)))
            .child(
                div()
                    .text_size(ts(22.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(theme::INK))
                    .child(label.to_string()),
            )
            .into_any_element()
    }

    /// 标签牌内的清单小图标（三行递减白条，纯 div 绘制）
    fn flow_label_glyph() -> gpui::AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(s(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(s(4.))
                    .child(div().size(s(4.)).rounded_full().bg(gpui::rgba(0xFFFFFF)))
                    .child(
                        div()
                            .w(s(18.))
                            .h(s(4.))
                            .rounded(s(4.))
                            .bg(gpui::rgba(0xFFFFFF)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(s(4.))
                    .child(div().size(s(4.)).rounded_full().bg(gpui::rgba(0xFFFFFFB3)))
                    .child(
                        div()
                            .w(s(13.))
                            .h(s(4.))
                            .rounded(s(4.))
                            .bg(gpui::rgba(0xFFFFFFB3)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(s(4.))
                    .child(div().size(s(4.)).rounded_full().bg(gpui::rgba(0xFFFFFF80)))
                    .child(
                        div()
                            .w(s(8.))
                            .h(s(4.))
                            .rounded(s(4.))
                            .bg(gpui::rgba(0xFFFFFF80)),
                    ),
            )
            .into_any_element()
    }
}

/// 步骤之间的箭头
fn home_step_arrow() -> gpui::AnyElement {
    div()
        .flex_none()
        .px(s(6.))
        .text_size(ts(22.))
        .font_weight(FontWeight::EXTRA_BOLD)
        .text_color(c(theme::TEAL))
        .child("→")
        .into_any_element()
}
