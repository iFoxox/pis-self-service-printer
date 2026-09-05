//! 通用自绘组件（按钮 / 键盘 / 卡片 / 遮罩），视觉对齐 renderer/src/assets/main.css

use gpui::{
    App, Div, FontWeight, InteractiveElement, MouseButton, MouseDownEvent, ParentElement, Styled,
    Window, div, linear_color_stop, linear_gradient,
};

use crate::theme::{self, c, s};

/// 白色卡片容器
pub fn card() -> Div {
    div()
        .flex()
        .flex_col()
        .bg(c(theme::CARD))
        .rounded(s(24.))
        .shadow_md()
}

/// 全屏遮罩（深蓝半透明，对应 .error-alert-overlay / .query-loading-overlay）
///
/// 注意：gpui 0.2.2 Windows 下 absolute 元素的 size_full 会按物理像素解析
/// （比窗口大 scale 倍），导致内容偏移出屏。因此遮罩不做绝对定位覆盖层，
/// 直接作为内容区的弹性填充子元素渲染（接管式弹窗，居中可靠）。
pub fn overlay() -> Div {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::rgba(0x0D2138B3))
}

/// 弹窗白卡（统一圆角）
pub fn dialog_card() -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .bg(c(theme::CARD))
        .rounded(s(24.))
        .px(s(46.))
        .pt(s(40.))
        .pb(s(34.))
        .shadow_lg()
}

/// 基础触控按钮：渐变底 + 可选图标 + 加粗文字（on_mouse_down 触发，免 ElementId）
pub fn action_button(
    icon: Option<&'static str>,
    label: impl Into<String>,
    top: u32,
    bottom: u32,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let mut el = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_center()
        .gap_3()
        .min_w(s(200.))
        .min_h(s(60.))
        .px(s(32.))
        .rounded(s(18.))
        .bg(linear_gradient(
            135.,
            linear_color_stop(c(top), 0.),
            linear_color_stop(c(bottom), 1.),
        ))
        .text_color(c(0xFFFFFF))
        .shadow_md()
        .text_size(theme::ts(32.))
        .font_weight(FontWeight::BOLD)
        .on_mouse_down(MouseButton::Left, on_down)
        .child(label.into());
    if let Some(path) = icon {
        el = el.child(crate::icons::icon(path, 32., 0xFFFFFF));
    }
    el
}

/// 主色按钮（蓝渐变，对应 .primary-touch-button / .query-button / .print-button）
pub fn primary_button(
    label: impl Into<String>,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    action_button(
        None,
        label,
        theme::BUTTON_TOP,
        theme::BUTTON_BOTTOM,
        on_down,
    )
}

/// 返回 / 次要导航（白底描边 + 图标色块，同一套导航配色）
///
/// 语义约定：蓝色 = 前进主操作，绿色仅用于成功态；
/// 返回 / 上一步等次要导航一律用描边次按钮，避免与主操作混淆。
pub fn ghost_button(
    icon: Option<&'static str>,
    label: impl Into<String>,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap(s(12.))
        .min_w(s(220.))
        .min_h(s(64.))
        .px(s(30.))
        .rounded(s(16.))
        .border_2()
        .border_color(c(0xD8E7F7))
        .bg(c(0xFFFFFF))
        .text_color(c(0x33547A))
        .text_size(theme::ts(28.))
        .font_weight(FontWeight::BOLD)
        .on_mouse_down(MouseButton::Left, on_down)
        .children(icon.map(|path| {
            div()
                .flex()
                .items_center()
                .justify_center()
                .size(s(38.))
                .rounded(s(11.))
                .bg(c(0xEAF3FC))
                .child(crate::icons::icon(path, 24., 0x33547A))
        }))
        .child(label.into())
}

/// 绿色导航按钮（浅绿渐变 + 深绿文字；用于查询页「返回首页」）
pub fn mint_button(
    icon: Option<&'static str>,
    label: impl Into<String>,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap_3()
        .min_h(s(84.))
        .rounded(s(20.))
        .border_2()
        .border_color(c(0x9BDCC7))
        .bg(linear_gradient(
            180.,
            linear_color_stop(c(0xE4F8F0), 0.),
            linear_color_stop(c(0xC9EFE1), 1.),
        ))
        .text_color(c(0x086B54))
        .text_size(theme::ts(34.))
        .line_height(theme::ts(34.))
        .font_weight(FontWeight::BOLD)
        .shadow_sm()
        .on_mouse_down(MouseButton::Left, on_down)
        .children(icon.map(|p| crate::icons::icon(p, 34., 0x086B54)))
        .child(label.into())
}

/// 暖色导航按钮（浅琥珀渐变 + 深琥珀文字；用于「上一步」等返回动作）
pub fn warm_button(
    icon: Option<&'static str>,
    label: impl Into<String>,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .gap_3()
        .min_h(s(84.))
        .rounded(s(20.))
        .border_2()
        .border_color(c(0xF2CD9B))
        .bg(linear_gradient(
            180.,
            linear_color_stop(c(0xFFF4E4), 0.),
            linear_color_stop(c(0xFFE8C9), 1.),
        ))
        .text_color(c(0x91560F))
        .text_size(theme::ts(34.))
        .line_height(theme::ts(34.))
        .font_weight(FontWeight::BOLD)
        .shadow_sm()
        .on_mouse_down(MouseButton::Left, on_down)
        .children(icon.map(|p| crate::icons::icon(p, 34., 0x91560F)))
        .child(label.into())
}

/// 禁用态按钮（降低不透明度）
pub fn disabled(primary: bool, label: impl Into<String>) -> Div {
    let mut base = if primary {
        primary_button(label, |_, _, _| {})
    } else {
        ghost_button(None, label, |_, _, _| {})
    };
    base = base.opacity(0.42).shadow_none();
    base
}

/// 数字键盘按键（normal / utility / pressed 三态，对应 .keypad-grid button）
pub fn keypad_button(
    label: impl Into<String>,
    pressed: bool,
    utility: bool,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let fg = if pressed {
        c(theme::INK)
    } else if utility {
        c(0x1F78D1)
    } else {
        c(theme::HEADER_TEXT)
    };

    let bg_fill: gpui::Fill = if pressed {
        c(theme::PRESSED).into()
    } else {
        gpui::linear_gradient(
            180.,
            linear_color_stop(c(0xFFFFFF), 0.),
            linear_color_stop(c(0xF8FBFF), 1.),
        )
        .into()
    };

    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap_2()
        .min_h(s(84.))
        .rounded(s(18.))
        .border_2()
        .border_color(c(if pressed { 0xC9D4E2 } else { theme::KEY_BORDER }))
        .bg(bg_fill)
        .text_color(fg)
        .text_2xl()
        .font_weight(FontWeight::BOLD)
        .shadow_sm()
        .on_mouse_down(MouseButton::Left, on_down)
        .child(label.into())
}

/// 键盘"退格"宽键（占两列，删除图标 + 文案）
pub fn keypad_backspace_button(
    pressed: bool,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let bg_fill: gpui::Fill = if pressed {
        c(theme::PRESSED).into()
    } else {
        gpui::linear_gradient(
            180.,
            linear_color_stop(c(0xFFFFFF), 0.),
            linear_color_stop(c(0xF8FBFF), 1.),
        )
        .into()
    };

    div().flex_none().w(gpui::relative(0.66)).child(
        div()
            .flex()
            .w_full()
            .items_center()
            .justify_center()
            .gap_2()
            .min_h(s(84.))
            .rounded(s(18.))
            .border_2()
            .border_color(c(if pressed { 0xC9D4E2 } else { theme::KEY_BORDER }))
            .bg(bg_fill)
            .shadow_sm()
            .on_mouse_down(MouseButton::Left, on_down)
            .child(crate::icons::icon(crate::icons::DELETE, 28., 0x1F78D1))
            .child(
                div()
                    .text_size(theme::ts(24.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(c(0x1F78D1))
                    .child("删除"),
            ),
    )
}

/// 步骤编号标题行左侧的 "01 ——"
pub fn step_number(text: &str) -> Div {
    div()
        .flex()
        .items_center()
        .child(
            div()
                .text_3xl()
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(c(theme::WARM))
                .child(text.to_string()),
        )
        .child(div().ml_3().w(s(36.)).h(s(1.)).bg(c(0xD6DFDE)))
}

/// 空心复选框（报告列表 / 全选共用）
pub fn checkbox(checked: bool, size: f32, disabled: bool) -> Div {
    let (border_color, bg) = if disabled {
        (c(0xD6E0EB), c(0xF2F5F9))
    } else if checked {
        (c(theme::TEAL), c(theme::TEAL))
    } else {
        (c(0xAFC2D8), c(0xF8FBFF))
    };

    div()
        .flex()
        .items_center()
        .justify_center()
        .w(s(size))
        .h(s(size))
        .rounded(s(9.))
        .border_2()
        .border_color(border_color)
        .bg(bg)
        .shadow_sm()
        .text_color(c(0xFFFFFF))
        .children(checked.then(|| crate::icons::icon(crate::icons::CHECK, size * 0.58, 0xFFFFFF)))
        .opacity(if disabled { 0.76 } else { 1.0 })
}

/// 勾选态为绿色的复选框（全选按钮等强调「完成」语义的场景）
pub fn checkbox_success(checked: bool, size: f32, disabled: bool) -> Div {
    let (border_color, bg) = if disabled {
        (c(0xD6E0EB), c(0xF2F5F9))
    } else if checked {
        (c(theme::SUCCESS), c(theme::SUCCESS))
    } else {
        (c(0xAFC2D8), c(0xF8FBFF))
    };

    div()
        .flex()
        .items_center()
        .justify_center()
        .w(s(size))
        .h(s(size))
        .rounded(s(9.))
        .border_2()
        .border_color(border_color)
        .bg(bg)
        .shadow_sm()
        .text_color(c(0xFFFFFF))
        .children(checked.then(|| crate::icons::icon(crate::icons::CHECK, size * 0.58, 0xFFFFFF)))
        .opacity(if disabled { 0.76 } else { 1.0 })
}

// ==== 设置页表单控件（对齐 Element Plus：switch / radio-button / stepper / 小按钮） ====

/// 开关（对应 el-switch，触控友好尺寸）
pub fn switch(
    checked: bool,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let track_color = if checked { theme::TEAL } else { 0xC0CCDA };
    let knob_size = if checked { s(22.) } else { s(16.) };
    let knob = div()
        .size(knob_size)
        .rounded_full()
        .bg(c(0xFFFFFF))
        .shadow_sm();
    let track = div()
        .flex()
        .items_center()
        .w(s(54.))
        .h(s(28.))
        .rounded_full()
        .bg(c(track_color))
        .px(s(3.))
        .on_mouse_down(MouseButton::Left, on_down);
    if checked {
        track.justify_end().child(knob)
    } else {
        track.justify_start().child(knob)
    }
}

/// 分段单选（对应 el-radio-button）
pub fn segmented(
    options: &[&str],
    active: usize,
    on_select: impl Fn(&usize, &mut Window, &mut App) + 'static,
) -> Div {
    let on_select = std::rc::Rc::new(on_select);
    let mut el = div().flex().gap_2();
    for (index, label) in options.iter().enumerate() {
        let selected = index == active;
        let callback = on_select.clone();
        el = el.child(
            div()
                .id(gpui::ElementId::Name(format!("seg-{index}-{label}").into()))
                .px(s(16.))
                .py(s(8.))
                .rounded(s(8.))
                .border_2()
                .border_color(c(if selected {
                    theme::TEAL
                } else {
                    theme::KEY_BORDER
                }))
                .bg(c(if selected { theme::TEAL_SOFT } else { 0xFFFFFF }))
                .text_size(s(14.))
                .font_weight(FontWeight::BOLD)
                .text_color(c(if selected {
                    theme::TEAL_DARK
                } else {
                    theme::MUTED
                }))
                .on_mouse_down(MouseButton::Left, move |_event, window, app| {
                    callback(&index, window, app)
                })
                .child(label.to_string()),
        );
    }
    el
}

/// 数字步进器（对应 el-input-number）
pub fn stepper(
    value: String,
    on_dec: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_inc: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .border_2()
        .border_color(c(theme::KEY_BORDER))
        .rounded(s(8.))
        .overflow_hidden()
        .child(
            div()
                .id("step-dec")
                .px(s(14.))
                .py(s(8.))
                .text_base()
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(c(theme::TEAL))
                .on_mouse_down(MouseButton::Left, on_dec)
                .child("−"),
        )
        .child(
            div()
                .min_w(s(56.))
                .px(s(12.))
                .py(s(8.))
                .text_center()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(c(theme::INK))
                .child(value),
        )
        .child(
            div()
                .id("step-inc")
                .px(s(14.))
                .py(s(8.))
                .text_base()
                .font_weight(FontWeight::EXTRA_BOLD)
                .text_color(c(theme::TEAL))
                .on_mouse_down(MouseButton::Left, on_inc)
                .child("+"),
        )
}

/// 滑动条轨道视觉（进度填充 + 圆形滑块；点击分段交互层由调用方叠加）
pub fn slider_track(ratio: f32) -> Div {
    let ratio = ratio.clamp(0.0, 1.0);
    div()
        .relative()
        .w_full()
        .h(s(36.))
        .flex()
        .items_center()
        // 轨道
        .child(
            div()
                .absolute()
                .left(s(0.))
                .right(s(0.))
                .top(s(14.))
                .h(s(8.))
                .rounded(s(4.))
                .bg(c(theme::PROGRESS_TRACK)),
        )
        // 已填充进度
        .child(
            div()
                .absolute()
                .left(s(0.))
                .top(s(14.))
                .w(gpui::relative(ratio))
                .h(s(8.))
                .rounded(s(4.))
                .bg(linear_gradient(
                    90.,
                    linear_color_stop(c(0x6CC3F5), 0.),
                    linear_color_stop(c(theme::TEAL), 1.),
                )),
        )
        // 滑块（圆心对准当前值位置）
        .child(
            div()
                .absolute()
                .left(gpui::relative(ratio))
                .top(s(6.))
                .ml(s(-12.))
                .size(s(24.))
                .rounded_full()
                .bg(c(0xFFFFFF))
                .border_2()
                .border_color(c(theme::TEAL))
                .shadow_sm(),
        )
}

/// 设置页小按钮（对应 el-button size=small，略加大便于触控）
pub fn small_button(
    label: impl Into<String>,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .px(s(16.))
        .py(s(9.))
        .rounded(s(9.))
        .border_1()
        .border_color(c(theme::KEY_BORDER))
        .bg(c(0xFFFFFF))
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(c(theme::CLOCK_DATE))
        .on_mouse_down(MouseButton::Left, on_down)
        .child(label.into())
}

/// 设置页小按钮（危险红色，对应 danger 按钮）
pub fn small_danger_button(
    label: impl Into<String>,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .px(s(16.))
        .py(s(9.))
        .rounded(s(9.))
        .bg(c(0xFDECEC))
        .text_sm()
        .font_weight(FontWeight::BOLD)
        .text_color(c(theme::DANGER))
        .on_mouse_down(MouseButton::Left, on_down)
        .child(label.into())
}

// ==== gpui-component 框架按钮封装（kiosk 视觉微调） ====
//
// 注意：框架 Button 的标签容器会按尺寸档位强制设置字号（button_text_size），
// 并把 .icon() 缩到 size*0.75——因此 kiosk 大字按钮的文案/图标必须用
// 显式样式的子元素（child）传入，而不是 .label()/.icon()。

/// 按钮文案子元素（显式字号，避免被框架 button_text_size 缩小）
pub fn kiosk_label(label: impl Into<String>, size: f32) -> Div {
    div()
        .flex_none()
        .text_size(theme::ts(size))
        .line_height(theme::ts(size))
        .font_weight(FontWeight::BOLD)
        .child(label.into())
}
/// 框架主按钮：蓝渐变 + 白字大按钮（对应 primary_button 的视觉，交互走 Button::on_click）
use gpui_component::button::ButtonVariants as _;
use gpui_component::Disableable as _;

pub fn fw_primary(id: &'static str, label: &'static str) -> gpui_component::button::Button {
    gpui_component::button::Button::new(id)
        .primary()
        .child(kiosk_label(label, 32.))
        .rounded(s(18.))
        .min_h(s(60.))
        .min_w(s(200.))
        .px(s(32.))
        .bg(linear_gradient(
            135.,
            linear_color_stop(c(theme::BUTTON_TOP), 0.),
            linear_color_stop(c(theme::BUTTON_BOTTOM), 1.),
        ))
        .text_size(theme::ts(32.))
        .shadow_md()
}

/// 框架薄荷绿导航按钮（返回首页）
pub fn fw_mint(id: &'static str, label: &'static str, icon: Option<&'static str>) -> gpui_component::button::Button {
    let btn = gpui_component::button::Button::new(id)
        .outline()
        .children(icon.map(|p| crate::icons::icon(p, 34., 0x086B54)))
        .child(kiosk_label(label, 34.))
        .rounded(s(20.))
        .min_h(s(84.))
        .min_w(s(220.))
        .px(s(30.))
        .border_color(c(0x9BDCC7))
        .bg(linear_gradient(
            180.,
            linear_color_stop(c(0xE4F8F0), 0.),
            linear_color_stop(c(0xC9EFE1), 1.),
        ))
        .text_color(c(0x086B54))
        .text_size(theme::ts(34.))
        .shadow_sm();
    btn
}

/// 框架暖色导航按钮（上一步）
pub fn fw_warm(id: &'static str, label: &'static str, icon: Option<&'static str>) -> gpui_component::button::Button {
    let btn = gpui_component::button::Button::new(id)
        .outline()
        .children(icon.map(|p| crate::icons::icon(p, 34., 0x91560F)))
        .child(kiosk_label(label, 34.))
        .rounded(s(20.))
        .min_h(s(84.))
        .min_w(s(220.))
        .px(s(30.))
        .border_color(c(0xF2CD9B))
        .bg(linear_gradient(
            180.,
            linear_color_stop(c(0xFFF4E4), 0.),
            linear_color_stop(c(0xFFE8C9), 1.),
        ))
        .text_color(c(0x91560F))
        .text_size(theme::ts(34.))
        .shadow_sm();
    btn
}

/// 框架禁用主按钮（打印中 / 未选中时）
pub fn fw_disabled(id: &'static str, label: &'static str) -> gpui_component::button::Button {
    gpui_component::button::Button::new(id)
        .primary()
        .child(kiosk_label(label, 28.))
        .disabled(true)
        .rounded(s(18.))
        .min_h(s(58.))
        .min_w(s(230.))
        .px(s(32.))
        .text_size(theme::ts(28.))
}
