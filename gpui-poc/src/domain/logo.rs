//! 内置 Logo 预设注册表
//!
//! 新增内置 Logo：把图片放进 `resources/assets/logos/<hospital|operator>/`，
//! 在下方对应预设表追加一条即可（id 唯一）。设置页 Logo 行以下拉选择内置
//! 预设或外部文件，写入配置的 `hospitalLogoPreset` / `footerLogoPreset`。

/// 内置 Logo 预设
pub struct LogoPreset {
    /// 预设 ID（配置文件中存储的值，勿改动，否则存量配置失配）
    pub id: &'static str,
    /// 设置页下拉展示名
    pub label: &'static str,
    pub bytes: &'static [u8],
}

/// 下拉首项：内置占位图（配置值为空）
pub const SELECT_DEFAULT_LABEL: &str = "占位图（默认）";
/// 下拉末项：切换为外部文件模式
pub const SELECT_CUSTOM_LABEL: &str = "自定义图片（本地文件）";

/// 院徽预设（顶部，对应 hospitalLogoPreset）
pub const HOSPITAL_PRESETS: &[LogoPreset] = &[
    LogoPreset {
        id: "guiyi",
        label: "贵州医科大学附属医院",
        bytes: include_bytes!("../../resources/assets/logos/hospital/guiyi.png"),
    },
];

/// 运营方 Logo 预设（底部，对应 footerLogoPreset）
pub const OPERATOR_PRESETS: &[LogoPreset] = &[
    LogoPreset {
        id: "huayin",
        label: "华银康集团",
        bytes: include_bytes!("../../resources/assets/logos/operator/huayin.png"),
    },
];

/// 按 ID 查找预设（id 为配置中存储的值）
pub fn find(presets: &'static [LogoPreset], id: &str) -> Option<&'static LogoPreset> {
    let id = id.trim();
    presets.iter().find(|p| p.id == id)
}

/// 按 label 查找预设（下拉回填用）
fn find_by_label(presets: &'static [LogoPreset], label: &str) -> Option<&'static LogoPreset> {
    presets.iter().find(|p| p.label == label)
}

/// 下拉选项列表：占位图（默认）→ 各内置预设 → 自定义文件
pub fn select_items(presets: &'static [LogoPreset]) -> Vec<String> {
    let mut items = vec![SELECT_DEFAULT_LABEL.to_string()];
    items.extend(presets.iter().map(|p| p.label.to_string()));
    items.push(SELECT_CUSTOM_LABEL.to_string());
    items
}

/// 当前配置对应的下拉选中行（0 = 占位图；末行 = 自定义文件）
pub fn select_row(presets: &'static [LogoPreset], preset_id: &str, custom_name: &str) -> usize {
    let id = preset_id.trim();
    if !id.is_empty() {
        if let Some(pos) = presets.iter().position(|p| p.id == id) {
            return pos + 1;
        }
    }
    if custom_name.trim().is_empty() {
        0
    } else {
        presets.len() + 1
    }
}

/// 下拉选中项的写回语义
pub enum LogoSelection {
    /// 占位图（清空预设；自定义文件保留但停用）
    Default,
    /// 内置预设（携带预设 ID）
    Preset(&'static str),
    /// 自定义文件（清空预设，保留已导入的文件名）
    Custom,
}

/// 按下拉选中标签解析写回语义
pub fn apply_selection(presets: &'static [LogoPreset], label: &str) -> LogoSelection {
    if label == SELECT_CUSTOM_LABEL {
        return LogoSelection::Custom;
    }
    match find_by_label(presets, label) {
        Some(p) => LogoSelection::Preset(p.id),
        None => LogoSelection::Default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_ids_are_unique() {
        for (presets, kind) in [(HOSPITAL_PRESETS, "hospital"), (OPERATOR_PRESETS, "operator")] {
            let mut ids: Vec<_> = presets.iter().map(|p| p.id).collect();
            ids.sort_unstable();
            let len = ids.len();
            ids.dedup();
            assert_eq!(ids.len(), len, "{kind} 预设 id 重复");
        }
    }

    #[test]
    fn select_row_maps_configuration() {
        assert_eq!(select_row(HOSPITAL_PRESETS, "", ""), 0);
        assert_eq!(select_row(HOSPITAL_PRESETS, "guiyi", ""), 1);
        // 预设优先于自定义文件
        assert_eq!(select_row(HOSPITAL_PRESETS, "guiyi", "a.png"), 1);
        assert_eq!(select_row(HOSPITAL_PRESETS, "", "a.png"), 2);
        // 未知 id 回落：有自定义文件走自定义，否则占位
        assert_eq!(select_row(HOSPITAL_PRESETS, "ghost", "a.png"), 2);
        assert_eq!(select_row(HOSPITAL_PRESETS, "ghost", ""), 0);
    }

    #[test]
    fn apply_selection_maps_labels() {
        assert!(matches!(
            apply_selection(HOSPITAL_PRESETS, SELECT_DEFAULT_LABEL),
            LogoSelection::Default
        ));
        assert!(matches!(
            apply_selection(HOSPITAL_PRESETS, SELECT_CUSTOM_LABEL),
            LogoSelection::Custom
        ));
        match apply_selection(HOSPITAL_PRESETS, "贵州医科大学附属医院") {
            LogoSelection::Preset(id) => assert_eq!(id, "guiyi"),
            _ => panic!("应解析为预设"),
        }
    }
}
