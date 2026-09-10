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

/// 缓存的 HTTP 客户端（超时秒数 + Client）：复用连接池避免每次请求重建
/// （重建会丢掉 keep-alive 连接，每次都重新 TCP/TLS 握手）。
/// 配置超时仅经设置页修改（1–5 秒），变更频率极低，按值比对、不同则重建。
static HTTP_CLIENT: std::sync::Mutex<Option<(u64, reqwest::Client)>> = std::sync::Mutex::new(None);

fn http_client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    let mut guard = HTTP_CLIENT
        .lock()
        .map_err(|_| "网络初始化失败，请联系工作人员！".to_string())?;
    if let Some((secs, client)) = guard.as_ref() {
        if *secs == timeout_secs {
            return Ok(client.clone()); // Client 内部为 Arc，克隆零成本
        }
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .map_err(|e| {
            log::error("pis-api", &format!("HTTP 客户端初始化失败: {e}"));
            "网络初始化失败，请联系工作人员！".to_string()
        })?;
    *guard = Some((timeout_secs, client.clone()));
    Ok(client)
}

/// Only errors proven to occur before dispatch may be retried for counter updates.
#[derive(Debug)]
pub struct RequestFailure {
    pub message: String,
    pub safe_to_retry: bool,
}
impl From<String> for RequestFailure {
    fn from(message: String) -> Self {
        Self {
            message,
            safe_to_retry: false,
        }
    }
}

pub(crate) async fn post<T: DeserializeOwned + Default>(
    config: &AppConfig,
    pathname: &str,
    body: Value,
) -> Result<T, String> {
    post_with_delivery(config, pathname, body)
        .await
        .map_err(|e| e.message)
}

/// 发起签名 POST 请求并解析统一响应
/// 返回的错误为面向患者的友好提示（技术细节只写入日志）
pub(crate) async fn post_with_delivery<T: DeserializeOwned + Default>(
    config: &AppConfig,
    pathname: &str,
    body: Value,
) -> Result<T, RequestFailure> {
    validate_config(config).map_err(|e| {
        log::error("pis-api", &format!("配置校验失败: {e}"));
        RequestFailure {
            message: "终端尚未完成配置，请联系工作人员！".into(),
            safe_to_retry: true,
        }
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
    let client = http_client(timeout_secs)?;

    log::info(
        "pis-api",
        &format!("POST {url} 请求入参: {}", Value::Object(map.clone())),
    );

    let response = client
        .post(url)
        .header("Content-Type", "application/json")
        .header("Pis-Api-Key", &config.service.api_key)
        .json(&map)
        .send()
        .await
        .map_err(|e| {
            log::error("pis-api", &format!("{pathname} 接口请求失败: {e}"));
            let safe_to_retry = e.is_connect();
            let message = if e.is_timeout() {
                "接口请求超时，请联系工作人员！".to_string()
            } else if e.is_connect() {
                "网络连接失败，请联系工作人员！".to_string()
            } else {
                "网络连接异常，请联系工作人员！".to_string()
            };
            RequestFailure {
                message,
                safe_to_retry,
            }
        })?;

    let status = response.status();
    let text = response.text().await.map_err(|e| {
        log::error(
            "pis-api",
            &format!("{pathname} HTTP {} 读取接口响应失败: {e}", status.as_u16()),
        );
        RequestFailure {
            message: format!("读取接口响应失败：{e}"),
            safe_to_retry: false,
        }
    })?;
    log::info(
        "pis-api",
        &format!("{pathname} HTTP {} 响应: {text}", status.as_u16()),
    );
    let payload: Result<PisResponse<T>, _> = serde_json::from_str(&text);

    if !status.is_success() {
        let detail = payload
            .as_ref()
            .ok()
            .and_then(|p| p.msg.clone())
            .unwrap_or_else(|| format!("请求失败（HTTP {}）", status.as_u16()));
        log::error("pis-api", &format!("{pathname} {detail}"));
        return Err((if status.as_u16() >= 500 {
            "服务暂时不可用，请联系工作人员！".to_string()
        } else {
            "查询失败，请联系工作人员！".to_string()
        })
        .into());
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
            Err("查询失败，请联系工作人员！".to_string().into())
        }
        Err(e) => {
            log::error("pis-api", &format!("{pathname} 响应解析失败: {e}"));
            Err("服务返回数据异常，请联系工作人员！".to_string().into())
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

#[cfg(test)]
mod delivery_tests {
    use super::*;
    use std::io::{Read, Write};
    fn config(url: String) -> AppConfig {
        let mut c = AppConfig::default();
        c.service.base_url = url;
        c.service.org_id = "test".into();
        c.service.api_key = "test-key".into();
        c.service.secret_key = "test-secret".into();
        c.service.request_timeout_seconds = 1;
        c
    }
    #[test]
    fn dropped_response_and_invalid_response_are_never_safe_to_retry() {
        for response in [
            "",
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad",
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let c = config(format!("http://{}", listener.local_addr().unwrap()));
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 1024];
                loop {
                    let n = stream.read(&mut buffer).unwrap();
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..n]);
                    if let Some(pos) = request.windows(4).position(|v| v == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&request[..pos]).to_lowercase();
                        let size: usize = headers
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length:"))
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        if request.len() >= pos + 4 + size {
                            break;
                        }
                    }
                }
                assert!(String::from_utf8_lossy(&request).contains("\"ids\""));
                stream.write_all(response.as_bytes()).unwrap();
            });
            let rt = tokio::runtime::Runtime::new().unwrap();
            let failure = rt
                .block_on(post_with_delivery::<bool>(
                    &c,
                    "/update/patient/print/status",
                    json!({"ids":["A"]}),
                ))
                .unwrap_err();
            assert!(!failure.safe_to_retry);
            server.join().unwrap();
        }
    }
    #[test]
    fn refused_connection_is_safe_to_retry() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let c = config(format!("http://{}", listener.local_addr().unwrap()));
        drop(listener);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let failure = rt
            .block_on(post_with_delivery::<bool>(
                &c,
                "/update/patient/print/status",
                json!({"ids":["A"]}),
            ))
            .unwrap_err();
        assert!(failure.safe_to_retry);
    }
}
