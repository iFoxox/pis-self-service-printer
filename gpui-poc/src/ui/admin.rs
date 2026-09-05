//! 管理员验证弹窗（对应 App.vue exit-password-card：4 位密码 + 键盘 + 30s 倒计时）
//!
//! gpui-0.2：外壳改为 gpui-component 的 Dialog（由 state.rs open_admin 经
//! 窗口句柄打开），内容为 AdminView 视图——密码点/倒计时实时变化，
//! 因此必须用 Entity 视图而非一次性元素。

use gpui::prelude::*;
use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, div, WeakEntity};
use gpui_component::button::Button;
use gpui_component::{button::ButtonVariants as _, WindowExt as _};

use crate::state::{AdminMatch, KioskState};
use crate::theme::{self, c, s};
use crate::widgets;

/// 在窗口上打开管理员验证 Dialog（由 KioskState::open_admin 调用）
pub fn open_admin_dialog(
    window: &mut gpui::Window,
    cx: &mut gpui::App,
    kiosk: &gpui::Entity<KioskState>,
) {
    let view = cx.new(|cx| AdminView::new(kiosk.clone(), cx));
    let kiosk_weak = kiosk.downgrade();
    let margin_top = crate::ui::dialog_center_margin(window, 720.);
    window.open_dialog(cx, {
        let view = view.clone();
        let kiosk_weak = kiosk_weak.clone();
        move |dialog, _window, _cx| {
            let kiosk_weak = kiosk_weak.clone();
            dialog
                .title("管理员验证")
                .w(s(620.))
                .margin_top(margin_top)
                // 无遮罩点击关闭 / 无右上角 X / 键盘不绑定：
                // 退出只经「取消」「Esc（根键盘路由）」与倒计时，状态单一来源
                .overlay_closable(false)
                .close_button(false)
                .keyboard(false)
                .on_close(move |_, _, cx| {
                    if let Some(kiosk) = kiosk_weak.upgrade() {
                        kiosk.update(cx, |state, _| state.close_admin());
                    }
                })
                .child(view.clone())
        }
    });
}

/// 管理员验证弹窗内容视图：随 KioskState 通知实时刷新（密码点/错误/倒计时）
struct AdminView {
    kiosk: WeakEntity<KioskState>,
}

impl AdminView {
    fn new(kiosk: gpui::Entity<KioskState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&kiosk, |_, _, cx| cx.notify()).detach();
        Self {
            kiosk: kiosk.downgrade(),
        }
    }
}

impl Render for AdminView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let kiosk = self.kiosk.upgrade();
        let Some(kiosk) = kiosk else {
            return div().into_any_element();
        };
        let state = kiosk.read(cx);
        let password = state.admin_password.clone();
        let error = state.admin_error.clone();
        let countdown = state.admin_countdown;
        let kiosk = kiosk.clone();

        // 密码圆点
        let mut dots = div().flex().items_center().justify_center().gap(s(15.));
        for i in 0..4 {
            let filled = password.len() > i;
            dots = dots.child(
                div()
                    .size(s(18.))
                    .rounded_full()
                    .border_2()
                    .border_color(c(if error.is_empty() { 0xA8BAD0 } else { 0xF19A95 }))
                    .bg(c(if filled { theme::TEAL } else { theme::CARD }))
                    .when(filled, |el| el.shadow_sm()),
            );
        }

        // 键盘（kiosk 专属触控键盘，保留自绘按下高亮）
        // 行内 flex_1 自适应 Dialog 宽度，任何缩放比例下都不会溢出裁切
        let key_rows: [[&str; 3]; 4] = [
            ["1", "2", "3"],
            ["4", "5", "6"],
            ["7", "8", "9"],
            ["清空", "0", "退格"],
        ];
        let mut keypad = div().flex().flex_col().gap_3().mt_5().w_full();
        for row in key_rows {
            let mut row_el = div().flex().gap_3().w_full();
            for key in row {
                let is_utility = key == "清空" || key == "退格";
                let kiosk = kiosk.clone();
                row_el = row_el.child(
                    div().flex_1().min_h(s(68.)).child(widgets::keypad_button(
                        key,
                        false,
                        is_utility,
                        move |_event, _window, cx| {
                            kiosk.update(cx, |state, cx| {
                                state.press_admin_key(key, cx);
                                cx.notify();
                            });
                        },
                    )),
                );
            }
            keypad = keypad.child(row_el);
        }

        // 倒计时 + 错误提示
        let mut notice = div().mt(s(14.)).text_xs().text_color(c(theme::MUTED));
        if error.is_empty() {
            notice = notice.child(format!("{countdown} 秒后自动关闭"));
        } else {
            notice = notice
                .font_weight(FontWeight::BOLD)
                .text_color(c(theme::DANGER))
                .child(error.clone());
        }

        let cancel_kiosk = kiosk.clone();
        let confirm_kiosk = kiosk.clone();

        div()
            .flex()
            .flex_col()
            .items_center()
            .child(dots)
            .child(notice)
            .child(keypad)
            .child(
                div()
                    .flex()
                    .w_full()
                    .gap_3()
                    .mt(s(18.))
                    .child(
                        Button::new("admin-cancel")
                            .outline()
                            .child(widgets::kiosk_label("取消", 24.))
                            .min_h(s(52.))
                            .flex_1()
                            .on_click(move |_event, window, cx| {
                                cancel_kiosk.update(cx, |state, cx| {
                                    state.play_click(cx);
                                    state.close_admin();
                                    cx.notify();
                                });
                                window.close_dialog(cx);
                            }),
                    )
                    .child(
                        Button::new("admin-confirm")
                            .primary()
                            .child(widgets::kiosk_label("确认", 24.))
                            .min_h(s(52.))
                            .flex_1()
                            .on_click(move |_event, window, cx| {
                                let matched = confirm_kiosk.update(cx, |state, cx| {
                                    state.play_click(cx);
                                    state.confirm_admin()
                                });
                                // 密码匹配才会返回 Some（此时关闭弹窗）；
                                // 密码错误保留弹窗展示错误提示
                                if let Some(matched) = matched {
                                    window.close_dialog(cx);
                                    match matched {
                                        AdminMatch::Minimize => window.minimize_window(),
                                        AdminMatch::Settings => {
                                            confirm_kiosk.update(cx, |state, cx| {
                                                state.open_settings(window, cx);
                                            });
                                        }
                                        AdminMatch::Logs => {
                                            confirm_kiosk.update(cx, |state, cx| {
                                                state.open_logs(cx);
                                            });
                                        }
                                    }
                                }
                            }),
                    ),
            )
            .into_any_element()
    }
}
