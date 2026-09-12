//! 打印机枚举与系统打印（平移自 src-tauri/src/printer.rs，移除 Tauri IPC）
//!
//! Windows 平台走 winspool 原生 API（EnumPrintersW / GetDefaultPrinterW）；
//! lpstat / lp 仅用于 macOS/Linux。
//! 另提供打印机实时状态诊断（缺纸/卡纸/脱机等），供打印前预检与失败后定位。

// Windows 下 lpstat / lp 调用被条件编译排除
#[cfg(not(target_os = "windows"))]
use std::process::Command;

use serde::Serialize;

#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Printing::{
    PRINTER_ATTRIBUTE_WORK_OFFLINE, PRINTER_STATUS_BUSY, PRINTER_STATUS_DOOR_OPEN,
    PRINTER_STATUS_ERROR, PRINTER_STATUS_INITIALIZING, PRINTER_STATUS_MANUAL_FEED,
    PRINTER_STATUS_NOT_AVAILABLE, PRINTER_STATUS_NO_TONER, PRINTER_STATUS_OFFLINE,
    PRINTER_STATUS_OUTPUT_BIN_FULL, PRINTER_STATUS_PAPER_JAM, PRINTER_STATUS_PAPER_OUT,
    PRINTER_STATUS_PAPER_PROBLEM, PRINTER_STATUS_PAUSED, PRINTER_STATUS_POWER_SAVE,
    PRINTER_STATUS_PRINTING, PRINTER_STATUS_PROCESSING, PRINTER_STATUS_TONER_LOW,
    PRINTER_STATUS_USER_INTERVENTION, PRINTER_STATUS_WAITING, PRINTER_STATUS_WARMING_UP,
};

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
    use windows::Win32::Graphics::Printing::{
        EnumPrintersW, PRINTER_ATTRIBUTE_DEFAULT, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL,
        PRINTER_INFO_2W,
    };

    // 原生 winspool 枚举（毫秒级）：旧实现走 PowerShell + N+1 CIM 查询需 1~3 秒
    unsafe {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let mut needed = 0u32;
        let mut count = 0u32;
        // 第一次调用仅探测所需缓冲区大小（必然"失败"并回填 needed）
        let _ = EnumPrintersW(flags, None, 2, None, &mut needed, &mut count);
        if needed == 0 {
            return Ok(Vec::new());
        }
        let mut buffer = vec![0u8; needed as usize];
        EnumPrintersW(
            flags,
            None,
            2,
            Some(buffer.as_mut_slice()),
            &mut needed,
            &mut count,
        )
        .map_err(|e| format!("枚举打印机失败: {e}"))?;

        let base = buffer.as_ptr() as *const PRINTER_INFO_2W;
        let mut printers = Vec::with_capacity(count as usize);
        for i in 0..count {
            let info = &*base.add(i as usize);
            let name = wide_string(info.pPrinterName);
            if name.is_empty() {
                continue;
            }
            let is_default = (info.Attributes & PRINTER_ATTRIBUTE_DEFAULT) != 0;
            let work_offline = (info.Attributes & PRINTER_ATTRIBUTE_WORK_OFFLINE) != 0;
            printers.push(PrinterInfo {
                name: name.clone(),
                display_name: name,
                is_default,
                status: describe_status(info.Status, work_offline),
            });
        }
        Ok(printers)
    }
}

/// 读取 PRINTER_INFO_2W 的 UTF-16 字符串成员（PWSTR）
#[cfg(target_os = "windows")]
fn wide_string(ptr: windows::core::PWSTR) -> String {
    unsafe {
        let mut len = 0usize;
        while *ptr.0.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr.0, len))
    }
}

/// 致命状态标志位：命中任意一个都无法正常接单。按优先级排列，
/// 先具体（缺纸/卡纸）后通用（故障），多位置位时取第一个命中
#[cfg(target_os = "windows")]
const FATAL_STATUS_FLAGS: &[(u32, &str)] = &[
    (PRINTER_STATUS_PAPER_JAM, "卡纸"),
    (PRINTER_STATUS_PAPER_OUT, "缺纸"),
    (PRINTER_STATUS_PAPER_PROBLEM, "纸张异常"),
    (PRINTER_STATUS_OUTPUT_BIN_FULL, "出纸口已满"),
    (PRINTER_STATUS_DOOR_OPEN, "盖板未关闭"),
    (PRINTER_STATUS_NO_TONER, "墨粉已用尽"),
    (PRINTER_STATUS_MANUAL_FEED, "等待手动进纸"),
    (PRINTER_STATUS_USER_INTERVENTION, "需要人工干预"),
    (PRINTER_STATUS_OFFLINE, "脱机"),
    (PRINTER_STATUS_NOT_AVAILABLE, "不可用"),
    (PRINTER_STATUS_PAUSED, "已暂停"),
    (PRINTER_STATUS_ERROR, "故障"),
];

/// 信息性状态标志位：不阻断打印，仅用于设置页展示
#[cfg(target_os = "windows")]
const INFO_STATUS_FLAGS: &[(u32, &str)] = &[
    (PRINTER_STATUS_PRINTING, "打印中"),
    (PRINTER_STATUS_PROCESSING, "处理中"),
    (PRINTER_STATUS_WARMING_UP, "预热中"),
    (PRINTER_STATUS_INITIALIZING, "初始化中"),
    (PRINTER_STATUS_TONER_LOW, "墨粉不足"),
    (PRINTER_STATUS_BUSY, "忙碌"),
    (PRINTER_STATUS_WAITING, "等待中"),
    (PRINTER_STATUS_POWER_SAVE, "省电模式"),
];

/// 把 winspool 状态标志位翻译为简短中文；「脱机使用打印机」属性（WORK_OFFLINE）
/// 优先于状态位。无任何命中 = 就绪
#[cfg(target_os = "windows")]
fn describe_status(status: u32, work_offline: bool) -> String {
    let label = if work_offline {
        Some("脱机")
    } else {
        FATAL_STATUS_FLAGS
            .iter()
            .chain(INFO_STATUS_FLAGS.iter())
            .find(|(flag, _)| status & flag != 0)
            .map(|(_, label)| *label)
    };
    label.unwrap_or("就绪").to_string()
}

/// 查询打印机实时状态，判断是否无法接单（缺纸、卡纸、脱机等致命状态）。
/// 返回 `Some(中文原因)` 表示驱动已上报致命状态；`None` 表示状态正常或查询失败。
///
/// 查询失败刻意放行（fail-open）：预检只是尽力而为，不因预检本身故障阻断打印，
/// 真正的故障由实际打印流程报错。
///
/// 注意：winspool 状态由驱动上报，部分驱动仅在队列有作业时才刷新，
/// 空闲时缺纸可能仍显示就绪——打印失败后的复查才是准确率最高的诊断时机。
#[cfg(target_os = "windows")]
pub fn printer_fatal_status(printer: &str) -> Option<String> {
    use windows::Win32::Graphics::Printing::{
        ClosePrinter, GetPrinterW, OpenPrinterW, PRINTER_HANDLE, PRINTER_INFO_2W,
    };
    use windows::core::PCWSTR;

    let name = printer.trim();
    if name.is_empty() {
        return None;
    }
    let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let mut handle = PRINTER_HANDLE::default();
        if OpenPrinterW(PCWSTR(name_wide.as_ptr()), &mut handle, None).is_err() {
            return None;
        }
        let fatal = (|| {
            let mut needed = 0u32;
            // 第一次调用仅探测缓冲区大小（必然"失败"并回填 needed）
            let _ = GetPrinterW(handle, 2, None, &mut needed);
            if needed == 0 {
                return None;
            }
            let mut buffer = vec![0u8; needed as usize];
            GetPrinterW(handle, 2, Some(&mut buffer), &mut needed).ok()?;
            let info = &*(buffer.as_ptr() as *const PRINTER_INFO_2W);
            if info.Attributes & PRINTER_ATTRIBUTE_WORK_OFFLINE != 0 {
                return Some("脱机".to_string());
            }
            FATAL_STATUS_FLAGS
                .iter()
                .find(|(flag, _)| info.Status & flag != 0)
                .map(|(_, label)| label.to_string())
        })();
        let _ = ClosePrinter(handle);
        fatal
    }
}

/// 系统默认打印机名（失败或不存在时返回空字符串）
pub fn default_printer_name() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::core::PWSTR;
        use windows::Win32::Foundation::GetLastError;
        use windows::Win32::Graphics::Printing::GetDefaultPrinterW;
        unsafe {
            let mut needed = 0u32;
            // 第一次调用仅探测缓冲区大小（缓冲区不足返回 false 并回填 needed）
            let _ = GetDefaultPrinterW(None, &mut needed);
            if needed == 0 {
                return Ok(String::new());
            }
            let mut buffer = vec![0u16; needed as usize];
            let mut written = 0u32;
            if !GetDefaultPrinterW(Some(PWSTR(buffer.as_mut_ptr())), &mut written).as_bool() {
                crate::domain::log::warn(
                    "printer",
                    &format!("读取默认打印机失败（GetLastError={:?}）", GetLastError()),
                );
                return Ok(String::new());
            }
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            Ok(String::from_utf16_lossy(&buffer[..len]))
        }
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
pub const PRINT_UNCERTAIN_ERR: &str = "__print_uncertain__";

pub const PRINT_CANCELLED_ERR: &str = "__print_cancelled__";

#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::*;

    /// 原生 winspool 枚举可用：至少枚举到一台打印机（本机必有 Microsoft Print to PDF）
    #[test]
    fn native_enumeration_lists_printers() {
        let printers = list_printers().expect("枚举打印机失败");
        assert!(!printers.is_empty(), "应至少枚举到一台打印机");
        assert!(
            printers.iter().all(|p| !p.name.is_empty()),
            "打印机名不应为空"
        );
    }

    /// 默认打印机与枚举结果一致（存在默认打印机时）
    #[test]
    fn default_printer_matches_enumeration() {
        let default = default_printer_name().unwrap_or_default();
        if default.is_empty() {
            return; // 无默认打印机（如全新系统），跳过
        }
        let printers = list_printers().expect("枚举打印机失败");
        assert!(
            printers.iter().any(|p| p.name == default),
            "枚举结果应包含默认打印机 {default}"
        );
    }

    /// 状态分类：致命位优先于信息位，映射顺序即优先级（具体状态先于通用「故障」）
    #[test]
    fn classifies_status_flags() {
        assert_eq!(describe_status(0, false), "就绪");
        assert_eq!(describe_status(PRINTER_STATUS_PRINTING, false), "打印中");
        assert_eq!(describe_status(PRINTER_STATUS_PAPER_OUT, false), "缺纸");
        assert_eq!(describe_status(PRINTER_STATUS_PAPER_JAM, false), "卡纸");
        assert_eq!(
            describe_status(PRINTER_STATUS_PAPER_OUT | PRINTER_STATUS_ERROR, false),
            "缺纸"
        );
        // 「脱机使用打印机」属性独立于状态位
        assert_eq!(describe_status(0, true), "脱机");
    }

    /// 预检 fail-open：查询不存在的打印机或空名称返回 None，不阻断打印
    #[test]
    fn fatal_status_fails_open_for_unknown_printer() {
        assert!(printer_fatal_status("__no_such_printer__").is_none());
        assert!(printer_fatal_status("   ").is_none());
    }

    /// 预检真实打印机：本机必有 Microsoft Print to PDF，正常情况下不报致命状态
    #[test]
    fn fatal_status_queries_real_printer() {
        let printers = list_printers().expect("枚举打印机失败");
        let target = printers.iter().find(|p| p.is_default).unwrap_or(&printers[0]);
        // 不断言具体结果（状态取决于设备），只验证查询本身不出错
        let _ = printer_fatal_status(&target.name);
    }
}

/// 打印一个文件（PDF）到指定打印机
/// - macOS / Linux：`lp`，支持 media 与 orientation-requested
/// - Windows：直调 Win32 打印 API（printer_win.rs）
pub fn print_file(
    file_path: &str,
    printer: Option<&str>,
    paper: Option<&str>,
    orientation: Option<&str>,
) -> Result<Option<i32>, String> {
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
            Ok(None)
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
