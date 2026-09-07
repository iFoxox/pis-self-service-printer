//! Durable local print ledger and non-idempotent status outbox.
//! No PDF, patient keyword or API credentials are stored. A request whose delivery
//! is uncertain is never replayed: the status endpoint increments a counter.
use super::{
    config::{AppConfig, ConfigStore},
    report::ReportItem,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
enum Status {
    Reserved,
    Pending,
    Sending,
    Confirmed,
    Review,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Job {
    sequence: u64,
    scope: String,
    report_id: String,
    file_id: String,
    count: i64,
    spool_job_id: Option<i32>,
    last_error: Option<String>,
    status: Status,
    attempts: u32,
    retry_at: i64,
}

struct Ledger {
    file: File,
    failed: bool,
    jobs: BTreeMap<u64, Job>,
}
pub struct PrintJobs {
    inner: Mutex<Ledger>,
}

fn scope(config: &AppConfig) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        Sha256::digest(format!(
            "{}\n{}",
            config.service.base_url.trim_end_matches('/'),
            config.service.org_id.trim()
        ))
    )
}

impl Ledger {
    fn append(&mut self, job: Job) -> Result<(), String> {
        if self.failed {
            return Err("打印记录存储不可用，请联系工作人员".into());
        }
        let mut data = serde_json::to_vec(&job).map_err(|e| e.to_string())?;
        data.push(b'\n');
        let offset = self.file.metadata().map_err(|e| e.to_string())?.len();
        if let Err(e) = self
            .file
            .write_all(&data)
            .and_then(|_| self.file.sync_all())
        {
            self.failed = true;
            // Roll back partial records; never append behind a torn JSON record.
            let _ = self.file.set_len(offset);
            let _ = self.file.sync_all();
            return Err(format!("保存打印记录失败：{e}"));
        }
        self.jobs.insert(job.sequence, job);
        Ok(())
    }
}

impl PrintJobs {
    fn open(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut options = OpenOptions::new();
        options.create(true).read(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        let valid_len = bytes.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
        if valid_len != bytes.len() {
            // Never erase a torn Sending record: its preceding Pending record
            // would otherwise make a non-idempotent POST eligible for replay.
            return Err("打印记录写入中断，请联系工作人员核对；已保留原始记录".into());
        }
        let mut jobs = BTreeMap::new();
        for line in bytes[..valid_len]
            .split(|b| *b == b'\n')
            .filter(|s| !s.is_empty())
        {
            let job: Job = serde_json::from_slice(line)
                .map_err(|e| format!("打印记录损坏，请联系工作人员：{e}"))?;
            jobs.insert(job.sequence, job);
        }
        file.set_len(valid_len as u64).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        let mut ledger = Ledger {
            file,
            jobs,
            failed: false,
        };
        // Crash during spool submission or status POST: neither can safely be replayed.
        let interrupted: Vec<_> = ledger
            .jobs
            .values()
            .filter(|j| matches!(j.status, Status::Reserved | Status::Sending))
            .cloned()
            .collect();
        for mut job in interrupted {
            job.status = Status::Review;
            job.last_error = Some("进程中断，打印或回写结果需要人工核对".into());
            ledger.append(job)?;
        }
        Ok(Self {
            inner: Mutex::new(ledger),
        })
    }

    pub fn start(config: ConfigStore) -> Result<Arc<Self>, String> {
        let jobs = Arc::new(Self::open(
            crate::paths::app_data_dir().join("print-jobs.jsonl"),
        )?);
        let worker = jobs.clone();
        std::thread::Builder::new()
            .name("print-status-outbox".into())
            .spawn(move || {
                loop {
                    if let Err(e) = worker.process_one(
                        &config.get(),
                        chrono::Utc::now().timestamp(),
                        super::update_print_status_blocking,
                    ) {
                        super::log::error("print-outbox", &e);
                    }
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            })
            .map_err(|e| format!("无法启动打印状态回写：{e}"))?;
        Ok(jobs)
    }

    fn matches(job: &Job, scope: &str, report: &ReportItem) -> bool {
        job.scope == scope && job.report_id == report.id && job.file_id == report.report_file_id
    }

    pub fn blocked(&self, config: &AppConfig, report: &ReportItem) -> bool {
        let Ok(ledger) = self.inner.lock() else {
            return true;
        };
        if ledger.failed {
            return true;
        }
        let scope = scope(config);
        ledger.jobs.values().any(|j| {
            Self::matches(j, &scope, report)
                && match j.status {
                    Status::Failed => false,
                    Status::Confirmed => !config.print.allow_reprint,
                    _ => true,
                }
        })
    }

    pub fn overlay(&self, config: &AppConfig, reports: &mut [ReportItem]) {
        let Ok(mut ledger) = self.inner.lock() else {
            return;
        };
        let scope = scope(config);
        // Read-only reconciliation: seeing the expected counter is sufficient to
        // clear a lost acknowledgment; a lower counter never causes a replay.
        let confirmed: Vec<_> = ledger
            .jobs
            .values()
            .filter(|j| {
                j.status == Status::Review
                    && j.attempts > 0
                    && reports
                        .iter()
                        .any(|r| Self::matches(j, &scope, r) && r.patient_print_count >= j.count)
            })
            .cloned()
            .collect();
        for mut job in confirmed {
            job.status = Status::Confirmed;
            job.last_error = None;
            if let Err(e) = ledger.append(job) {
                super::log::error("print-outbox", &e);
            }
        }
        for report in reports {
            let count = ledger
                .jobs
                .values()
                .filter(|j| {
                    Self::matches(j, &scope, report)
                        && (matches!(
                            j.status,
                            Status::Pending | Status::Sending | Status::Confirmed
                        ) || (j.status == Status::Review && j.attempts > 0))
                })
                .map(|j| j.count)
                .max();
            if let Some(count) = count {
                report.patient_print_count = report.patient_print_count.max(count);
                report.is_patient_print = Some(1);
            }
        }
    }

    /// Persist intent before touching the spooler. Disk failures stop printing.
    pub fn reserve(&self, config: &AppConfig, report: &ReportItem) -> Result<u64, String> {
        let mut ledger = self.inner.lock().map_err(|e| e.to_string())?;
        let scope = scope(config);
        let related: Vec<_> = ledger
            .jobs
            .values()
            .filter(|j| Self::matches(j, &scope, report) && j.status != Status::Failed)
            .collect();
        if related
            .iter()
            .any(|j| j.status != Status::Confirmed || !config.print.allow_reprint)
        {
            return Err("该报告已提交打印或有待核对的打印记录，请联系工作人员".into());
        }
        let count = related
            .iter()
            .map(|j| j.count)
            .max()
            .unwrap_or(0)
            .max(report.patient_print_count)
            .saturating_add(1);
        let sequence = ledger
            .jobs
            .keys()
            .next_back()
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("打印记录编号已耗尽")?;
        ledger.append(Job {
            sequence,
            scope,
            report_id: report.id.clone(),
            file_id: report.report_file_id.clone(),
            count,
            spool_job_id: None,
            last_error: None,
            status: Status::Reserved,
            attempts: 0,
            retry_at: 0,
        })?;
        Ok(sequence)
    }

    pub fn finish(
        &self,
        sequence: u64,
        result: &Result<super::report::PrintReportResult, String>,
    ) -> Result<(), String> {
        let mut ledger = self.inner.lock().map_err(|e| e.to_string())?;
        let mut job = ledger
            .jobs
            .get(&sequence)
            .cloned()
            .ok_or("打印记录不存在")?;
        if let Ok(result) = result {
            job.spool_job_id = result.spool_job_id;
        }
        job.last_error = result.as_ref().err().cloned();
        job.status = match result {
            Ok(_) => Status::Pending,
            Err(e) if e.contains(super::printer::PRINT_UNCERTAIN_ERR) => Status::Review,
            Err(_) => Status::Failed,
        };
        ledger.append(job)
    }

    fn process_one(
        &self,
        config: &AppConfig,
        now: i64,
        send: impl FnOnce(AppConfig, Vec<String>) -> Result<(), super::pis::RequestFailure>,
    ) -> Result<(), String> {
        let job = {
            let mut ledger = self.inner.lock().map_err(|e| e.to_string())?;
            let Some(mut job) = ledger
                .jobs
                .values()
                .find(|j| {
                    j.scope == scope(config) && j.status == Status::Pending && j.retry_at <= now
                })
                .cloned()
            else {
                return Ok(());
            };
            job.status = Status::Sending;
            job.attempts = job.attempts.saturating_add(1);
            ledger.append(job.clone())?; // Persist BEFORE the non-idempotent request.
            job
        };
        let result = send(config.clone(), vec![job.report_id.clone()]);
        let mut job = job;
        job.last_error = result.as_ref().err().map(|e| e.message.clone());
        match result {
            Ok(()) => {
                job.status = Status::Confirmed;
            }
            Err(e) if e.safe_to_retry => {
                job.status = Status::Pending;
                job.retry_at =
                    now.saturating_add((30_i64 * (1_i64 << job.attempts.min(7))).min(3600));
                super::log::warn(
                    "print-outbox",
                    &format!("回写未发送，将重试（记录 {}）：{}", job.sequence, e.message),
                );
            }
            Err(e) => {
                job.status = Status::Review;
                super::log::error(
                    "print-outbox",
                    &format!(
                        "记录 {} 报告 {} 回写结果未确认，禁止自动重发，请人工核对：{}",
                        job.sequence, job.report_id, e.message
                    ),
                );
            }
        }
        self.inner.lock().map_err(|e| e.to_string())?.append(job)
    }
}

/// Stop at the first failure; never lose preceding successful submissions.
pub fn run_batch(
    config: &AppConfig,
    jobs: &PrintJobs,
    reports: Vec<ReportItem>,
    mut print: impl FnMut(&AppConfig, ReportItem) -> Result<super::report::PrintReportResult, String>,
) -> (Vec<String>, Option<String>) {
    let mut printed = Vec::new();
    for report in reports {
        let sequence = match jobs.reserve(config, &report) {
            Ok(sequence) => sequence,
            Err(e) => return (printed, Some(e)),
        };
        let id = report.id.clone();
        let name = report.subject_name.clone();
        let result = print(config, report);
        if result.is_ok() {
            printed.push(id);
        }
        if let Err(e) = jobs.finish(sequence, &result) {
            return (
                printed,
                Some(format!(
                    "打印记录保存失败，请勿重复打印，请联系工作人员：{e}"
                )),
            );
        }
        if let Err(e) = result {
            return (printed, Some(format!("{name}：{e}")));
        }
    }
    (printed, None)
}

/// Update successful items even when the batch stops on an error or cancellation.
pub fn apply_submitted(reports: &mut [ReportItem], selected: &mut [bool], ids: &[String]) {
    for (index, report) in reports.iter_mut().enumerate() {
        if ids.contains(&report.id) {
            report.is_patient_print = Some(1);
            report.patient_print_count = report.patient_print_count.saturating_add(1);
            if let Some(value) = selected.get_mut(index) {
                *value = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        pis::RequestFailure,
        printer::{PRINT_CANCELLED_ERR, PRINT_UNCERTAIN_ERR},
        report::PrintReportResult,
    };
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "pis-outbox-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
        fn path(&self) -> PathBuf {
            self.0.join("jobs.jsonl")
        }
        fn open(&self) -> PrintJobs {
            PrintJobs::open(self.path()).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn report(id: &str) -> ReportItem {
        serde_json::from_value(serde_json::json!({"orderApplyId":"order", "subjectName":"report", "id":id,"reportFileId":format!("file-{id}"),"reportData":"unused"})).unwrap()
    }
    fn submitted(id: &str) -> Result<PrintReportResult, String> {
        Ok(PrintReportResult {
            report_id: id.into(),
            spool_job_id: Some(42),
        })
    }
    fn pending(jobs: &PrintJobs, config: &AppConfig, id: &str) -> u64 {
        let seq = jobs.reserve(config, &report(id)).unwrap();
        jobs.finish(seq, &submitted(id)).unwrap();
        seq
    }

    #[test]
    fn partial_failure_and_cancel_preserve_success_and_retry_only_remaining() {
        for error in ["printer offline", PRINT_CANCELLED_ERR] {
            let fixture = Fixture::new();
            let jobs = fixture.open();
            let cfg = AppConfig::default();
            let mut reports = vec![report("A"), report("B"), report("C")];
            let mut calls = Vec::new();
            let (ids, failure) = run_batch(&cfg, &jobs, reports.clone(), |_, r| {
                calls.push(r.id.clone());
                if r.id == "B" {
                    Err(error.into())
                } else {
                    submitted(&r.id)
                }
            });
            assert!(failure.unwrap().contains(error));
            assert_eq!(calls, ["A", "B"]);
            assert_eq!(ids, ["A"]);
            let mut selected = vec![true; 3];
            apply_submitted(&mut reports, &mut selected, &ids);
            assert_eq!(selected, [false, true, true]);
            assert_eq!(reports[0].patient_print_count, 1);
            let remaining = reports
                .into_iter()
                .zip(selected)
                .filter_map(|(r, selected)| selected.then_some(r))
                .collect();
            let (ids, failure) = run_batch(&cfg, &jobs, remaining, |_, r| submitted(&r.id));
            assert!(failure.is_none());
            assert_eq!(ids, ["B", "C"]);
        }
    }

    #[test]
    fn pending_survives_restart_and_stale_query_cannot_enable_reprint() {
        let fixture = Fixture::new();
        let cfg = AppConfig::default();
        {
            let jobs = fixture.open();
            pending(&jobs, &cfg, "A");
        }
        let jobs = fixture.open();
        let mut reports = vec![report("A")];
        jobs.overlay(&cfg, &mut reports);
        assert_eq!(reports[0].is_patient_print, Some(1));
        assert_eq!(reports[0].patient_print_count, 1);
        assert!(jobs.blocked(&cfg, &report("A")));
        jobs.process_one(&cfg, 0, |_, ids| {
            assert_eq!(ids, ["A"]);
            Ok(())
        })
        .unwrap();
        jobs.process_one(&cfg, i64::MAX, |_, _| {
            panic!("confirmed POST must not repeat")
        })
        .unwrap();
        let ledger = jobs.inner.lock().unwrap();
        assert_eq!(ledger.jobs[&1].spool_job_id, Some(42));
        assert_eq!(ledger.jobs[&1].status, Status::Confirmed);
    }

    #[test]
    fn only_proven_unsent_failure_is_retried_with_backoff() {
        let fixture = Fixture::new();
        let jobs = fixture.open();
        let cfg = AppConfig::default();
        pending(&jobs, &cfg, "A");
        jobs.process_one(&cfg, 0, |_, _| {
            Err(RequestFailure {
                message: "connect failed".into(),
                safe_to_retry: true,
            })
        })
        .unwrap();
        jobs.process_one(&cfg, 1, |_, _| panic!("retry must back off"))
            .unwrap();
        jobs.process_one(&cfg, 3600, |_, _| Ok(())).unwrap();
        assert_eq!(
            jobs.inner.lock().unwrap().jobs[&1].status,
            Status::Confirmed
        );
    }

    #[test]
    fn lost_ack_is_never_resent_even_after_restart_but_query_can_reconcile() {
        let fixture = Fixture::new();
        let mut cfg = AppConfig::default();
        cfg.print.allow_reprint = true;
        {
            let jobs = fixture.open();
            pending(&jobs, &cfg, "A");
            jobs.process_one(&cfg, 0, |_, _| {
                Err(RequestFailure {
                    message: "timeout after sending".into(),
                    safe_to_retry: false,
                })
            })
            .unwrap();
        }
        let jobs = fixture.open();
        jobs.process_one(&cfg, i64::MAX, |_, _| {
            panic!("counter update must not repeat")
        })
        .unwrap();
        let mut stale = vec![report("A")];
        jobs.overlay(&cfg, &mut stale);
        assert!(jobs.blocked(&cfg, &report("A")));
        let mut fresh = vec![report("A")];
        fresh[0].patient_print_count = 1;
        jobs.overlay(&cfg, &mut fresh);
        assert!(!jobs.blocked(&cfg, &report("A")));
        assert!(jobs.reserve(&cfg, &fresh[0]).is_ok());
    }

    #[test]
    fn crash_during_dispatch_and_torn_last_record_never_replay() {
        for torn in [false, true] {
            let fixture = Fixture::new();
            let cfg = AppConfig::default();
            {
                let jobs = fixture.open();
                pending(&jobs, &cfg, "A");
                let mut ledger = jobs.inner.lock().unwrap();
                if torn {
                    ledger.file.write_all(b"{\"sequence\":1,").unwrap();
                    ledger.file.sync_all().unwrap();
                } else {
                    let mut job = ledger.jobs[&1].clone();
                    job.status = Status::Sending;
                    ledger.append(job).unwrap();
                }
            }
            if torn {
                assert!(PrintJobs::open(fixture.path()).is_err());
                continue;
            }
            let jobs = fixture.open();
            jobs.process_one(&cfg, i64::MAX, |_, _| {
                panic!("crashed dispatch is ambiguous")
            })
            .unwrap();
            assert!(jobs.blocked(&cfg, &report("A")));
            assert_eq!(jobs.inner.lock().unwrap().jobs[&1].status, Status::Review);
        }
    }

    #[test]
    fn partial_output_is_blocked_without_status_increment() {
        let fixture = Fixture::new();
        let jobs = fixture.open();
        let mut cfg = AppConfig::default();
        cfg.print.allow_reprint = true;
        let (ids, error) = run_batch(&cfg, &jobs, vec![report("A")], |_, _| {
            Err(format!("{PRINT_UNCERTAIN_ERR}EndDoc failed"))
        });
        assert!(ids.is_empty());
        assert!(error.is_some());
        assert!(jobs.blocked(&cfg, &report("A")));
        jobs.process_one(&cfg, i64::MAX, |_, _| {
            panic!("uncertain spool output must not increment counter")
        })
        .unwrap();
    }

    #[test]
    fn destination_change_does_not_send_old_jobs_to_new_organization() {
        let fixture = Fixture::new();
        let jobs = fixture.open();
        let cfg = AppConfig::default();
        pending(&jobs, &cfg, "A");
        let mut other = cfg.clone();
        other.service.org_id = "other".into();
        jobs.process_one(&other, i64::MAX, |_, _| panic!("wrong organization"))
            .unwrap();
        assert!(!jobs.blocked(&other, &report("A")));
        jobs.process_one(&cfg, 0, |_, _| Ok(())).unwrap();
    }

    #[test]
    fn ledger_write_failure_prevents_spool_submission() {
        let fixture = Fixture::new();
        let jobs = fixture.open();
        jobs.inner.lock().unwrap().file = File::open(fixture.path()).unwrap();
        let cfg = AppConfig::default();
        let (ids, error) = run_batch(&cfg, &jobs, vec![report("A")], |_, _| {
            panic!("disk failure must stop printing")
        });
        assert!(ids.is_empty());
        assert!(error.is_some());
        assert!(jobs.blocked(&cfg, &report("B")));
        jobs.process_one(&cfg, i64::MAX, |_, _| {
            panic!("must not send after disk failure")
        })
        .unwrap();
    }

    #[test]
    fn malformed_complete_record_fails_closed() {
        let fixture = Fixture::new();
        {
            let _ = fixture.open();
        }
        std::fs::write(fixture.path(), b"not json\n").unwrap();
        assert!(PrintJobs::open(fixture.path()).is_err());
    }
}
