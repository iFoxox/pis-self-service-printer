//! 打印机枚举与系统打印（平移自 src-tauri/src/printer.rs，移除 Tauri IPC）
//!
//! Windows 平台的子进程调用统一走 super::syscmd::silent（避免弹出控制台黑框），
//! lpstat / lp 仅用于 macOS/Linux。

// Windows 下 lpstat / lp 调用被条件编译排除
#[cfg(not(target_os = "windows"))]
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterInfo {
    pub name: String,
    pub display_name: String,
    pub is_default: bool,
    pub status: String,
}

/// 列出系统打印机
pub fn list_printers() -> Result<Vec<PrinterInfo>, String> {
    #[cfg(target_os = "windows")]
    {
        list_printers_windows()
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        list_printers_lpstat()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Ok(Vec::new())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn list_printers_lpstat() -> Result<Vec<PrinterInfo>, String> {
    let default_name = default_printer_name().unwrap_or_default();
    let queue_output = Command::new("lpstat")
        .arg("-e")
        .output()
        .map_err(|e| format!("调用 lpstat 失败: {e}"))?;
    let queues = parse_destinations(&String::from_utf8_lossy(&queue_output.stdout));

    // macOS 会按系统语言本地化 lpstat 状态行。队列名仍用 `lpstat -e` 读取，
    // 显示名则从 `lpstat -D -p` 的第二条缩进行（description）中提取。
    let detail_output = Command::new("lpstat")
        .args(["-D", "-p"])
        .output()
        .map_err(|e| format!("调用 lpstat 失败: {e}"))?;
    let detail_text = String::from_utf8_lossy(&detail_output.stdout);
    let (descriptions, statuses) = parse_details(&queues, &detail_text);

    Ok(queues
        .into_iter()
        .map(|name| {
            let display_name = descriptions
                .get(&name)
                .cloned()
                .unwrap_or_else(|| name.clone());
            PrinterInfo {
                display_name,
                is_default: name == default_name,
                status: statuses
                    .get(&name)
                    .cloned()
                    .unwrap_or_else(|| "unknown".into()),
                name,
            }
        })
        .collect())
}

/// 解析 `lpstat -e`：每行是一个 CUPS 打印队列名
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn parse_destinations(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// 解析默认队列。macOS/Linux 本地化文案可能使用全角冒号。
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn parse_default_printer(output: &str) -> String {
    for line in output.lines() {
        let Some((_, value)) = split_label_value(line) else {
            continue;
        };
        let value = value.trim();
        if !value.is_empty() && !value.eq_ignore_ascii_case("none") {
            return value.to_string();
        }
    }
    String::new()
}

/// 解析 `lpstat -D -p`：状态行关联队列，其后第一条缩进行是描述
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn parse_details(
    queues: &[String],
    output: &str,
) -> (
    std::collections::HashMap<String, String>,
    std::collections::HashMap<String, String>,
) {
    let mut descriptions = std::collections::HashMap::new();
    let mut statuses = std::collections::HashMap::new();
    let mut current: Option<&str> = None;
    let mut want_description = false;

    for line in output.lines() {
        if line.starts_with('\t') || line.starts_with(' ') {
            if want_description {
                if let Some((_, value)) = split_label_value(line) {
                    if let Some(name) = current {
                        descriptions.insert(name.to_string(), value.trim().to_string());
                    }
                }
                want_description = false;
            }
            continue;
        }

        current = queues
            .iter()
            .filter(|name| line.contains(name.as_str()))
            .map(|name| name.as_str())
            .max_by_key(|name| name.len());
        want_description = current.is_some();
        if let Some(name) = current {
            statuses.insert(name.to_string(), normalize_status(line));
        }
    }

    (descriptions, statuses)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn normalize_status(line: &str) -> String {
    let lower = line.to_lowercase();
    if lower.contains("idle") || line.contains("闲置") {
        "idle".into()
    } else if lower.contains("disabled") || line.contains("停止") {
        "stopped".into()
    } else if lower.contains("printing") || line.contains("打印中") {
        "printing".into()
    } else {
        "unknown".into()
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn split_label_value(line: &str) -> Option<(&str, &str)> {
    let (idx, colon) = line
        .char_indices()
        .find(|(_, ch)| *ch == ':' || *ch == '：')?;
    Some((&line[..idx], &line[idx + colon.len_utf8()..]))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_localized_queues_and_descriptions() {
        let queues = parse_destinations("_10_15_48_37\n_10_15_48_7\n");
        let output = "打印机_10_15_48_37闲置，启用时间始于Tue Apr  7 16:47:23 2026\n\t描述：门诊二楼的黑白打印机\n\t警报：none\n打印机_10_15_48_7闲置，启用时间始于Tue Apr  7 16:47:23 2026\n\t描述：门诊二楼的彩色打印机\n\t警报：toner-empty-warning\n";
        let (descriptions, statuses) = parse_details(&queues, output);

        assert_eq!(queues, ["_10_15_48_37", "_10_15_48_7"]);
        assert_eq!(
            descriptions.get("_10_15_48_37").unwrap(),
            "门诊二楼的黑白打印机"
        );
        assert_eq!(
            descriptions.get("_10_15_48_7").unwrap(),
            "门诊二楼的彩色打印机"
        );
        assert_eq!(statuses.get("_10_15_48_37").unwrap(), "idle");
    }

    #[test]
    fn parses_localized_default_printer() {
        assert_eq!(
            parse_default_printer("系统默认目的位置：_10_15_48_37\n"),
            "_10_15_48_37"
        );
        assert_eq!(
            parse_default_printer("system default destination: _queue\n"),
            "_queue"
        );
        assert_eq!(parse_default_printer("no system default destination\n"), "");
    }
}

#[cfg(target_os = "windows")]
fn list_printers_windows() -> Result<Vec<PrinterInfo>, String> {
    let script = r#"Get-Printer | Select-Object Name,@{n='Default';e={(Get-CimInstance Win32_Printer -Filter "Name='$($_.Name)'").Default}},PrinterStatus | ConvertTo-Json -Compress"#;
    let output = super::syscmd::silent("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| format!("调用 PowerShell 失败: {e}"))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| format!("解析打印机列表失败: {e}"))?;
    let arr = match value {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(_) => vec![value],
        _ => return Ok(Vec::new()),
    };
    Ok(arr
        .into_iter()
        .map(|v| PrinterInfo {
            name: v
                .get("Name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            display_name: v
                .get("Name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            is_default: v.get("Default").and_then(|x| x.as_bool()).unwrap_or(false),
            status: v
                .get("PrinterStatus")
                .map(|x| x.to_string())
                .unwrap_or_default(),
        })
        .filter(|p| !p.name.is_empty())
        .collect())
}

/// 系统默认打印机名（失败或不存在时返回空字符串）
pub fn default_printer_name() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let script = r#"[Console]::OutputEncoding=[System.Text.Encoding]::UTF8; (Get-CimInstance -ClassName Win32_Printer -Filter "Default=TRUE").Name"#;
        let output = super::syscmd::silent("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .map_err(|e| format!("调用 PowerShell 失败: {e}"))?;
        let name = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        Ok(name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("lpstat")
            .arg("-d")
            .output()
            .map_err(|e| format!("调用 lpstat 失败: {e}"))?;
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(parse_default_printer(&text))
    }
}

/// 用户在系统打印对话框 / 假脱机程序取消打印的标记错误。
/// UI 层据此静默返回报告页并重新计时，而不是弹出错误。
pub const PRINT_CANCELLED_ERR: &str = "__print_cancelled__";

/// 打印一个文件（PDF）到指定打印机
/// - macOS / Linux：`lp`，支持 media 与 orientation-requested
/// - Windows：直调 Win32 打印 API（printer_win.rs）
pub fn print_file(
    file_path: &str,
    printer: Option<&str>,
    paper: Option<&str>,
    orientation: Option<&str>,
) -> Result<(), String> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        let mut cmd = Command::new("lp");
        if let Some(p) = printer {
            if !p.trim().is_empty() {
                cmd.args(["-d", p]);
            }
        }
        if let Some(paper) = paper {
            if paper == "A4" || paper == "A5" {
                let media = format!("media={paper}");
                cmd.args(["-o", &media]);
            }
        }
        if let Some(orientation) = orientation {
            let value = if orientation == "landscape" { "4" } else { "3" };
            let opt = format!("orientation-requested={value}");
            cmd.args(["-o", &opt]);
        }
        cmd.arg(file_path);
        let out = cmd.output().map_err(|e| format!("调用 lp 失败: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }
    #[cfg(target_os = "windows")]
    {
        super::printer_win::print_pdf(file_path, printer, paper, orientation)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        Err("当前平台不支持系统打印".into())
    }
}
