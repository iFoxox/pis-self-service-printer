//! 运行日志弹窗（对应 LogViewerDialog，文件切换 + 尾部内容）
//!
//! gpui-0.2：外壳改为 gpui-component 的 Dialog（由 state.rs open_logs 经
//! 窗口句柄打开），内容为 LogsView 视图——日期下拉与日志内容实时变化。

use gpui::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Styled, div};
use gpui_component::WindowExt as _;
use gpui_component::button::Button;
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::select::{Select, SelectEvent, SelectState};
use gpui_component::{IndexPath, Sizable as _, button::ButtonVariants as _};

use crate::state::KioskState;
use crate::theme::{c, s};

/// 日志文件名 → 下拉展示标签（app-2026-09-05.log → 2026-09-05）
fn log_label(name: &str) -> String {
    name.trim_start_matches("app-")
        .trim_end_matches(".log")
        .to_string()
}

/// 在窗口上打开运行日志 Dialog（由 KioskState::open_logs 调用）
///
/// 注意：本函数在 KioskState::update 闭包内被调用，禁止 kiosk.read(cx)
/// （GPUI 重入读取会 panic）。下拉条目留空，由 LogsView 首帧渲染时同步。
pub fn open_logs_dialog(
    window: &mut gpui::Window,
    cx: &mut gpui::App,
    kiosk: &gpui::Entity<KioskState>,
) {
    let select = cx.new(|cx| SelectState::new(Vec::new(), None, window, cx));
    let view = cx.new(|cx| LogsView::new(kiosk.clone(), select, cx));
    let kiosk_weak = kiosk.downgrade();
    let margin_top = crate::ui::dialog_center_margin(window, 700.);
    window.open_dialog(cx, {
        let view = view.clone();
        let kiosk_weak = kiosk_weak.clone();
        move |dialog, _window, _cx| {
            let kiosk_weak = kiosk_weak.clone();
            dialog
                .title("运行日志")
                .w(s(720.))
                .margin_top(margin_top)
                .on_close(move |_, _, cx| {
                    if let Some(kiosk) = kiosk_weak.upgrade() {
                        kiosk.update(cx, |state, cx| state.close_logs(cx));
                    }
                })
                .child(view.clone())
        }
    });
}

/// 运行日志内容视图：随 KioskState 通知实时刷新（文件切换 / 刷新 / 内容）
struct LogsView {
    kiosk: gpui::Entity<KioskState>,
    select: gpui::Entity<SelectState<Vec<String>>>,
    /// 已同步进下拉的文件列表快照（跨天 / 刷新后经 Window 重新 set_items）
    synced_files: Vec<String>,
}

impl LogsView {
    fn new(
        kiosk: gpui::Entity<KioskState>,
        select: gpui::Entity<SelectState<Vec<String>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&kiosk, |_, _, cx| cx.notify()).detach();
        // 下拉确认 → 按标签定位日志文件并加载
        cx.subscribe(&select, |this, _entity, event: &SelectEvent<Vec<String>>, cx| {
            if let SelectEvent::Confirm(Some(label)) = event {
                this.kiosk.update(cx, |state, cx| {
                    if let Some(index) = state
                        .logs
                        .iter()
                        .position(|name| log_label(name) == *label)
                    {
                        state.play_click(cx);
                        state.load_log(index);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
        // 同样禁止在此读取 kiosk（创建时处于 KioskState::update 内）；
        // 快照留空，首帧渲染时与实际列表做差异同步
        Self {
            kiosk,
            select,
            synced_files: Vec::new(),
        }
    }
}

impl Render for LogsView {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (content, files, current) = {
            let state = self.kiosk.read(cx);
            (
                state.log_content.clone(),
                state.logs.clone(),
                state.log_current,
            )
        };

        // 文件列表变化（刷新 / 跨天）→ 同步进下拉（set_items 需要 Window）
        if self.synced_files != files {
            self.synced_files = files.clone();
            let items: Vec<String> = files.iter().map(|n| log_label(n)).collect();
            self.select.update(cx, |select, cx| {
                select.set_items(items, window, cx);
                select.set_selected_index(Some(IndexPath::default().row(current)), window, cx);
            });
        }

        let refresh_kiosk = self.kiosk.clone();
        let close_kiosk = self.kiosk.clone();

        div()
            .flex()
            .flex_col()
            .h(s(500.))
            // 固定高度包一层：框架 Select 内部的锚定层会干扰 flex 布局，
            // 不隔离会把下方日志内容区挤塌
            .child(
                div()
                    .flex_none()
                    .h(s(44.))
                    .child(Select::new(&self.select).w_full()),
            )
            .child(
                div()
                    .id("log-content")
                    .flex_1()
                    .min_h_0()
                    .mt_3()
                    .p(s(14.))
                    .rounded(s(12.))
                    .bg(c(0x0B1B2B))
                    .overflow_y_scrollbar()
                    .text_sm()
                    .font_family("Consolas")
                    .text_color(c(0x9FD6A0))
                    .child(if content.is_empty() {
                        "暂无日志内容".to_string()
                    } else {
                        content
                    }),
            )
            // 底部操作条（分隔线 + 左刷新 / 右关闭，对齐设置页页脚）
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mt_3()
                    .pt_3()
                    .border_t_1()
                    .border_color(c(crate::theme::LINE))
                    .child(
                        Button::new("refresh-logs")
                            .small()
                            .outline()
                            .label("刷新")
                            .on_click(move |_event, _window, cx| {
                                refresh_kiosk.update(cx, |state, cx| {
                                    state.play_click(cx);
                                    state.refresh_logs(cx);
                                });
                            }),
                    )
                    .child(
                        Button::new("close-logs")
                            .small()
                            .primary()
                            .label("关闭")
                            .on_click(move |_event, window, cx| {
                                close_kiosk.update(cx, |state, cx| {
                                    state.play_click(cx);
                                    state.close_logs(cx);
                                });
                                window.close_dialog(cx);
                            }),
                    ),
            )
    }
}
