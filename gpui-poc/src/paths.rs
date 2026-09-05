//! 路径解析（替代 tauri::path）
//!
//! 数据目录必须与 Tauri 版保持一致（`app_data_dir` / `com.pis.report.kiosk`），
//! 保证 PoC 可直接复用现有终端的 app-config.json 与语音、Logo 资源联调。

use std::path::PathBuf;

pub const APP_IDENTIFIER: &str = "com.pis.report.kiosk";

/// 应用数据目录（与 Tauri `app_data_dir` 行为一致）
pub fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
            .join(APP_IDENTIFIER)
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        home.join("Library")
            .join("Application Support")
            .join(APP_IDENTIFIER)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
            })
            .unwrap_or_default();
        base.join(APP_IDENTIFIER)
    }
}

/// 内置资源根目录：生产环境为 exe 同级目录（config/ 随包分发），
/// 开发环境回退到工程根 resources/（cargo run 时 target/debug 下没有打包资源）
pub fn resource_dir() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join("config").exists() {
                return Some(dir.to_path_buf());
            }
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("resources");
    if dev.exists() {
        return Some(dev);
    }
    None
}

/// 配置文件路径：安装目录 config\app-config.json（单文件方案，
/// 升级重装前需手动备份该文件）
pub fn config_file_path() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(app_data_dir);
    dir.join("config").join("app-config.json")
}

/// 内置配置模板：debug 开发联调读仓库 resources/config/app-config-dev.json
/// （dev 环境地址与测试凭据，模板变化时经托管更新覆盖本地调试配置）；
/// release 安装包已把 resources/config/app-config.json 放到安装目录 config/
/// 作为用户配置直接读写，无需模板，返回 None。
///
/// 不能复用 resource_dir()：debug 下首次运行后 target/debug/config/ 已存在，
/// resource_dir() 会指向 exe 目录而非仓库 resources/。
pub fn bundled_template_path() -> Option<PathBuf> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("resources")
        .join("config")
        .join("app-config-dev.json");
    path.exists().then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// debug 构建必须能定位仓库 dev 模板，且内容确为联调配置（非生产模板）
    #[test]
    fn debug_build_locates_dev_template() {
        let Some(template) = bundled_template_path() else {
            panic!("debug 构建应能定位 resources/config/app-config-dev.json");
        };
        let text = std::fs::read_to_string(&template).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["terminalCode"], "PIS-KIOSK-DEV");
    }

    /// 首次运行：dev 模板经托管更新合并进空配置，联调字段生效
    /// （baseUrl 断言取模板自身值，避免在源码中出现具体接口地址）
    #[test]
    fn fresh_load_applies_dev_template() {
        let Some(template) = bundled_template_path() else {
            panic!("debug 构建应能定位 dev 模板");
        };
        let text = std::fs::read_to_string(&template).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        let expected_base_url = value["service"]["baseUrl"]
            .as_str()
            .expect("dev 模板应含 service.baseUrl")
            .to_string();
        let dir = std::env::temp_dir().join(format!("pis-paths-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg_path = dir.join("app-config.json");
        let (store, info) =
            crate::domain::config::ConfigStore::load(cfg_path, Some(template), None);
        assert!(info.managed_update.is_some(), "首次加载应执行托管更新");
        let config = store.get();
        assert_eq!(config.terminal_code, "PIS-KIOSK-DEV");
        assert_eq!(config.service.base_url, expected_base_url);
        std::fs::remove_dir_all(&dir).ok();
    }
}
