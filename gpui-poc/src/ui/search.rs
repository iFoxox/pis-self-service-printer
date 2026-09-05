//! 查询页（对应 SearchView.vue：步骤标题 + 大输入框 + 14 键触控键盘 + 查询遮罩）

use gpui::{
    Animation, AnimationExt, Context, FontWeight, InteractiveElement as _, IntoElement,
    MouseButton, ParentElement, Styled, div,
};


use crate::state::KioskState;
use crate::theme::{self, c, s, ts};
use crate::{icons, widgets};

/// 闪烁光标（空态时置于文字最左，有内容时跟在文字后）
fn caret_element() -> impl gpui::IntoElement {
    div()
        .flex_none()
        .w(s(4.))
        .h(s(54.))
        .rounded(s(2.))
        .bg(c(theme::TEAL))
        .with_animation(
            "caret",
            Animation::new(std::time::Duration::from_millis(1100)).repeat(),
            |el, delta| el.opacity(if delta < 0.6 { 1.0 } else { 0.0 }),
        )
}

/// 键盘按键（1-9 / X / 0 / 00；清空与退格在末行单独成行，避免重复）
const KEYS: [&str; 12] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "X", "0", "00"];

impl KioskState {
    /// 步骤标题行（查询 / 报告页共用）
    ///
    /// `subtitle_value`：副标题尾部追加的强调值（主题蓝加粗显示）
    pub fn render_step_heading(
        &mut self,
        step: &'static str,
        label: &str,
        title: &str,
        subtitle: String,
        subtitle_value: Option<String>,
    ) -> gpui::AnyElement {
        let countdown = self.countdown;
        div()
            .flex()
            .items_center()
            .gap(s(18.))
            .mb(s(22.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(theme::TEAL))
                            .child(label.to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(s(18.))
                            // 数字、横线与标题同行且垂直居中对齐
                            .child(widgets::step_number(step))
                            .child(
                                div()
                                    .text_size(ts(34.))
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .text_color(c(theme::INK))
                                    .child(title.to_string()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(s(5.))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(c(theme::MUTED))
                                            .child(subtitle),
                                    )
                                    .children(subtitle_value.map(|value| {
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::EXTRA_BOLD)
                                            .text_color(c(theme::SUCCESS))
                                            .child(value)
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px(s(15.))
                    .py(s(10.))
                    .rounded(s(15.))
                    .bg(c(theme::CARD))
                    .shadow_sm()
                    .child(
                        div()
                            .text_size(ts(24.))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(c(theme::WARM))
                            .child(countdown.to_string()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_xs()
                            .text_color(c(theme::MUTED))
                            .child("秒后自动返回"),
                    ),
            )
            .into_any_element()
    }

    /// 查询 / 打印加载遮罩（对应 .query-loading-card，接管内容区）
    pub fn render_loading_overlay(
        &self,
        title: &str,
        desc: &str,
        caption: &str,
        icon: &'static str,
    ) -> gpui::AnyElement {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x19304E94))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    // 统一弹窗尺寸：720 × 520
                    .w(s(720.))
                    .h(s(520.))
                    .px(s(52.))
                    // 四角统一圆角
                    .rounded(s(24.))
                    .bg(c(theme::CARD))
                    .shadow_lg()
                    .child({
                        // 环形渐隐追逐点：8 个小圆点依次亮起再淡出，
                        // 等效于小段圆弧绕图标旋转（gpui 0.2 无旋转变换）
                        const RING_SIZE: f32 = 88.;
                        const DOT: f32 = 7.;
                        const RADIUS: f32 = (RING_SIZE - DOT) / 2. - 2.;
                        let mut ring = div()
                            .absolute()
                            .inset_0()
                            .size_full();
                        for i in 0..8usize {
                            let phase = i as f32 / 8.;
                            let angle = phase * std::f32::consts::TAU;
                            let cx = RING_SIZE / 2. + RADIUS * angle.sin();
                            let cy = RING_SIZE / 2. - RADIUS * angle.cos();
                            ring = ring.child(
                                div()
                                    .absolute()
                                    .left(s(cx - DOT / 2.))
                                    .top(s(cy - DOT / 2.))
                                    .size(s(DOT))
                                    .rounded_full()
                                    .bg(c(theme::TEAL))
                                    .with_animation(
                                        ("loading-tick", i),
                                        Animation::new(std::time::Duration::from_millis(1000))
                                            .repeat(),
                                        move |el, delta| {
                                            let t = (delta - phase).rem_euclid(1.0);
                                            el.opacity(if t < 0.2 {
                                                1.0 - (t / 0.2) * 0.75
                                            } else {
                                                0.25
                                            })
                                        },
                                    ),
                            );
                        }
                        div()
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(s(88.))
                            .rounded_full()
                            .bg(c(theme::TEAL_SOFT))
                            .text_color(c(theme::TEAL))
                            .child(icons::icon(icon, 40., theme::TEAL))
                            .child(ring)
                    })
                    .child(
                        div()
                            .mt(s(25.))
                            .text_size(ts(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(theme::TEAL))
                            .child(caption.to_string()),
                    )
                    .child(
                        div()
                            .mt(s(6.))
                            .text_size(ts(28.))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(c(theme::INK))
                            .child(title.to_string()),
                    )
                    .child(
                        div()
                            .mt(s(10.))
                            .px(s(6.))
                            .text_center()
                            // 绿色提醒，比正文弱一档但保持可读
                            .text_size(ts(19.))
                            .line_height(ts(28.))
                            .text_color(c(theme::SUCCESS))
                            .child(desc.to_string()),
                    ),
            )
            .into_any_element()
    }

    pub fn render_search(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let cfg = self.cfg();
        let hint = if cfg.terminal.input_hint.is_empty() {
            "输入登记号/病历号".to_string()
        } else {
            cfg.terminal.input_hint.clone()
        };
        let keyword = self.keyword.clone();
        let keyword_selected = self.keyword_selected;
        let loading = self.loading;
        let pressed = self.pressed_key.clone();

        let page = div()
            .flex_1()
            .flex()
            .flex_col()
            .px(s(96.))
            .pt(s(30.))
            .pb(s(24.))
            .child(self.render_step_heading(
                "01",
                "第一步",
                "输入报告查询信息",
                hint.clone(),
                None,
            ))
            .child(self.render_search_body(
                cx,
                &hint,
                &keyword,
                keyword_selected,
                loading,
                pressed,
            ));

        // 加载中：遮罩直接接管内容区（居中可靠）
        if loading {
            return self.render_loading_overlay(
                "正在查询报告",
                "正在获取已签发的病理报告，请稍候",
                "REPORT SEARCH",
                icons::SEARCH,
            );
        }
        page.into_any_element()
    }

    /// 主体：左输入面板 + 右键盘面板（半透明白卡）
    fn render_search_body(
        &mut self,
        cx: &mut Context<Self>,
        hint: &str,
        keyword: &str,
        keyword_selected: bool,
        loading: bool,
        pressed: Option<String>,
    ) -> gpui::AnyElement {
        // ==== 输入面板 ====
        // 布局：输入组占满剩余空间并垂直居中（与右侧键盘平衡），返回按钮贴底对齐查询按钮
        let input_panel = div()
            .flex()
            .flex_col()
            .flex_1()
            .pr(s(8.))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(
                        div()
                            .mb(s(14.))
                            .text_size(ts(20.))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(c(theme::HEADER_TEXT))
                            .child(hint.to_string()),
                    )
                    // 大输入框
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .relative()
                            .h(s(92.))
                            .px(s(78.))
                            .rounded_tl(s(20.))
                            .rounded_tr(s(20.))
                            .rounded_br(s(20.))
                            .rounded_bl(s(7.))
                            .border_2()
                            .border_color(c(if keyword.is_empty() {
                                theme::INPUT_BORDER
                            } else {
                                theme::TEAL
                            }))
                            .bg(c(if keyword.is_empty() {
                                theme::INPUT_BG
                            } else {
                                theme::CARD
                            }))
                            .shadow_sm()
                            // 放大镜图标
                            .child(div().absolute().left(s(32.)).child(icons::icon(
                                icons::SEARCH,
                                27.,
                                theme::TEAL,
                            )))
                            // 文本 + 光标：空态时光标在最左、占位提示很淡；有内容时光标跟在文字后
                            .child(if keyword.is_empty() {
                                div().flex().items_center().child(caret_element()).child(
                                    div()
                                        .ml(s(3.))
                                        .text_size(ts(32.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(c(0xC2D3E0))
                                        .child(format!("请{hint}")),
                                )
                            } else {
                                div()
                                    .flex()
                                    .items_center()
                                    .child(
                                        div()
                                            .px(s(3.))
                                            .rounded(s(4.))
                                            .bg(if keyword_selected {
                                                c(theme::TEAL)
                                            } else {
                                                gpui::rgba(0x00000000)
                                            })
                                            .text_size(ts(32.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c(if keyword_selected {
                                                0xFFFFFF
                                            } else {
                                                theme::INK
                                            }))
                                            .child(keyword.to_string()),
                                    )
                                    .child(div().ml(s(3.)).child(caret_element()))
                            }),
                    ),
            )
            // 返回首页（淡绿通栏按钮，与输入框等宽、和右侧查询按钮镜像呼应；
            // 按压时加深底色与描边，与数字键盘一致的 300ms 反馈）
            .child(div().pt(s(28.)).child({
                let pressed = self.pressed_action.as_deref() == Some("search-back-home");
                let btn = widgets::fw_mint("search-back-home", "返回首页", Some(icons::HOME))
                    .w_full()
                    .min_h(s(84.));
                let btn = if pressed {
                    btn.bg(c(0xBEE9D6)).border_color(c(0x7FC9AE))
                } else {
                    btn
                };
                btn.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &gpui::MouseDownEvent, _w, cx| {
                        this.press_action("search-back-home", cx);
                    }),
                )
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.play_click(cx);
                    this.go_home();
                    cx.notify();
                }))
            }));

        // ==== 键盘面板 ====
        let mut keypad = div().flex().flex_col().gap_3();
        for row in KEYS.chunks(3) {
            let mut row_el = div().flex().gap_3();
            for key in row {
                let key = key.to_string();
                let is_utility = key == "清空";
                row_el = row_el.child(widgets::keypad_button(
                    key.clone(),
                    pressed.as_deref() == Some(key.as_str()),
                    is_utility,
                    cx.listener(move |this, _event, _window, cx| {
                        this.press_keyword_key(&key, cx);
                        cx.notify();
                    }),
                ));
            }
            keypad = keypad.child(row_el);
        }
        // 末行：清空 + 退格（宽）
        keypad = keypad.child(
            div()
                .flex()
                .gap_3()
                .child(widgets::keypad_button(
                    "清空",
                    pressed.as_deref() == Some("清空"),
                    true,
                    cx.listener(|this, _event, _window, cx| {
                        this.press_keyword_key("清空", cx);
                        cx.notify();
                    }),
                ))
                .child(widgets::keypad_backspace_button(
                    pressed.as_deref() == Some("退格"),
                    cx.listener(|this, _event, _window, cx| {
                        this.press_keyword_key("退格", cx);
                        cx.notify();
                    }),
                )),
        );

        let query_label = if loading {
            "正在查询..."
        } else {
            "查询可打印报告"
        };

        let keypad_panel = div()
            .flex()
            .flex_col()
            .gap_3()
            .w(s(580.))
            .justify_end()
            .child(keypad)
            .child(if loading {
                widgets::disabled(true, query_label)
                    .min_h(s(84.))
                    .w_full()
                    .into_any_element()
            } else {
                let pressed = self.pressed_action.as_deref() == Some("submit-query");
                let btn = widgets::fw_primary("submit-query", query_label)
                    .min_h(s(84.))
                    .w_full();
                let btn = if pressed {
                    btn.bg(c(theme::BUTTON_BOTTOM))
                } else {
                    btn
                };
                btn.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &gpui::MouseDownEvent, _w, cx| {
                        this.press_action("submit-query", cx);
                    }),
                )
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.submit_query(cx);
                    cx.notify();
                }))
                .into_any_element()
            });

        div()
            .flex()
            .flex_1()
            .gap(s(56.))
            .px(s(56.))
            .py(s(36.))
            .rounded_tl(s(34.))
            .rounded_tr(s(10.))
            .rounded_br(s(34.))
            .rounded_bl(s(34.))
            .bg(gpui::rgba(0xFFFFFF2E))
            .border_1()
            .border_color(gpui::rgba(0xFFFFFF66))
            .shadow_none()
            .child(input_panel)
            .child(keypad_panel)
            .into_any_element()
    }
}
