//! 配置存储（平移自 src-tauri/src/config.rs，移除 Tauri IPC）
//!
//! 含托管式更新：内置模板内容指纹变化时，模板预置值自动覆盖用户配置
//! （仅覆盖模板中包含的字段），无需人工维护版本号。

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Digest;
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
            idle_timeout_seconds: 60,
            exit_password: "1200".into(),
            minimize_password: "9900".into(),
            log_password: "1600".into(),
            report_notice: DEFAULT_REPORT_NOTICE.into(),
            input_hint: "输入登记号/病历号".into(),
            log_retention_days: 30,
            log_dir: String::new(),
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
    /// 已应用的内置模板内容指纹（空 = 未应用；仅由后端维护，保存设置时传入的值会被忽略）
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
            config_version: 1,
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

/// 计算模板内容的稳定指纹（sha256 前 8 字节十六进制）。
/// 先解析为 JSON 再规范化序列化，键序/缩进/空格差异不影响指纹；
/// 模板内容一旦被修改，指纹必然变化，从而自动触发托管更新。
fn template_fingerprint(template: &Value) -> String {
    let canonical = serde_json::to_string(template).unwrap_or_default();
    let digest = sha2::Sha256::digest(canonical.as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// 备份损坏的用户配置文件，返回说明文字（备份失败返回 None）
fn backup_corrupted_config(path: &std::path::Path) -> Option<String> {
    let bak = path.with_extension("json.bak");
    if std::fs::copy(path, &bak).is_ok() {
        let name = bak
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        return Some(format!(
            "用户配置文件解析失败，已备份为 {name} 并回退默认配置"
        ));
    }
    None
}

/// 配置加载结果说明
#[derive(Debug, Default)]
pub struct ConfigLoadInfo {
    /// 用户配置损坏并被回退的说明（warn 级日志）
    pub warning: Option<String>,
    /// 本次启动应用的托管模板更新指纹（info 级日志）
    pub managed_update: Option<String>,
    /// 本次启动应用了本地覆盖文件（info 级日志）
    pub local_override_applied: bool,
}

// ==== ConfigStore ====

/// 配置存储：内部持锁，支持读写文件
#[derive(Clone)]
pub struct ConfigStore {
    path: PathBuf,
    inner: std::sync::Arc<std::sync::Mutex<AppConfig>>,
}

impl ConfigStore {
    /// 从文件加载配置。
    ///
    /// 常规路径：默认值 <- 打包内置模板 <- 用户配置文件 <- 本地覆盖文件
    /// （逐级覆盖，越靠后优先级越高）。
    ///
    /// 本地覆盖文件（app-config.local.json，与内置模板同目录）：
    /// 供运维在终端机上手工配置，**安装器升级不覆盖、卸载不删除**，
    /// 始终拥有最高优先级——需要"用户改了就以用户为准"的字段放这里，
    /// 而不是直接改内置模板（模板会在升级时被新包覆盖）。
    ///
    /// 托管式更新：模板内容指纹与用户配置记录的 appliedTemplateVersion 不一致
    /// （即模板被修改过，或用户配置尚未应用过模板）时，改为
    /// 默认值 <- 用户配置 <- 模板（模板预置值覆盖用户值），
    /// 应用后记录新指纹并立即持久化；此后模板未变则不再覆盖用户的后续修改。
    /// 本地覆盖文件不受托管更新影响，任何情况下最后应用。
    ///
    /// 用户配置文件损坏（JSON 解析失败或字段类型不兼容）时，
    /// 先备份为 <原名>.json.bak 再回退默认值。
    pub fn load(
        path: PathBuf,
        bundled: Option<PathBuf>,
        local_override: Option<PathBuf>,
    ) -> (Self, ConfigLoadInfo) {
        let mut info = ConfigLoadInfo::default();
        let defaults = serde_json::to_value(AppConfig::default()).unwrap_or(json!({}));

        // 内置模板
        let template: Option<Value> = bundled
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| serde_json::from_str(&text).ok());

        // 用户配置（损坏视为不存在，走备份回退流程）
        let user_value: Option<Value> = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(value) => Some(value),
                Err(_) => {
                    info.warning = backup_corrupted_config(&path);
                    None
                }
            },
            Err(_) => None,
        };

        // 本地覆盖文件（最高优先级；不存在或解析失败则忽略）
        let local_override_value: Option<Value> = local_override
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|text| serde_json::from_str(&text).ok());

        // 模板内容指纹与用户配置记录的指纹
        let template_fp = template.as_ref().map(template_fingerprint);
        let applied_fp = user_value
            .as_ref()
            .and_then(|u| u.get("appliedTemplateVersion"))
            .and_then(|v| v.as_str())
            .map(str::to_string);

        // 指纹不一致 = 模板被修改过（或用户配置尚未应用过模板）→ 执行托管更新
        let needs_apply = match (&template_fp, &applied_fp) {
            (Some(fp), applied) => applied.as_deref() != Some(fp.as_str()),
            (None, _) => false,
        };

        let mut base;
        if needs_apply {
            // 托管式更新：模板预置值覆盖用户配置（仅覆盖模板中包含的字段）
            base = defaults;
            if let Some(user) = &user_value {
                base = merge_json(&base, user);
            }
            if let Some(t) = &template {
                base = merge_json(&base, t);
            }
            if let Some(fp) = template_fp.as_deref() {
                if let Value::Object(map) = &mut base {
                    map.insert("appliedTemplateVersion".into(), json!(fp));
                }
            }
            info.managed_update = template_fp;
        } else {
            base = defaults;
            if let Some(t) = &template {
                base = merge_json(&base, t);
            }
            if let Some(user) = &user_value {
                base = merge_json(&base, user);
            }
        }

        // 本地覆盖文件最后应用：任何情况下优先级最高
        if let Some(local) = &local_override_value {
            base = merge_json(&base, local);
            info.local_override_applied = true;
        }

        let config = match serde_json::from_value::<AppConfig>(base) {
            Ok(config) => config,
            Err(_) => {
                if info.warning.is_none() {
                    info.warning = backup_corrupted_config(&path);
                }
                AppConfig::default()
            }
        };

        let store = Self {
            path,
            inner: std::sync::Arc::new(std::sync::Mutex::new(config)),
        };

        // 托管更新立即持久化：即使随后崩溃，下次启动也不会重复覆盖用户配置
        if info.managed_update.is_some() {
            let _ = store.save();
        }

        (store, info)
    }

    pub fn get(&self) -> AppConfig {
        self.inner.lock().unwrap().clone()
    }

    /// 更新配置并保存到磁盘；返回保存结果
    pub fn set(&self, config: AppConfig) -> std::io::Result<()> {
        let mut config = config;
        // appliedTemplateVersion 仅由后端维护，忽略调用方传入值，防止意外重置
        // （否则重置为空会导致下次启动重复执行模板托管更新、覆盖用户修改）
        config.applied_template_version =
            self.inner.lock().unwrap().applied_template_version.clone();
        *self.inner.lock().unwrap() = config;
        self.save()
    }
    /// 保存到磁盘；返回写入结果（失败时记录错误日志，供 UI 提示）
    pub fn save(&self) -> std::io::Result<()> {
        let config = self
            .inner
            .lock()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let json = serde_json::to_string_pretty(&*config).unwrap_or_default();
        let result = std::fs::write(&self.path, json);
        if let Err(e) = &result {
            crate::domain::log::error(
                "config",
                &format!("配置写入失败 {}: {e}", self.path.display()),
            );
        }
        result
    }
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
