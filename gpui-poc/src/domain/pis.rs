//! PIS 接口客户端（平移自 src-tauri/src/pis.rs，移除 Tauri AppHandle）
//!
//! - 签名：过滤 null 与 pisDataSignature，按 key ASCII 字典序拼接，
//!   以 Secret Key 执行 HMAC-SHA256 并输出 Base64
//! - 鉴权头：Pis-Api-Key
//! - 请求 / 响应：{ code, msg, data }，code === 0 视为成功

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use sha2::Sha256;

use super::config::AppConfig;
use super::log;
use super::report::ReportItem;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, serde::Deserialize)]
struct PisResponse<T> {
    code: i64,
    #[serde(default)]
    msg: Option<String>,
    #[serde(default)]
    data: Option<T>,
}

/// 校验接口配置是否完整
fn validate_config(config: &AppConfig) -> Result<(), String> {
    let mut missing = Vec::new();
    if config.service.base_url.trim().is_empty() {
        missing.push("接口地址");
    }
    if config.service.org_id.trim().is_empty() {
        missing.push("机构 ID");
    }
    if config.service.api_key.trim().is_empty() {
        missing.push("API Key");
    }
    if config.service.secret_key.trim().is_empty() {
        missing.push("Secret Key");
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!("终端尚未完成配置：{}", missing.join("、")))
    }
}

/// 计算请求体签名（与前端 buildSignString 逻辑一致）
fn sign_body(body: &mut Map<String, Value>, secret_key: &str) -> Result<String, String> {
    let mut pairs: Vec<(String, String)> = body
        .iter()
        .filter(|(key, value)| key.as_str() != "pisDataSignature" && !value.is_null())
        .map(|(key, value)| {
            let value_str = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            (key.clone(), value_str)
        })
        .collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let sign_string = pairs
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");

    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
        .map_err(|e| format!("签名初始化失败: {e}"))?;
    mac.update(sign_string.as_bytes());
    let signature = mac.finalize().into_bytes();
    Ok(STANDARD.encode(signature))
}

/// 发起签名 POST 请求并解析统一响应
/// 返回的错误为面向患者的友好提示（技术细节只写入日志）
pub(crate) async fn post<T: DeserializeOwned + Default>(
    config: &AppConfig,
    pathname: &str,
    body: Value,
) -> Result<T, String> {
    validate_config(config).map_err(|e| {
        log::error("pis-api", &format!("配置校验失败: {e}"));
        "终端尚未完成配置，请联系工作人员！".to_string()
    })?;

    let mut map = body.as_object().cloned().unwrap_or_default();
    let signature = sign_body(&mut map, &config.service.secret_key).map_err(|e| {
        log::error("pis-api", &format!("请求签名失败: {e}"));
        "终端配置异常，请联系工作人员！".to_string()
    })?;
    map.insert("pisDataSignature".into(), Value::String(signature));

    let url = format!(
        "{}{}",
        config.service.base_url.trim_end_matches('/'),
        pathname
    );
    let timeout_secs = u64::from(config.service.request_timeout_seconds.min(5));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| {
            log::error("pis-api", &format!("HTTP 客户端初始化失败: {e}"));
            "网络初始化失败，请联系工作人员！".to_string()
        })?;

    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Pis-Api-Key", &config.service.api_key)
        .json(&map)
        .send()
        .await
        .map_err(|e| {
            log::error("pis-api", &format!("接口请求失败: {e}"));
            if e.is_timeout() {
                "接口请求超时，请联系工作人员！".to_string()
            } else if e.is_connect() {
                "网络连接失败，请联系工作人员！".to_string()
            } else {
                "网络连接异常，请联系工作人员！".to_string()
            }
        })?;

    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let payload: Result<PisResponse<T>, _> = serde_json::from_str(&text);

    if !status.is_success() {
        let detail = payload
            .as_ref()
            .ok()
            .and_then(|p| p.msg.clone())
            .unwrap_or_else(|| format!("请求失败（HTTP {}）", status.as_u16()));
        log::error("pis-api", &format!("{pathname} {detail}"));
        return Err(if status.as_u16() >= 500 {
            "服务暂时不可用，请联系工作人员！".to_string()
        } else {
            "查询失败，请联系工作人员！".to_string()
        });
    }

    match payload {
        Ok(p) if p.code == 0 => Ok(p.data.unwrap_or_default()),
        Ok(p) => {
            log::error(
                "pis-api",
                &format!(
                    "{pathname} 接口返回错误码 {}：{}",
                    p.code,
                    p.msg.unwrap_or_default()
                ),
            );
            Err("查询失败，请联系工作人员！".into())
        }
        Err(e) => {
            log::error("pis-api", &format!("{pathname} 响应解析失败: {e}"));
            Err("服务返回数据异常，请联系工作人员！".into())
        }
    }
}

/// 查询可打印报告（异步）
/// POST /{orgId}/query/patient/print
pub async fn query_patient_print(
    config: &AppConfig,
    keyword: String,
) -> Result<Vec<ReportItem>, String> {
    let pathname = format!("/{}/query/patient/print", config.service.org_id.trim());
    let result =
        post::<Vec<ReportItem>>(config, &pathname, json!({ "keyword": keyword.trim() })).await;

    match &result {
        Ok(list) => log::info("pis-api", &format!("查询报告成功，共 {} 条", list.len())),
        Err(e) => log::warn("pis-api", &format!("查询报告失败: {e}")),
    }
    result
}
