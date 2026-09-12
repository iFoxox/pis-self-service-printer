//! 配置存储（平移自 src-tauri/src/config.rs，移除 Tauri IPC）
//!
//! 用户配置优先；结构升级前备份，校验成功后原子保存。

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, Write};

pub const CURRENT_CONFIG_VERSION: u32 = 1;
use std::path::PathBuf;

pub const DEFAULT_REPORT_NOTICE: &str = "只能查询到180天以内的报告，超出时间请到窗口询问工作人员";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServiceConfig {
    pub base_url: String,
    pub org_id: String,
    pub api_key: String,
    pub secret_key: String,
    pub request_timeout_seconds: u32,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:8080".into(),
            org_id: String::new(),
            api_key: String::new(),
            secret_key: String::new(),
            request_timeout_seconds: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrintConfig {
    pub default_printer: String,
    pub paper: String,
    pub orientation: String,
    pub allow_reprint: bool,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            default_printer: String::new(),
            paper: "A4".into(),
            orientation: "portrait".into(),
            allow_reprint: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TerminalConfig {
    pub fullscreen: bool,
    pub idle_timeout_seconds: u32,
    pub exit_password: String,
    pub minimize_password: String,
    pub log_password: String,
    pub report_notice: String,
    pub input_hint: String,
    pub log_retention_days: u32,
    pub log_dir: String,
    #[serde(default)]
    pub api_logging_enabled: bool,
    pub auto_select_reports: bool,
    pub voice_enabled: bool,
    pub voice_volume: u32,
    pub voice_rate: f64,
    pub click_enabled: bool,
    pub click_volume: u32,
    pub voice_input: String,
    pub voice_reports_found: String,
    pub voice_print_complete: String,
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            fullscreen: true,
            idle_timeout_seconds: 120,
            exit_password: "1200".into(),
            minimize_password: "9900".into(),
            log_password: "1600".into(),
            report_notice: DEFAULT_REPORT_NOTICE.into(),
            input_hint: "输入登记号/病历号".into(),
            log_retention_days: 30,
            log_dir: String::new(),
            api_logging_enabled: false,
            auto_select_reports: false,
            voice_enabled: true,
            voice_volume: 80,
            voice_rate: 0.9,
            click_enabled: true,
            click_volume: 70,
            voice_input: String::new(),
            voice_reports_found: String::new(),
            voice_print_complete: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub config_version: u32,
    /// 保留旧版模板指纹以兼容历史配置；不再用于覆盖用户设置。
    #[serde(default)]
    pub applied_template_version: String,
    pub hospital_name: String,
    pub hospital_logo: String,
    /// 顶部院徽内置预设 ID（非空时优先于 hospitalLogo 自定义文件；空 = 占位图）
    #[serde(default)]
    pub hospital_logo_preset: String,
    /// 底部运营方 Logo（空 = 使用内置默认 Logo）
    #[serde(default)]
    pub footer_logo: String,
    /// 底部运营方 Logo 内置预设 ID（优先级同上）
    #[serde(default)]
    pub footer_logo_preset: String,
    pub terminal_code: String,
    pub service: ServiceConfig,
    pub print: PrintConfig,
    pub terminal: TerminalConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: CURRENT_CONFIG_VERSION,
            applied_template_version: String::new(),
            hospital_name: "病理报告自助服务".into(),
            hospital_logo: String::new(),
            hospital_logo_preset: String::new(),
            footer_logo: String::new(),
            footer_logo_preset: String::new(),
            terminal_code: "PIS-KIOSK-001".into(),
            service: ServiceConfig::default(),
            print: PrintConfig::default(),
            terminal: TerminalConfig::default(),
        }
    }
}

/// 深度合并 JSON：defaults 中已有的键被 overlay 覆盖，缺失键保留默认值。
fn merge_json(defaults: &Value, overlay: &Value) -> Value {
    match (defaults, overlay) {
        (Value::Object(base), Value::Object(extra)) => {
            let mut out = base.clone();
            for (key, value) in extra {
                out.insert(
                    key.clone(),
                    match out.get(key) {
                        Some(existing) => merge_json(existing, value),
                        None => value.clone(),
                    },
                );
            }
            Value::Object(out)
        }
        _ => overlay.clone(),
    }
}

// ==== ConfigStore ====

/// 配置存储：内部持锁，支持读写文件
#[derive(Clone)]
pub struct ConfigStore {
    path: PathBuf,
    migration_warning: Option<String>,
    inner: std::sync::Arc<std::sync::Mutex<AppConfig>>,
}

impl ConfigStore {
    /// 仅初始化时使用模板；已有用户值始终优先。失败时不修改原文件。
    pub fn load(
        path: PathBuf,
        bundled: Option<PathBuf>,
        legacy: &[PathBuf],
    ) -> anyhow::Result<Self> {
        let source = if path.try_exists()? {
            Some(path.clone())
        } else {
            let mut found = None;
            for candidate in legacy {
                if candidate.try_exists()? {
                    found = Some(candidate.clone());
                    break;
                }
            }
            found
        };
        let defaults = serde_json::to_value(AppConfig::default())?;
        let value = if let Some(source) = &source {
            let text = std::fs::read_to_string(source)
                .with_context(|| format!("无法读取配置 {}", source.display()))?;
            serde_json::from_str::<Value>(&text)
                .with_context(|| format!("配置 JSON 无效 {}", source.display()))?
        } else if let Some(template) = bundled {
            serde_json::from_slice::<Value>(&std::fs::read(template)?)?
        } else {
            json!({})
        };
        let migrated = migrate_config(value.clone())?;
        let merged = merge_json(&defaults, &migrated);
        let config: AppConfig = serde_json::from_value(merged)?;
        let needs_save = source.as_ref() != Some(&path) || migrated != value;
        if needs_save {
            if let Some(source) = &source {
                backup_before_change(source, &path)?;
            }
            write_config(&path, &config)?;
        }
        // 只有新配置落盘后才标记旧文件；标记失败不回滚已成功的迁移。
        let migration_warning = source.as_ref().filter(|source| *source != &path)
            .and_then(|source| retire_legacy_config(source, &path).err())
            .map(|error| format!("配置已迁移到 {}，但旧文件未能完整标记：{error}。请只编辑新配置，退出程序后修改并重新启动。", path.display()));
        if let Some(warning) = &migration_warning {
            crate::domain::log::warn("config", warning);
        }
        Ok(Self {
            migration_warning,
            path,
            inner: std::sync::Arc::new(std::sync::Mutex::new(config)),
        })
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn migration_warning(&self) -> Option<&str> {
        self.migration_warning.as_deref()
    }

    pub fn get(&self) -> AppConfig {
        self.inner.lock().unwrap().clone()
    }

    /// 持久化成功后才更新内存，避免 UI 与磁盘状态不一致。
    pub fn set(&self, mut config: AppConfig) -> io::Result<()> {
        let mut current = self
            .inner
            .lock()
            .map_err(|e| io::Error::other(e.to_string()))?;
        config.config_version = CURRENT_CONFIG_VERSION;
        config.applied_template_version = current.applied_template_version.clone();
        backup_before_change(&self.path, &self.path)?;
        write_config(&self.path, &config)?;
        *current = config;
        Ok(())
    }
}

/// 保留已迁移文件供人工回退；绝不覆盖已有归档。
fn retire_legacy_config(source: &std::path::Path, target: &std::path::Path) -> io::Result<()> {
    let archive = source.with_file_name("app-config.migrated.json");
    if archive.try_exists()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("归档已存在 {}", archive.display()),
        ));
    }
    let notice = source.with_file_name("config-migration.txt");
    std::fs::write(
        &notice,
        format!(
            "旧配置已迁移。实际配置文件：\r\n{}\r\n\r\napp-config.migrated.json 为历史归档，修改它不会生效。\r\n请使用安装目录的“打开配置目录”入口，或设置页的“打开配置目录”按钮。\r\n先退出终端，再编辑真实配置；保存后重新启动生效。\r\n请使用运行终端的同一 Windows 账号。\r\n",
            target.display()
        ),
    )?;
    // 复制到不覆盖的归档后删除旧名称；也支持不提供硬链接的文件系统。
    let mut copy = tempfile::NamedTempFile::new_in(source.parent().expect("旧配置目录"))?;
    copy.write_all(&std::fs::read(source)?)?;
    copy.as_file().sync_all()?;
    copy.persist_noclobber(&archive)
        .map_err(|error| error.error)?;
    std::fs::remove_file(source)?;
    Ok(())
}

/// 当前 v1 没有需要改名的字段。以后在这里逐版本添加显式转换。
fn migrate_config(mut value: Value) -> anyhow::Result<Value> {
    anyhow::ensure!(value.is_object(), "配置必须是 JSON 对象");
    let version = match value.get("configVersion") {
        None => 1,
        Some(v) => v
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("configVersion 必须为正整数"))?,
    };
    anyhow::ensure!(
        version >= 1 && version <= CURRENT_CONFIG_VERSION as u64,
        "不支持配置版本 {version}，当前程序支持版本 {CURRENT_CONFIG_VERSION}；请使用兼容程序或恢复升级前备份"
    );
    value["configVersion"] = json!(CURRENT_CONFIG_VERSION);
    Ok(value)
}

/// 不轮转的升级/保存前快照，避免被周期备份清理。
fn backup_before_change(path: &std::path::Path, target: &std::path::Path) -> io::Result<()> {
    if !path.try_exists()? {
        return Ok(());
    }
    let dir = target
        .parent()
        .ok_or_else(|| io::Error::other("配置缺少父目录"))?
        .join("config-history");
    std::fs::create_dir_all(&dir)?;
    let mut backup = tempfile::Builder::new()
        .prefix("app-config-")
        .suffix(".json")
        .tempfile_in(dir)?;
    backup.write_all(&std::fs::read(path)?)?;
    backup.as_file().sync_all()?;
    backup.keep().map_err(|e| e.error)?;
    Ok(())
}

fn write_config(path: &std::path::Path, config: &AppConfig) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("配置缺少父目录"))?;
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer_pretty(&mut file, config)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}

/// 配置文件定时备份。
///
/// 规则：配置内容与最近一份备份不同时才新增一份（按字节比较，
/// 效果等同 MD5 比对且无碰撞风险）；滚动保留最多 `max_backups` 份
/// （超出时从最旧开始删除）。备份目录位于 %APPDATA%（安装目录之外，
/// 重装/升级不会触碰）。
///
/// 文件名 app-config-YYYYmmdd-HHMMSS.json，按文件名排序即按时间排序。
pub fn backup_config_file(config_path: &std::path::Path, max_backups: usize) {
    let Ok(content) = std::fs::read(config_path) else {
        return; // 配置文件尚不存在（首次运行前）
    };
    let backup_dir = crate::paths::app_data_dir().join("config-backups");
    if std::fs::create_dir_all(&backup_dir).is_err() {
        return;
    }

    let json_files = || -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&backup_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("app-config-") && n.ends_with(".json"))
                    .unwrap_or(false)
            })
            .collect();
        files.sort();
        files
    };

    // 与最近一份备份比较内容，相同则不重复备份
    let files = json_files();
    if let Some(latest) = files.last() {
        if std::fs::read(latest).is_ok_and(|prev| prev == content) {
            return;
        }
    }

    let stamp = chrono::Local::now().format("app-config-%Y%m%d-%H%M%S.json");
    let dest = backup_dir.join(stamp.to_string());
    match std::fs::write(&dest, &content) {
        Ok(()) => crate::domain::log::info("config", &format!("配置已备份：{}", dest.display())),
        Err(e) => {
            crate::domain::log::warn("config", &format!("配置备份失败: {e}"));
            return;
        }
    }

    // 滚动清理：最多保留 max_backups 份
    let mut files = json_files();
    while files.len() > max_backups {
        let Some(oldest) = files.first() else { break };
        let _ = std::fs::remove_file(oldest);
        files.remove(0);
    }
}

#[cfg(test)]
mod upgrade_tests {
    use super::*;

    #[test]
    fn upgrade_preserves_site_values_and_adds_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app-config.json");
        let original = br#"{"configVersion":1,"hospitalName":"Site","service":{"baseUrl":"https://site","apiKey":"site-key"},"print":{"defaultPrinter":"Printer"}}"#;
        std::fs::write(&path, original).unwrap();
        let template = dir.path().join("template.json");
        std::fs::write(&template, br#"{"hospitalName":"New default"}"#).unwrap();
        let store = ConfigStore::load(path.clone(), Some(template), &[]).unwrap();
        assert_eq!(store.get().hospital_name, "Site");
        assert_eq!(store.get().service.api_key, "site-key");
        assert_eq!(store.get().print.default_printer, "Printer");
        assert_eq!(store.get().terminal.idle_timeout_seconds, 120);
        assert_eq!(std::fs::read(path).unwrap(), original);
    }

    #[test]
    fn legacy_migration_is_backed_up_and_runs_once() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("legacy.json");
        let path = dir.path().join("data/app-config.json");
        let original = br#"{"hospitalName":"Legacy"}"#;
        std::fs::write(&legacy, original).unwrap();
        let store = ConfigStore::load(path.clone(), None, &[legacy.clone()]).unwrap();
        assert_eq!(store.get().hospital_name, "Legacy");
        assert!(!legacy.exists());
        let notice =
            std::fs::read_to_string(legacy.with_file_name("config-migration.txt")).unwrap();
        assert!(notice.contains(&path.display().to_string()));
        assert!(notice.contains("重新启动"));
        assert_eq!(
            std::fs::read(legacy.with_file_name("app-config.migrated.json")).unwrap(),
            original
        );
        let backup = std::fs::read_dir(path.parent().unwrap().join("config-history"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read(backup).unwrap(), original);
        std::fs::write(&legacy, b"broken").unwrap();
        assert_eq!(
            ConfigStore::load(path, None, &[legacy])
                .unwrap()
                .get()
                .hospital_name,
            "Legacy"
        );
    }

    #[test]
    fn failed_migration_keeps_legacy_and_no_archive() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("app-config.json");
        let target = dir.path().join("data/app-config.json");
        std::fs::write(&source, b"invalid json").unwrap();
        assert!(ConfigStore::load(target.clone(), None, &[source.clone()]).is_err());
        assert_eq!(std::fs::read(&source).unwrap(), b"invalid json");
        assert!(!source.with_file_name("app-config.migrated.json").exists());
        assert!(!target.exists());
        // Valid source but blocked target directory must also leave the source untouched.
        std::fs::write(&source, b"{}").unwrap();
        std::fs::write(target.parent().unwrap(), b"blocked").unwrap();
        assert!(ConfigStore::load(target, None, &[source.clone()]).is_err());
        assert_eq!(std::fs::read(source).unwrap(), b"{}");
    }

    #[test]
    fn archive_collision_warns_without_overwriting_either_file() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("app-config.json");
        let archive = source.with_file_name("app-config.migrated.json");
        let target = dir.path().join("data/app-config.json");
        std::fs::write(&source, br#"{"hospitalName":"Site"}"#).unwrap();
        std::fs::write(&archive, b"previous archive").unwrap();
        let store = ConfigStore::load(target.clone(), None, &[source.clone()]).unwrap();
        assert!(store.migration_warning().is_some());
        assert_eq!(store.path(), target);
        assert_eq!(store.get().hospital_name, "Site");
        assert!(source.exists());
        assert_eq!(std::fs::read(archive).unwrap(), b"previous archive");
    }

    #[test]
    fn invalid_and_future_configs_are_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app-config.json");
        for content in [
            "broken",
            "null",
            "{\"configVersion\":99}",
            "{\"service\":{\"baseUrl\":4}}",
        ] {
            std::fs::write(&path, content).unwrap();
            assert!(ConfigStore::load(path.clone(), None, &[]).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
        }
    }

    #[test]
    fn fresh_install_and_failed_save_keep_consistent_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data/app-config.json");
        let template = dir.path().join("template.json");
        std::fs::write(&template, br#"{"hospitalName":"Initial"}"#).unwrap();
        let store = ConfigStore::load(path.clone(), Some(template), &[]).unwrap();
        let mut config = store.get();
        config.hospital_name = "Saved".into();
        store.set(config).unwrap();
        assert_eq!(
            ConfigStore::load(path.clone(), None, &[])
                .unwrap()
                .get()
                .hospital_name,
            "Saved"
        );
        // A file blocks backup-directory creation: save must fail before touching live data.
        std::fs::remove_dir_all(path.parent().unwrap().join("config-history")).unwrap();
        std::fs::write(path.parent().unwrap().join("config-history"), b"block").unwrap();
        let before = std::fs::read(&path).unwrap();
        let mut config = store.get();
        config.hospital_name = "Unsaved".into();
        assert!(store.set(config).is_err());
        assert_eq!(store.get().hospital_name, "Saved");
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}
