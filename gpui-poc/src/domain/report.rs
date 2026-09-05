//! 报告打印：base64 PDF → 系统打印队列（平移自 src-tauri/src/report.rs）

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use super::config;
use super::log;
use super::printer;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportItem {
    pub order_apply_id: String,
    pub subject_name: String,
    #[serde(default)]
    pub subject_no: Option<String>,
    #[serde(default)]
    pub pathology_no: Option<String>,
    pub id: String,
    #[serde(default)]
    pub report_type: i64,
    pub report_file_id: String,
    #[serde(default)]
    pub patient_print_count: i64,
    pub report_data: String,
    #[serde(default)]
    pub pathology_type_name: Option<String>,
    #[serde(default)]
    pub master_item_name: Option<String>,
    #[serde(default)]
    pub patient_print_at: Option<String>,
    #[serde(default)]
    pub is_patient_print: Option<i64>,
    #[serde(default)]
    pub authorize_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintReportResult {
    pub report_id: String,
}

/// 解析报告数据：兼容 `...,base64,...` 前缀与裸 base64，并校验 PDF 魔数
fn decode_report_data(value: &str) -> Result<Vec<u8>, String> {
    let content = value.trim();
    let base64 = if content.len() >= 5 && content.as_bytes()[..5].eq_ignore_ascii_case(b"data:") {
        // data:[mediatype][;base64],<payload>
        content
            .split_once(',')
            .map(|(_, payload)| payload)
            .ok_or_else(|| "报告数据缺少 Data URL 内容".to_string())?
    } else {
        match content.find(",base64,") {
            Some(idx) => &content[idx + ",base64,".len()..],
            None => content,
        }
    };
    let compact: String = base64.chars().filter(|c| !c.is_whitespace()).collect();
    let buffer = STANDARD
        .decode(compact)
        .map_err(|e| format!("报告数据不是有效的 Base64: {e}"))?;
    if buffer.len() < 5 || &buffer[..5] != b"%PDF-" {
        return Err("报告文件不是有效的 PDF 数据".into());
    }
    Ok(buffer)
}

/// 报告 id 用于临时文件名，仅保留安全字符
fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// 解析默认打印机：配置项 -> 系统默认打印机 -> 打印机列表第一个
fn resolve_printer(config: &config::AppConfig) -> Result<String, String> {
    if !config.print.default_printer.trim().is_empty() {
        return Ok(config.print.default_printer.trim().to_string());
    }
    let system_default = printer::default_printer_name().unwrap_or_default();
    if !system_default.trim().is_empty() {
        return Ok(system_default);
    }
    let printers = printer::list_printers()?;
    printers
        .first()
        .map(|p| p.name.clone())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| "未检测到可用打印机，请联系工作人员".to_string())
}

/// 打印报告（阻塞，供后台线程调用）
pub fn print_report(
    config: &config::AppConfig,
    report: ReportItem,
) -> Result<PrintReportResult, String> {
    if report.id.trim().is_empty() || report.report_data.trim().is_empty() {
        return Err("报告数据不完整".into());
    }

    if report.is_patient_print == Some(1) && !config.print.allow_reprint {
        return Err("该报告已打印，如需补打请联系工作人员".into());
    }

    let buffer = decode_report_data(&report.report_data)?;
    let printer_name = resolve_printer(config)?;

    let file_name = format!(
        "pis-patient-report-{}-{}.pdf",
        sanitize_id(&report.id),
        chrono::Local::now().timestamp_millis()
    );
    let file_path = std::env::temp_dir().join(file_name);

    let result = (|| -> Result<(), String> {
        std::fs::write(&file_path, &buffer).map_err(|e| format!("写入临时文件失败: {e}"))?;
        printer::print_file(
            file_path.to_str().unwrap_or_default(),
            Some(&printer_name),
            Some(&config.print.paper),
            Some(&config.print.orientation),
        )
    })();

    let _ = std::fs::remove_file(&file_path);

    match result {
        Ok(()) => {
            log::info(
                "report-print",
                &format!("报告 {} 已提交到打印机 {printer_name}", report.id),
            );
            Ok(PrintReportResult {
                report_id: report.id,
            })
        }
        Err(e) => {
            log::error("report-print", &format!("报告 {} 打印失败: {e}", report.id));
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PDF_HEADER: &[u8] = b"%PDF-";

    #[test]
    fn decode_raw_base64_report_data() {
        let data = decode_report_data(&STANDARD.encode(PDF_HEADER)).unwrap();
        assert_eq!(data, PDF_HEADER);
    }

    #[test]
    fn decode_data_url_report_data() {
        let encoded = STANDARD.encode(PDF_HEADER);
        let data = decode_report_data(&format!("data:application/pdf;base64,{encoded}")).unwrap();
        assert_eq!(data, PDF_HEADER);
    }

    #[test]
    fn decode_data_url_is_case_insensitive() {
        let encoded = STANDARD.encode(PDF_HEADER);
        let data = decode_report_data(&format!("DATA:APPLICATION/PDF;BASE64,{encoded}")).unwrap();
        assert_eq!(data, PDF_HEADER);
    }

    #[test]
    fn reject_data_url_without_payload() {
        assert!(decode_report_data("data:application/pdf;base64,").is_err());
    }
}
