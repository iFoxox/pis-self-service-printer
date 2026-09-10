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

/// 正式配置与程序分离；debug 配置隔离，避免开发覆盖现场设置。
pub fn config_file_path() -> PathBuf {
    let root = app_data_dir();
    root.join(if cfg!(debug_assertions) {
        "debug-config"
    } else {
        "config"
    })
    .join("app-config.json")
}

pub fn legacy_config_paths() -> Vec<PathBuf> {
    if cfg!(debug_assertions) {
        return vec![];
    }
    let mut paths = vec![];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            paths.push(dir.join("config/app-config.json"));
        }
    }
    paths.push(app_data_dir().join("app-config.json"));
    paths
}

/// 模板仅在无用户配置时初始化使用。
pub fn bundled_template_path() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        return Some(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../resources/config/app-config-dev.json"),
        );
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("config/app-config.example.json")))
}

/// 使用系统文件管理器打开目录；命令参数直接传递，支持空格和中文路径。
pub fn open_directory(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(target_os = "windows")]
    let program = "explorer.exe";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program).arg(path).spawn()?;
    Ok(())
}
