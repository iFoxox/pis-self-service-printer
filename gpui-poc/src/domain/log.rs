//! 日志模块（平移自 src-tauri/src/log.rs，路径解析改由 crate::paths 提供）
//!
//! 按天分文件存储：logs/app-YYYY-MM-DD.log，行格式：[时间] [LEVEL] [模块] 消息。

use std::io::Write;
use std::path::PathBuf;
use std::sync::RwLock;

const DATE_PATTERN: &str = "app-";
/// 日志最长保留天数
pub const MAX_RETENTION_DAYS: u32 = 30;

/// 配置的日志输出目录（None = 使用默认目录）
static LOG_DIR_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);

/// 默认日志目录：可执行文件所在目录 logs 下，失败回退应用数据目录
fn default_log_dir() -> PathBuf {
    let base = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(crate::paths::app_data_dir);
    base.join("logs")
}

/// 获取日志根目录
///
/// 默认/配置目录创建失败时（如安装到 Program Files 后普通用户无写权限），
/// 回退到 app_data_dir/logs（始终可写），避免日志静默丢失。
fn log_root() -> PathBuf {
    let dir = LOG_DIR_OVERRIDE
        .read()
        .ok()
        .and_then(|guard| guard.clone())
        .unwrap_or_else(default_log_dir);
    if std::fs::create_dir_all(&dir).is_ok() {
        return dir;
    }
    let fallback = crate::paths::app_data_dir().join("logs");
    std::fs::create_dir_all(&fallback).ok();
    fallback
}

/// 今日文件名 app-YYYY-MM-DD
fn today_filename() -> String {
    format!("app-{}", chrono::Local::now().format("%Y-%m-%d"))
}

/// 写入一条日志
pub fn write_log(level: &str, module: &str, message: &str) {
    let root = log_root();
    let now = chrono::Local::now();
    let file = root.join(format!("{}.log", today_filename()));
    let line = format!(
        "[{}] [{}] [{}] {}\n",
        now.format("%Y-%m-%dT%H:%M:%S%.3f%:z"),
        level,
        module,
        message
    );
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn info(module: &str, message: &str) {
    write_log("INFO", module, message);
}

pub fn warn(module: &str, message: &str) {
    write_log("WARN", module, message);
}

pub fn error(module: &str, message: &str) {
    write_log("ERROR", module, message);
}

/// 删除 N 天前的日志文件，返回删除的文件数
pub fn cleanup_logs(retention_days: u32) -> u32 {
    let root = log_root();
    let cutoff = chrono::Local::now().date_naive() - chrono::Duration::days(retention_days as i64);
    let mut removed = 0u32;
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stem) = name.strip_prefix(DATE_PATTERN) {
                if let Some(date_str) = stem.strip_suffix(".log") {
                    if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                        if date < cutoff {
                            let _ = std::fs::remove_file(entry.path());
                            removed += 1;
                        }
                    }
                }
            }
        }
    }
    removed
}

/// 根据配置应用日志目录与保留时长，并立即清理过期日志
pub fn apply_settings(log_dir: &str, retention_days: u32) {
    let trimmed = log_dir.trim();
    if let Ok(mut guard) = LOG_DIR_OVERRIDE.write() {
        *guard = if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        };
    }
    let days = retention_days.clamp(1, MAX_RETENTION_DAYS);
    let removed = cleanup_logs(days);
    if removed > 0 {
        info(
            "log",
            &format!("清理过期日志 {removed} 个文件（保留 {days} 天）"),
        );
    }
}

/// 列出日志文件名（按日期倒序，最新在前）
pub fn list_logs() -> Result<Vec<String>, String> {
    let root = log_root();
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .map_err(|e| format!("读取日志目录失败: {e}"))?
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.starts_with(DATE_PATTERN) && name.ends_with(".log"))
        .collect();
    names.sort();
    names.reverse();
    Ok(names)
}

/// 读取指定日志文件内容（仅允许 app-YYYY-MM-DD.log 形式的文件名）
pub fn read_log(name: &str) -> Result<String, String> {
    let file_name = std::path::Path::new(name)
        .file_name()
        .ok_or_else(|| "无效的文件名".to_string())?
        .to_string_lossy()
        .to_string();
    if file_name.is_empty() || file_name != name {
        return Err("无效的文件名".into());
    }
    if !file_name.starts_with(DATE_PATTERN) || !file_name.ends_with(".log") {
        return Err("无效的日志文件名".into());
    }
    let path = log_root().join(&file_name);
    std::fs::read_to_string(&path).map_err(|e| format!("读取日志失败: {e}"))
}
