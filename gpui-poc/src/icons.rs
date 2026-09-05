//! 内置图标资源：SVG 以 include_str! 内嵌，经 gpui AssetSource 供 `svg()` 元素渲染
//!
//! 图标样式对齐 Feather / Element Plus（24×24 viewBox，stroke = currentColor）。
//!
//! ⚠️ `svg().path()` 接收的是资源键（"icons/xxx.svg"），
//! 由 [IconAssets::load] 查得内嵌 SVG 文本再渲染——勿把 SVG 文本直接当 path 传入。

use gpui::prelude::*;
use gpui::{px, svg};
use std::borrow::Cow;

/// 图标资源键（svg().path() 按此键查找，勿直接传 SVG 文本）
pub const ARROW_LEFT: &str = "icons/arrow-left.svg";
pub const ARROW_RIGHT: &str = "icons/arrow-right.svg";
pub const SEARCH: &str = "icons/search.svg";
pub const CHECK: &str = "icons/check.svg";
pub const DELETE: &str = "icons/delete.svg";
pub const PRINTER: &str = "icons/printer.svg";
pub const DOCUMENT: &str = "icons/document.svg";
/// 报告单（剪贴板清单样式，与普通文档图标区分明显）
pub const REPORT: &str = "icons/report-clipboard.svg";
/// 圆弧加载圈（配合旋转动画做 spinner）
pub const SPINNER_ARC: &str = "icons/spinner-arc.svg";
pub const REFRESH: &str = "icons/refresh-right.svg";
pub const WARNING: &str = "icons/warning.svg";
pub const HOME: &str = "icons/home.svg";

const ARROW_LEFT_SVG: &str = include_str!("assets/icons/arrow-left.svg");
const ARROW_RIGHT_SVG: &str = include_str!("assets/icons/arrow-right.svg");
const SEARCH_SVG: &str = include_str!("assets/icons/search.svg");
const CHECK_SVG: &str = include_str!("assets/icons/check.svg");
const DELETE_SVG: &str = include_str!("assets/icons/delete.svg");
const PRINTER_SVG: &str = include_str!("assets/icons/printer.svg");
const DOCUMENT_SVG: &str = include_str!("assets/icons/document.svg");
const REPORT_SVG: &str = include_str!("assets/icons/report-clipboard.svg");
const SPINNER_ARC_SVG: &str = include_str!("assets/icons/spinner-arc.svg");
const REFRESH_SVG: &str = include_str!("assets/icons/refresh-right.svg");
const WARNING_SVG: &str = include_str!("assets/icons/warning.svg");
const HOME_SVG: &str = include_str!("assets/icons/home.svg");

const ALL: [(&str, &str); 12] = [
    (ARROW_LEFT, ARROW_LEFT_SVG),
    (ARROW_RIGHT, ARROW_RIGHT_SVG),
    (SEARCH, SEARCH_SVG),
    (CHECK, CHECK_SVG),
    (DELETE, DELETE_SVG),
    (PRINTER, PRINTER_SVG),
    (DOCUMENT, DOCUMENT_SVG),
    (REPORT, REPORT_SVG),
    (SPINNER_ARC, SPINNER_ARC_SVG),
    (REFRESH, REFRESH_SVG),
    (WARNING, WARNING_SVG),
    (HOME, HOME_SVG),
];

pub struct IconAssets;

impl gpui::AssetSource for IconAssets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(ALL
            .iter()
            .find(|(key, _)| *key == path)
            .map(|(_, text)| Cow::Borrowed(text.as_bytes())))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<gpui::SharedString>> {
        Ok(ALL
            .iter()
            .map(|(key, _)| *key)
            .filter(|p| p.starts_with(path.trim_end_matches('/')))
            .map(gpui::SharedString::from)
            .collect())
    }
}

/// 应用总资源源：自有图标优先，其余（gpui-component 的 IconName SVG）回退官方资源包
pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(data) = IconAssets.load(path)? {
            return Ok(Some(data));
        }
        gpui_component_assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<gpui::SharedString>> {
        let mut all = IconAssets.list(path)?;
        all.extend(gpui_component_assets::Assets.list(path)?);
        Ok(all)
    }
}

/// 渲染一个内嵌 SVG 图标（颜色跟随 currentColor）
pub fn icon(path: &'static str, size: f32, color: u32) -> gpui::AnyElement {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(crate::theme::c(color))
        .into_any_element()
}
