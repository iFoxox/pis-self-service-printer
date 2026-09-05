//! 报告页（对应 ReportsView.vue：报告表格 + 全选 + 打印流程遮罩 + 成功反馈）

use gpui::prelude::*;
use gpui::{
    Animation, AnimationExt, Context, FontWeight, IntoElement, MouseButton, ParentElement, Styled,
    div,
};
use gpui_component::scroll::ScrollableElement as _;

use crate::state::{KioskState, Page};
use crate::theme::{self, c, s, ts};
use crate::{icons, widgets};

/// 报告类型文案（对应 reportTypeMap）
fn report_type_style(report_type: i64) -> (&'static str, u32, u32) {
    match report_type {
        0 => ("常规报告", 0xE8F3FF, 0x1D66C5),
        1 | 5 => ("补充报告", 0xFFF4E3, 0x9A6200),
        2 => ("迟发报告", 0xFDECEC, 0xC13B35),
        4 => ("分子报告", 0xF1EBFF, 0x6742B5),
        _ => ("未知类型", 0xEEF2F6, 0x55657A),
    }
}

/// 签发时间截断到分钟（对应 formatAuthorizeAt）
fn format_authorize_at(value: &Option<String>) -> String {
    match value {
        Some(v) if v.len() >= 16 => v[..16].replace('T', " "),
        Some(v) => v.clone(),
        None => "--".to_string(),
    }
}

/// 估算文本宽度：CJK 近似全宽，拉丁/数字近似半宽
fn estimated_text_width(text: &str, size: f32) -> f32 {
    text.chars()
        .map(|ch| {
            if ch.is_ascii() {
                size * 0.58
            } else {
                size * 0.98
            }
        })
        .sum()
}

/// 收费项目列内容宽度（设计稿像素），与固定列 / 页面留白保持同一套公式
fn exam_column_width() -> f32 {
    let (viewport_width, _) = theme::viewport_logical();
    let design_width = viewport_width / theme::ui_scale();
    (design_width - 96. * 2. - 2. * 2. - 2. * 2. - 4. - 64. - 130.) / 5.
}

/// 超长收费项目横向循环滚动；普通长度保持居中
fn exam_item_cell(text: String, index: usize) -> gpui::AnyElement {
    let text_size = 20.;
    let column_width = exam_column_width().max(120.);
    let text_width = estimated_text_width(&text, text_size);
    let separator_width = text_size * 0.58 * 8.;
    let base_cell = div()
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .justify_center()
        .border_l_2()
        .border_color(c(theme::ROW_BORDER))
        .px(s(14.));

    if text_width + 16. <= column_width {
        return base_cell
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(ts(text_size))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(theme::INK))
                    .child(text),
            )
            .into_any_element();
    }

    let scrolling_text = format!("{text}        {text}");
    let travel_ratio = ((text_width + separator_width) / column_width).max(0.05);
    let duration = 7.0 + (text.chars().count() as f32 * 0.14).min(9.0);
    base_cell
        .child(
            div().relative().w_full().h(s(30.)).overflow_hidden().child(
                div()
                    .absolute()
                    .top(s(4.))
                    .left(gpui::relative(0.))
                    .whitespace_nowrap()
                    .text_size(ts(text_size))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(theme::INK))
                    .child(scrolling_text)
                    .with_animation(
                        ("exam-item-marquee", index),
                        Animation::new(std::time::Duration::from_secs_f32(duration)).repeat(),
                        move |el, delta| el.left(gpui::relative(-travel_ratio * delta)),
                    ),
            ),
        )
        .into_any_element()
}

impl KioskState {
    pub fn render_reports(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let loading = self.loading || self.printing;

        // 打印 / 查询中：遮罩直接接管内容区（居中可靠）
        if loading {
            let (title, desc, caption, icon) = if self.printing {
                (
                    "正在打印报告",
                    "报告已发送至打印机，请稍候，不要离开",
                    "REPORT PRINTING",
                    icons::PRINTER,
                )
            } else {
                (
                    "正在查询报告",
                    "正在获取已签发的病理报告，请稍候",
                    "REPORT SEARCH",
                    icons::SEARCH,
                )
            };
            return self.render_loading_overlay(title, desc, caption, icon);
        }

        // 打印成功：成功反馈直接接管内容区
        if self.completed {
            return self.render_success_overlay(cx);
        }

        div()
            .flex_1()
            .flex()
            .flex_col()
            .px(s(96.))
            .pt(s(8.))
            .pb(s(16.))
            // 没有查到报告时不显示「选择需要打印的报告」标题（无从选择）
            .children((!self.reports.is_empty()).then(|| {
                self.render_step_heading(
                    "02",
                    "第二步",
                    "选择需要打印的报告",
                    "查询信息：".to_string(),
                    Some(self.keyword.clone()),
                )
            }))
            .child(self.render_reports_body(cx))
            .into_any_element()
    }

    /// 报告工作区（工具栏 + 表格 + 底部操作条）或空态
    fn render_reports_body(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.reports.is_empty() {
            return self.render_empty_state(cx);
        }

        let total = self.reports.len();
        let all_selected = self.all_selected();

        // ==== 表格 ====
        let mut rows = div()
            .id("report-rows")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            // 框架滚动条：滚轮 + 可拖动拇指（触屏自助终端常显）
            .overflow_y_scrollbar();
        for index in 0..self.reports.len() {
            rows = rows.child(self.render_report_row(cx, index));
        }

        let workspace = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .px(s(2.))
            .pt(s(2.))
            .pb(s(2.))
            .rounded_tl(s(28.))
            .rounded_tr(s(8.))
            .rounded_br(s(28.))
            .rounded_bl(s(28.))
            .bg(gpui::rgba(0xFFFFFFB8))
            .border_1()
            .border_color(gpui::rgba(0xFFFFFFD9))
            .shadow_md()
            // 工具栏：左侧结果摘要卡 + 右侧全选切换按钮
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pb(s(6.))
                    // 结果摘要：文档图标 + 大数字 + 患者引导语
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(s(12.))
                            .px(s(16.))
                            .py(s(6.))
                            .rounded(s(14.))
                            .bg(c(0xF0F7FF))
                            .border_1()
                            .border_color(c(0xD5E8FF))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .size(s(38.))
                                    .rounded(s(10.))
                                    .bg(c(theme::TEAL_SOFT))
                                    .child(icons::icon(icons::REPORT, 22., theme::TEAL)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(s(1.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(s(5.))
                                            // 数字与文字同字号，保证基线对齐
                                            .text_size(ts(20.))
                                            .line_height(ts(28.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c(theme::INK))
                                            .child("查询到")
                                            .child(
                                                div()
                                                    .font_weight(FontWeight::EXTRA_BOLD)
                                                    .text_color(c(theme::TEAL))
                                                    .child(total.to_string()),
                                            )
                                            .child("份报告"),
                                    )
                                    .child(
                                        div()
                                            .px(s(10.))
                                            .py(s(2.))
                                            .rounded(s(8.))
                                            .bg(c(0xFFF6DC))
                                            .text_size(ts(13.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(c(0x8A6116))
                                            .child("请核对姓名与报告类型，勾选需要打印的报告"),
                                    ),
                            ),
                    )
                    // 全选切换：药丸按钮整块可点，选中后实心蓝
                    .child(
                        div()
                            .id("select-all")
                            .flex()
                            .items_center()
                            .gap(s(12.))
                            .px(s(24.))
                            .py(s(13.))
                            .rounded_full()
                            .shadow_sm()
                            .cursor_pointer()
                            // 选中态用浅蓝底 + 蓝字，勾选框保持可见，避免整块实心蓝
                            .bg(if all_selected {
                                c(theme::TEAL_SOFT)
                            } else {
                                c(0xFFFFFF)
                            })
                            .border_2()
                            .border_color(if all_selected {
                                c(theme::TEAL)
                            } else {
                                c(0xB9D7F5)
                            })
                            .child(widgets::checkbox_success(all_selected, 26., false))
                            .child(
                                div()
                                    .text_size(ts(19.))
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .text_color(c(theme::TEAL_DARK))
                                    .child(if all_selected {
                                        "已全选，点击取消"
                                    } else {
                                        "全部选择"
                                    }),
                            )
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.play_click(cx);
                                this.toggle_select_all();
                                cx.notify();
                            })),
                    ),
            )
            // 列表容器：表头 + 可滚动表体
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .border_2()
                    .border_color(c(0xD7E1EC))
                    .rounded(s(16.))
                    .bg(c(theme::CARD))
                    .overflow_hidden()
                    .child(self.render_table_header())
                    .child(rows),
            )
            // 底部操作条
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt(s(14.))
                    .border_t_1()
                    .border_color(c(theme::LINE))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(s(20.))
                            .child(
                                widgets::fw_mint(
                                    "reports-back-home",
                                    "返回首页",
                                    Some(icons::HOME),
                                )
                                .min_h(s(64.))
                                .min_w(s(220.))
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.play_click(cx);
                                    this.go_home();
                                    cx.notify();
                                })),
                            )
                            .child(
                                widgets::fw_warm(
                                    "reports-prev-step",
                                    "上一步",
                                    Some(icons::ARROW_LEFT),
                                )
                                .min_h(s(64.))
                                .min_w(s(220.))
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.play_click(cx);
                                    this.page = Page::Search;
                                    // 返回查询页：倒计时重新开始
                                    this.reset_countdown();
                                    cx.notify();
                                })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(s(20.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(s(4.))
                                    // 数字与文字同字号，保证基线对齐
                                    .text_size(ts(20.))
                                    .line_height(ts(28.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c(theme::MUTED))
                                    .child("已选择")
                                    .child(
                                        div()
                                            .font_weight(FontWeight::EXTRA_BOLD)
                                            .text_color(c(0x1E9E6A))
                                            .child(self.selected_count().to_string()),
                                    )
                                    .child("份"),
                            )
                            .child(self.render_print_button(cx)),
                    ),
            );
        workspace.into_any_element()
    }

    /// 确认打印按钮（禁用态随选中 / 打印中变化；0 选中时给出引导文案）
    fn render_print_button(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let count = self.selected_count();
        let printing = self.printing;
        if printing || count == 0 {
            let label = if printing {
                "正在提交打印..."
            } else if count == 0 {
                "请先选择报告"
            } else {
                "确认打印"
            };
            return widgets::fw_disabled("confirm-print-disabled", label).into_any_element();
        }
        widgets::fw_primary("confirm-print", "确认打印")
            .min_h(s(58.))
            .min_w(s(230.))
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.print_selected(cx);
                cx.notify();
            }))
            .into_any_element()
    }

    /// 表头行（蓝渐变；字号 ≥ 正文，保证层级）
    fn render_table_header(&self) -> gpui::AnyElement {
        let cells = [
            "患者姓名",
            "病理号",
            "报告类型",
            "收费项目",
            "签发时间",
            "打印状态",
        ];
        let mut header = div()
            .flex()
            .min_h(s(64.))
            .flex_none()
            // 数据行左侧有 4px 选中指示条；表头同步预留，弹性列宽才能逐列对齐
            .border_l_4()
            .border_color(gpui::rgba(0x00000000))
            .rounded_tl(s(12.))
            .rounded_tr(s(12.))
            .bg(gpui::linear_gradient(
                135.,
                gpui::linear_color_stop(c(theme::BUTTON_TOP), 0.),
                gpui::linear_color_stop(c(theme::BUTTON_BOTTOM), 1.),
            ))
            .shadow_sm()
            .child(
                div()
                    .w(s(64.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(ts(22.))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .text_color(c(0xFFFFFF))
                    .child("选择"),
            );
        for text in cells.iter() {
            let is_status = *text == "打印状态";
            let cell = div()
                .min_w_0()
                .when_else(is_status, |el| el.flex_none().w(s(130.)), |el| el.flex_1())
                .flex()
                .items_center()
                .justify_center()
                .border_l_2()
                .border_color(gpui::rgba(0xFFFFFF3D))
                .px(s(14.))
                .text_size(ts(22.))
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(c(0xFFFFFF))
                .child(text.to_string());
            header = header.child(cell);
        }
        header.into_any_element()
    }

    /// 单行报告
    fn render_report_row(&mut self, cx: &mut Context<Self>, index: usize) -> gpui::AnyElement {
        let report = self.reports[index].clone();
        let is_selected = self.selected.get(index).copied().unwrap_or(false);
        let disabled = self.report_disabled(&report);

        let name = if report.subject_name.is_empty() {
            "未提供".to_string()
        } else {
            report.subject_name.clone()
        };
        let pathology_no = report
            .pathology_no
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "未提供".to_string());
        let exam_item = report
            .master_item_name
            .clone()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "未提供".to_string());
        let printed = report.is_patient_print == Some(1);
        let print_count = report.patient_print_count;
        let (type_label, type_bg, type_fg) = report_type_style(report.report_type);

        let (row_bg, status_bg) = if disabled {
            (c(theme::ROW_DISABLED), c(theme::STATUS_PILL_PRINTED))
        } else if is_selected {
            (c(theme::SELECTED_BG), c(theme::TEAL_SOFT))
        } else {
            (c(theme::CARD), c(theme::TEAL_SOFT))
        };

        let row = div()
            .flex()
            .min_h(s(72.))
            .flex_none()
            .bg(row_bg)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _e, _w, cx| {
                    if disabled {
                        return;
                    }
                    this.play_click(cx);
                    this.toggle_report(index);
                    cx.notify();
                }),
            )
            .child(div().w(s(4.)).flex_none().bg(if is_selected && !disabled {
                c(theme::TEAL)
            } else {
                gpui::rgba(0x00000000)
            }))
            .child(
                div()
                    .w(s(64.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(widgets::checkbox(is_selected && !disabled, 34., disabled)),
            )
            .child(self.table_cell(name, 24., FontWeight::EXTRA_BOLD))
            .child(self.table_cell(pathology_no, 20., FontWeight::BOLD))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_l_2()
                    .border_color(c(theme::ROW_BORDER))
                    .px(s(14.))
                    .child(
                        div()
                            .px(s(14.))
                            .py(s(7.))
                            .rounded(s(10.))
                            .bg(c(type_bg))
                            .text_size(ts(18.))
                            .line_height(ts(18.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(type_fg))
                            .child(type_label),
                    ),
            )
            .child(exam_item_cell(exam_item, index))
            .child(self.table_cell(
                format_authorize_at(&report.authorize_at),
                20.,
                FontWeight::BOLD,
            ))
            .child(
                div()
                    .flex_none()
                    .w(s(130.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(s(9.))
                    .border_l_2()
                    .border_color(c(theme::ROW_BORDER))
                    .child(
                        div()
                            .px(s(13.))
                            .py(s(8.))
                            .rounded(s(10.))
                            .bg(status_bg)
                            .text_size(ts(16.))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(c(theme::INK))
                            .child(if printed { "已打印" } else { "可打印" }),
                    )
                    .child(
                        div()
                            .text_size(ts(14.))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(c(theme::INK))
                            .child(format!("打印次数 {print_count}")),
                    ),
            );
        let row = row.into_any_element();

        // 显式渲染分隔线：滚动行容器里的 border_b 在部分 GPUUI 版本下不够稳定
        div()
            .flex()
            .flex_col()
            .flex_none()
            .child(row)
            .child(div().h(s(2.)).flex_none().bg(c(0xC9D8E8)))
            .into_any_element()
    }

    /// 表格居中单元格
    fn table_cell(&self, text: String, size: f32, weight: FontWeight) -> gpui::AnyElement {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .justify_center()
            .border_l_2()
            .border_color(c(theme::ROW_BORDER))
            .px(s(14.))
            .text_size(ts(size))
            .font_weight(weight)
            .text_color(c(0x111827))
            .child(text)
            .into_any_element()
    }

    /// 空态（对应 .empty-state；保留倒计时提示，返回时不清空查询信息）
    fn render_empty_state(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let countdown = self.countdown;
        div()
            .flex_1()
            .flex()
            .flex_col()
            .w_full()
            // 右上角倒计时徽章（空态也要提示自动返回）
            .child(
                div()
                    .flex()
                    .justify_end()
                    .pb(s(10.))
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
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .flex_1()
                    .w_full()
            .rounded_tl(s(30.))
            .rounded_tr(s(8.))
            .rounded_br(s(30.))
            .rounded_bl(s(30.))
            .bg(gpui::rgba(0xFFFFFFB8))
            .shadow_md()
            .child(
                div()
                    .relative()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(s(100.))
                    .rounded_tl(s(32.))
                    .rounded_tr(s(32.))
                    .rounded_br(s(32.))
                    .rounded_bl(s(8.))
                    .bg(c(theme::TEAL_SOFT))
                    .text_color(c(theme::TEAL))
                    .child(icons::icon(icons::DOCUMENT, 46., theme::TEAL))
                    .child(
                        div()
                            .absolute()
                            .right(s(-5.))
                            .bottom(s(-5.))
                            .size(s(24.))
                            .rounded_full()
                            .border_4()
                            .border_color(c(0xFFFFFF))
                            .bg(c(theme::WARM)),
                    ),
            )
            .child(
                div()
                    .mt(s(24.))
                    .text_size(ts(25.))
                    .font_weight(FontWeight::EXTRA_BOLD)
                    .text_color(c(theme::INK))
                    .child("暂未查询到可显示的报告"),
            )
            .child(
                div()
                    .mt(s(8.))
                    .mb(s(24.))
                    .px(s(12.))
                    .py(s(4.))
                    .rounded(s(8.))
                    // 淡黄色提醒徽章（与报告列表摘要卡提醒一致）
                    .bg(c(0xFFF6DC))
                    .text_size(ts(15.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(0x8A6116))
                    .child("请核对输入的信息，或确认报告是否已经签发。"),
            )
            .child(
                widgets::fw_primary("empty-back-search", "返回重新查询")
                    .min_h(s(56.))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.play_click(cx);
                        // 保留已输入的查询信息，方便直接重查；倒计时重新开始
                        this.page = Page::Search;
                        this.reset_countdown();
                        cx.notify();
                    })),
            ),
            )
            .into_any_element()
    }

    /// 打印成功遮罩（对应 .success-card，6 秒后自动关闭）
    fn render_success_overlay(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let count = self.completed_count;
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x0F2B3099))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    // 统一弹窗尺寸：720 × 520
                    .w(s(720.))
                    .h(s(520.))
                    .px(s(40.))
                    // 四角统一圆角
                    .rounded(s(24.))
                    .bg(c(theme::CARD))
                    .shadow_lg()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(s(90.))
                            .rounded_full()
                            .bg(gpui::linear_gradient(
                                145.,
                                gpui::linear_color_stop(c(theme::PRINTER_TOP), 0.),
                                gpui::linear_color_stop(c(theme::PRINTER_BOTTOM), 1.),
                            ))
                            .text_color(c(0xFFFFFF))
                            .shadow_md()
                            .child(icons::icon(icons::CHECK, 46., 0xFFFFFF)),
                    )
                    .child(
                        div()
                            .mt(s(25.))
                            .text_size(ts(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(theme::TEAL))
                            .child("PRINTING COMPLETE"),
                    )
                    .child(
                        div()
                            .mt(s(6.))
                            .text_size(ts(28.))
                            .font_weight(FontWeight::EXTRA_BOLD)
                            .text_color(c(theme::INK))
                            .child("报告已送往打印机"),
                    )
                    .child(
                        div()
                            .mt(s(10.))
                            .px(s(6.))
                            .flex()
                            .flex_wrap()
                            .justify_center()
                            .text_center()
                            // 文案绿色，份数数字淡黄色突出
                            .text_size(ts(19.))
                            .line_height(ts(30.))
                            .text_color(c(theme::SUCCESS))
                            .child("本次共打印 ")
                            .child(
                                div()
                                    .font_weight(FontWeight::EXTRA_BOLD)
                                    .text_color(c(0xF7D154))
                                    .child(count.to_string()),
                            )
                            .child(" 份报告，请在出纸口取走并妥善保管。即将返回报告列表。"),
                    )
                    .child(
                        div()
                            .mt(s(15.))
                            .flex()
                            .items_center()
                            .gap(s(10.))
                            .px(s(14.))
                            .py(s(7.))
                            .rounded(s(12.))
                            .bg(c(0xF3F9F5))
                            .child(
                                div().w(s(20.)).h(s(1.)).bg(c(0x8DB69C)),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(c(0x5E8C70))
                                    .child("祝您早日康复"),
                            )
                            .child(
                                div().w(s(20.)).h(s(1.)).bg(c(0x8DB69C)),
                            ),
                    )
                    // 彩虹进度条：打印进度无法从系统队列获取，用流动彩虹条表达进行中
                    .child(
                        div()
                            .mt(s(16.))
                            .relative()
                            .w_full()
                            .h(s(12.))
                            .rounded_full()
                            .bg(c(theme::PROGRESS_TRACK))
                            .overflow_hidden()
                            .child(
                                div()
                                    .absolute()
                                    .top(s(0.))
                                    .flex()
                                    .h_full()
                                    .w(gpui::relative(0.42))
                                    .rounded_full()
                                    .overflow_hidden()
                                    // gpui 0.2 渐变仅支持两个色标，用分段色块拼出彩虹
                                    .child(div().flex_1().h_full().bg(c(0xFF6B6B)))
                                    .child(div().flex_1().h_full().bg(c(0xFFB347)))
                                    .child(div().flex_1().h_full().bg(c(0xFFD93D)))
                                    .child(div().flex_1().h_full().bg(c(0x6BCB77)))
                                    .child(div().flex_1().h_full().bg(c(0x4D96FF)))
                                    .child(div().flex_1().h_full().bg(c(0x9B59B6)))
                                    .with_animation(
                                        "rainbow-bar",
                                        Animation::new(std::time::Duration::from_millis(1600))
                                            .repeat(),
                                        |el, delta| {
                                            el.left(gpui::relative(delta * 1.42 - 0.42))
                                        },
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .mt(s(20.))
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(c(theme::TEAL))
                            .on_mouse_down(MouseButton::Left, cx.listener(
                                |this, _event, _window, cx| {
                                    this.play_click(cx);
                                    this.completed = false;
                                    // 返回报告列表：空闲倒计时重新开始
                                    this.reset_countdown();
                                    cx.notify();
                                },
                            ))
                            .child("查看报告列表"),
                    ),
            )
            .into_any_element()
    }
}
