//! 领域层：平移自 src-tauri/src，剥离 Tauri 耦合
//!
//! - 路径解析改由 crate::paths 提供
//! - 日志函数去掉 AppHandle 参数
//! - IPC command 全部移除，改为普通函数（网络请求阻塞式封装，
//!   由 UI 层经 gpui 后台线程调用，避免阻塞渲染）

pub mod config;
pub mod log;
pub mod logo;
pub mod pis;
pub mod print_jobs;
pub mod printer;
#[cfg(target_os = "windows")]
pub mod printer_win;
pub mod report;

use config::AppConfig;

/// 全局 tokio 运行时：reqwest 需要 tokio reactor，而 gpui 的 executor 是 smol 系，
/// 因此领域层对外提供阻塞接口，内部在独立 tokio runtime 上执行异步请求。
static TOKIO: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

fn runtime() -> &'static tokio::runtime::Runtime {
    TOKIO.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("初始化异步运行时失败")
    })
}

/// 查询可打印报告（阻塞，供后台线程调用）
pub fn query_reports_blocking(
    config: &AppConfig,
    keyword: &str,
) -> Result<Vec<report::ReportItem>, String> {
    runtime().block_on(pis::query_patient_print(config, keyword.to_string()))
}

/// Called only by the durable outbox, never inside the print loop.
pub fn update_print_status_blocking(
    config: AppConfig,
    ids: Vec<String>,
) -> Result<(), pis::RequestFailure> {
    runtime().block_on(async {
        match pis::post_with_delivery::<bool>(
            &config,
            "/update/patient/print/status",
            serde_json::json!({ "ids": ids }),
        )
        .await?
        {
            true => Ok(()),
            false => Err(pis::RequestFailure {
                message: "报告状态回写未确认".into(),
                safe_to_retry: false,
            }),
        }
    })
}

/// 打印报告（阻塞，供后台线程调用；PDFium 光栅化 + GDI 输出可能耗时数秒）
pub fn print_report_blocking(
    config: &AppConfig,
    report: report::ReportItem,
) -> Result<report::PrintReportResult, String> {
    report::print_report(config, report)
}

/// 列出系统打印机
#[allow(dead_code)]
pub fn list_printers() -> Result<Vec<printer::PrinterInfo>, String> {
    printer::list_printers()
}
