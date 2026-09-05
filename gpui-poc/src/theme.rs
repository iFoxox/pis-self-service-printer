//! 主题常量（对应 renderer/src/assets/main.css 的浅色医疗蓝 Kiosk 风格）
//!
//! 含视口自适应缩放：所有 UI 尺寸以 1920×1080 为设计基准书写，
//! 渲染时按窗口视口计算缩放系数，`s()` 把设计稿像素换算为实际像素，
//! 使 1366×768 / 1600×900 / 2K / 4K 等常见分辨率等比自适应。
//!
//! ⚠️ gpui 0.2.2 Windows 的文字渲染按显示器 DPI 额外放大（盒子按布局像素
//! 1:1 绘制，文字字形却 ×DPI），因此文字尺寸须用 `ts()`（除以 DPI 补偿），
//! 并同步调小窗口 rem 基准使 text_sm / text_xl 等相对字号保持一致。

use std::sync::atomic::{AtomicU32, Ordering};

use gpui::{Pixels, Window, px, rgb};

/// 设计基准宽度
const BASE_WIDTH: f32 = 1920.;
/// 设计基准高度
const BASE_HEIGHT: f32 = 1080.;
/// 缩放系数上下限（过小文字不可读，过大布局失衡）
const SCALE_MIN: f32 = 0.6;
const SCALE_MAX: f32 = 1.8;

static UI_SCALE_X100: AtomicU32 = AtomicU32::new(100);
static VIEWPORT_W: AtomicU32 = AtomicU32::new(1920);
static VIEWPORT_H: AtomicU32 = AtomicU32::new(1080);

/// 每帧渲染前调用：按窗口视口尺寸更新缩放系数
///
/// ⚠️ 实测修正（4K@150%）：gpui 0.2.2 Windows `viewport_size()` 返回的
/// `logical_size` 已经是正确的逻辑尺寸（物理 ÷ DPI），渲染时再把布局
/// 像素 ×DPI 绘制——因此视口直接使用即可，**不能再除以 DPI**。
/// 旧版注释声称 Windows 需除以 DPI 得到"可见逻辑视口"，属误诊：
/// 那会使内容只渲染在屏幕左上 1/DPI² 区域（2560×1440 物理 @4K）。
pub fn update_ui_scale(window: &Window) {
    let viewport = window.viewport_size();
    let dpi = window.scale_factor().max(0.01);
    // 平台一致：viewport_size() 在 Windows/macOS 均为逻辑尺寸，直接使用
    let (vw, vh) = (f32::from(viewport.width), f32::from(viewport.height));
    VIEWPORT_W.store(vw.round() as u32, Ordering::Relaxed);
    VIEWPORT_H.store(vh.round() as u32, Ordering::Relaxed);

    let scale = (vw / BASE_WIDTH)
        .min(vh / BASE_HEIGHT)
        .clamp(SCALE_MIN, SCALE_MAX);
    let prev = UI_SCALE_X100.swap((scale * 100.).round() as u32, Ordering::Relaxed);
    if prev != UI_SCALE_X100.load(Ordering::Relaxed) {
        crate::domain::log::info(
            "ui",
            &format!("可见视口 {vw:.0}×{vh:.0}（DPI {dpi:.2}），缩放系数 {scale:.2}"),
        );
    }
}

/// 可见逻辑视口（物理视口 / DPI），根容器应显式使用该尺寸
pub fn viewport_logical() -> (f32, f32) {
    (
        VIEWPORT_W.load(Ordering::Relaxed) as f32,
        VIEWPORT_H.load(Ordering::Relaxed) as f32,
    )
}

/// 当前缩放系数（0.6 ~ 1.8）
pub fn ui_scale() -> f32 {
    UI_SCALE_X100.load(Ordering::Relaxed) as f32 / 100.
}

/// 设计稿像素 → 实际布局像素（盒子/间距/图片/文字统一用）
pub fn s(value: f32) -> Pixels {
    px(value * ui_scale())
}

/// 设计稿像素 → 文字尺寸（与 s() 一致；保留别名以表达语义）
pub fn ts(value: f32) -> Pixels {
    s(value)
}

/// rem 基准像素值：使 text_sm / text_xl 等相对字号与设计稿一致
/// （默认 16px 是 96dpi 基准，按缩放系数换算）
pub fn rem_base() -> Pixels {
    px(16. * ui_scale())
}

// ==== 基础色板（与 main.css :root 变量一致） ====
pub const INK: u32 = 0x1F2D3D;
pub const MUTED: u32 = 0x6B7B8D;
pub const TEAL: u32 = 0x409EFF;
pub const TEAL_DARK: u32 = 0x337ECC;
pub const TEAL_SOFT: u32 = 0xECF5FF;
pub const CREAM: u32 = 0xF3F7FC;
pub const WARM: u32 = 0xF4A34C;
pub const LINE: u32 = 0xD9E6F5;

// ==== 扩展色（取自 main.css 各处字面值） ====
pub const TITLE: u32 = 0x25364D; // home h1
pub const HEADER_TEXT: u32 = 0x17283D; // clock / keypad / 标题
pub const CLOCK_DATE: u32 = 0x526B87; // clock 日期 / action-card 描述
pub const SUB_TEXT: u32 = 0x60758B; // touch-hint / 密码错误行
pub const NOTICE_TEXT: u32 = 0x24496F; // 首页报告提示
pub const GUIDE_TEXT: u32 = 0x536B84; // 操作流程标签
pub const KEY_BORDER: u32 = 0xD5E3F1; // 键盘按键描边
pub const INPUT_BORDER: u32 = 0xD8E5E3; // 大输入框描边
pub const INPUT_BG: u32 = 0xF9FBFB; // 大输入框底色
pub const PLACEHOLDER: u32 = 0x5E7388; // 输入框占位（对 #F9FBFB 底 ≥4.5:1 对比度）
pub const PRESSED: u32 = 0xE2E7EE; // 键盘按下反馈
pub const ROW_BORDER: u32 = 0xE2E8F0; // 表格行分隔线
pub const ROW_DISABLED: u32 = 0xF7F9FC; // 不可打印行底色
pub const SELECTED_BG: u32 = 0xEDF6FF; // 选中行底色
pub const STATUS_PILL_PRINTED: u32 = 0xEDF1F5; // 已打印徽章
pub const CHECK_BORDER: u32 = 0xAEBDCD; // 复选框描边
pub const CARD_LINE: u32 = 0xCFE3FA; // 操作流程卡描边
pub const NOTICE_BORDER: u32 = 0xC7E1FB; // 报告提示描边
pub const ICON_BG: u32 = 0xFDECEC; // 错误弹窗图标底
pub const DANGER: u32 = 0xE4564F;
pub const DANGER_DARK: u32 = 0xD24640;
#[allow(dead_code)]
pub const SUCCESS: u32 = 0x67C23A;
pub const PROGRESS_TRACK: u32 = 0xE4EEF9; // 加载进度条底

// 渐变色端点
pub const BUTTON_TOP: u32 = 0x409EFF; // 主按钮渐变起
pub const BUTTON_BOTTOM: u32 = 0x337ECC; // 主按钮渐变止
pub const PRINTER_TOP: u32 = 0x58AAFF; // 打印机机身渐变
pub const PRINTER_BOTTOM: u32 = 0x337ECC;
#[allow(dead_code)]
pub const HEADER_GRAD_END: u32 = 0x3B8FE8; // 表头渐变中间值

/// 快捷取色
pub fn c(value: u32) -> gpui::Rgba {
    rgb(value)
}

// ==== gpui-component 主题定制（把本文件色板映射进框架 Theme） ====

/// 初始化 gpui-component 主题：浅色模式 + 医疗蓝色板 + 雅黑字体。
/// 必须在 gpui_component::init() 之后、窗口创建之前调用。
pub fn init_component_theme(cx: &mut gpui::App) {
    use gpui_component::theme::{Theme, ThemeMode};

    Theme::change(ThemeMode::Light, None, cx);
    let theme = Theme::global_mut(cx);

    // 色板映射（组件通过 cx.theme() 读取这些颜色）
    // 背景=纯白：框架 Input 的底色、Dialog/卡片底色都取自它
    // （页面自身的浅色渐变由根容器自绘，不受影响）
    theme.colors.background = c(0xFFFFFF).into();
    theme.colors.foreground = c(INK).into();
    theme.colors.primary = c(BUTTON_TOP).into();
    theme.colors.primary_hover = c(0x3393F0).into();
    theme.colors.primary_active = c(BUTTON_BOTTOM).into();
    theme.colors.primary_foreground = c(0xFFFFFF).into();
    theme.colors.border = c(LINE).into();
    // theme.input 在框架里是「输入框描边色」，不是底色
    theme.colors.input = c(INPUT_BORDER).into();
    theme.colors.slider_bar = c(BUTTON_TOP).into();
    theme.colors.slider_thumb = c(0xFFFFFF).into();
    // 输入框光标（闪烁竖线）与主题蓝一致
    theme.colors.caret = c(BUTTON_TOP).into();
    theme.colors.danger = c(DANGER).into();
    theme.colors.danger_foreground = c(0xFFFFFF).into();
    theme.colors.secondary = c(TEAL_SOFT).into();
    theme.colors.secondary_foreground = c(TEAL_DARK).into();
    theme.colors.muted = c(0xEDF2F9).into();
    theme.colors.muted_foreground = c(MUTED).into();

    // 形态：圆角 / 阴影 / 字体 / 滚动条常显（触屏自助终端）
    theme.radius = px(10.);
    theme.shadow = true;
    theme.font_family = "Microsoft YaHei".into();
    theme.font_size = px(16.);
    theme.scrollbar_show = gpui_component::scroll::ScrollbarShow::Always;
}

// ==== 兼容别名（早期 PoC 常量名） ====
#[allow(dead_code)]
pub const BG: u32 = CREAM;
#[allow(dead_code)]
pub const CARD: u32 = 0xFFFFFF;
#[allow(dead_code)]
pub const PRIMARY: u32 = TEAL;
#[allow(dead_code)]
pub const PRIMARY_DARK: u32 = TEAL_DARK;
#[allow(dead_code)]
pub const PRIMARY_LIGHT: u32 = TEAL_SOFT;
#[allow(dead_code)]
pub const TEXT: u32 = INK;
#[allow(dead_code)]
pub const TEXT_MUTED: u32 = MUTED;
#[allow(dead_code)]
pub const ACCENT: u32 = WARM;
#[allow(dead_code)]
pub const BORDER: u32 = LINE;
