//! 全局状态（替代 Pinia store + vue-router）
//!
//! 单一 Entity 承载页面状态机、查询 / 打印流程、空闲倒计时与管理员验证，
//! 视图层（ui/）提供各页面的 render 方法。

use gpui::{AppContext, AnyWindowHandle};
use gpui_component::WindowExt as _;
use gpui_component::input::{InputEvent, InputState};
use gpui_component::slider::{SliderEvent, SliderState, SliderValue};

use crate::domain::config::ConfigStore;
use crate::domain::report::ReportItem;

/// 页面（替代 vue-router）
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Home,
    Search,
    Reports,
    Settings,
}

/// 管理员密码匹配结果（密码决定动作，对应 App.vue confirmPasswordAction）
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AdminMatch {
    Settings,
    Minimize,
    Logs,
}

/// 设置页草稿：完整配置的可编辑副本（对应 SettingsDialog form）。
/// 文本值由 gpui-component 的 Input 实体实时同步回 draft.config。
#[derive(Clone, Debug, Default)]
pub struct SettingsDraft {
    pub config: crate::domain::config::AppConfig,
}

/// 可编辑的文本字段（密码字段仅接受数字、最长 4 位）
pub const TEXT_FIELDS: [&str; 11] = [
    "hospital_name",
    "terminal_code",
    "report_notice",
    "input_hint",
    "base_url",
    "org_id",
    "api_key",
    "secret_key",
    "exit_password",
    "minimize_password",
    "log_password",
];

pub fn is_password_field(key: &str) -> bool {
    matches!(key, "exit_password" | "minimize_password" | "log_password")
}

pub struct KioskState {
    pub config: ConfigStore,
    pub focus_handle: gpui::FocusHandle,
    pub page: Page,
    // 查询 / 打印
    pub keyword: String,
    pub keyword_selected: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub error_countdown: u32,
    pub reports: Vec<ReportItem>,
    pub selected: Vec<bool>,
    pub printing: bool,
    pub completed: bool,
    pub completed_count: u32,
    pub success_countdown: u32,

    // 空闲倒计时 + 顶栏时钟
    pub countdown: u32,
    pub now: chrono::DateTime<chrono::Local>,

    // 触控键盘按下反馈（短暂高亮）
    pub pressed_key: Option<String>,

    // 大按钮按下反馈（返回首页 / 查询 / 上一步 / 确认打印等，短暂高亮）
    pub pressed_action: Option<String>,

    // 管理员验证弹窗
    pub admin_open: bool,
    pub admin_password: String,
    pub admin_error: String,
    pub admin_countdown: u32,
    pub logo_holding: bool,

    // 运行日志弹窗
    pub logs_open: bool,
    pub logs: Vec<String>,
    pub log_current: usize,
    pub log_content: String,

    // 设置草稿
    pub draft: SettingsDraft,
    /// 设置页保存成功提示（2 秒后自动消失）
    pub save_notice: Option<String>,
    pub printers: Vec<crate::domain::printer::PrinterInfo>,
    pub printers_loading: bool,
    /// 「选择文件」目标语音字段（选完文件后写回对应草稿配置）
    pub pending_voice_field: Option<&'static str>,
    /// 主窗口句柄（gpui-component 的 Dialog/Select 需要在 Window 上下文里操作）
    pub window_handle: Option<AnyWindowHandle>,
    /// 设置页打印机下拉（框架 Select 实体，异步刷新后经 window_handle 更新条目）
    pub printer_select: Option<gpui::Entity<gpui_component::select::SelectState<Vec<String>>>>,
    /// 设置页院徽内置预设下拉（框架 Select 实体）
    pub hospital_logo_select:
        Option<gpui::Entity<gpui_component::select::SelectState<Vec<String>>>>,
    /// 设置页运营方 Logo 内置预设下拉（框架 Select 实体）
    pub footer_logo_select: Option<gpui::Entity<gpui_component::select::SelectState<Vec<String>>>>,
    /// 设置页文本字段的 gpui-component Input 实体（键 = TEXT_FIELDS 键名）
    pub settings_inputs: std::collections::HashMap<&'static str, gpui::Entity<InputState>>,
    /// 设置页滑动条的 SliderState 实体（键 = SLIDER_FIELDS 键名）
    pub settings_sliders: std::collections::HashMap<&'static str, gpui::Entity<SliderState>>,
    /// 设置页实体订阅（Input/Slider 事件 → 草稿同步），须保持存活
    pub settings_subs: Vec<gpui::Subscription>,
}

pub const MAX_KEYWORD: usize = 18;

impl KioskState {
    pub fn new(
        config: ConfigStore,
        focus_handle: gpui::FocusHandle,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let countdown = config.get().terminal.idle_timeout_seconds.max(10);
        let state = Self {
            config,
            page: Page::Home,
            keyword: String::new(),
            keyword_selected: false,
            loading: false,
            error: None,
            error_countdown: 10,
            reports: Vec::new(),
            selected: Vec::new(),
            printing: false,
            completed: false,
            completed_count: 0,
            success_countdown: 6,
            countdown,
            now: chrono::Local::now(),
            pressed_key: None,
            pressed_action: None,
            admin_open: false,
            admin_password: String::new(),
            admin_error: String::new(),
            admin_countdown: 30,
            logo_holding: false,
            logs_open: false,
            logs: Vec::new(),
            log_current: 0,
            log_content: String::new(),
            draft: SettingsDraft::default(),
            save_notice: None,
            printers: Vec::new(),
            printers_loading: false,
            pending_voice_field: None,
            window_handle: None,
            printer_select: None,
            hospital_logo_select: None,
            footer_logo_select: None,
            settings_inputs: std::collections::HashMap::new(),
            settings_sliders: std::collections::HashMap::new(),
            settings_subs: Vec::new(),
            focus_handle,
        };
        state.start_idle_timer(cx);
        state
    }

    /// 当前生效配置快照
    pub fn cfg(&self) -> crate::domain::config::AppConfig {
        self.config.get()
    }

    /// 空闲倒计时重置（切换页面 / 任意操作时调用）
    pub fn reset_countdown(&mut self) {
        self.countdown = self.cfg().terminal.idle_timeout_seconds.max(10);
    }

    /// 展示错误弹窗（gpui-component Dialog，10 秒后由每秒循环自动关闭）
    pub fn show_error(&mut self, cx: &mut gpui::Context<Self>, message: impl Into<String>) {
        let message = message.into();
        self.error = Some(message.clone());
        self.error_countdown = 10;
        let kiosk = cx.weak_entity();
        // 必须推迟到 spawn：若从按钮点击/按键的事件派发里嵌套经句柄更新窗口，
        // 更新会被静默吞掉（弹窗不出现）。异步回调路径不受影响，统一走这里。
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |state, cx| {
                if state.error.is_none() {
                    return; // 弹窗已被提前关闭/清除
                }
                let Some(handle) = state.window_handle else {
                    return;
                };
                handle
                    .update(cx, |_, window, cx| {
                        crate::ui::open_error_dialog(window, cx, &kiosk, message.clone());
                    })
                    .ok();
            });
        })
        .detach();
    }

    /// 回到首页并清空查询上下文
    pub fn go_home(&mut self) {
        self.page = Page::Home;
        self.keyword.clear();
        self.keyword_selected = false;
        self.reports.clear();
        self.selected.clear();
        self.error = None;
        self.completed = false;
        self.pressed_key = None;
        crate::audio::stop_speaking();
        self.reset_countdown();
    }

    /// 报告是否不可选择（已打印且未开启补打）
    pub fn report_disabled(&self, report: &ReportItem) -> bool {
        report.is_patient_print == Some(1) && !self.cfg().print.allow_reprint
    }

    /// 可选择的报告下标
    pub fn selectable_indexes(&self) -> Vec<usize> {
        self.reports
            .iter()
            .enumerate()
            .filter(|(_, item)| !self.report_disabled(item))
            .map(|(i, _)| i)
            .collect()
    }

    /// 是否全选（所有可选择项都在选中集合中）
    pub fn all_selected(&self) -> bool {
        let selectable = self.selectable_indexes();
        !selectable.is_empty()
            && selectable
                .iter()
                .all(|&i| self.selected.get(i).copied().unwrap_or(false))
    }

    /// 查询结果装载（含自动全选逻辑，对应 store.setReports）
    pub fn set_reports(&mut self, items: Vec<ReportItem>) {
        let auto = self.cfg().terminal.auto_select_reports;
        self.reports = items;
        self.selected = self
            .reports
            .iter()
            .enumerate()
            .map(|(i, _)| auto && !self.report_disabled(&self.reports[i]))
            .collect();
    }

    /// 切换某行选中状态
    pub fn toggle_report(&mut self, index: usize) {
        if self.printing {
            return;
        }
        if self.reports.get(index).map(|r| self.report_disabled(r)) == Some(true) {
            return;
        }
        if let Some(flag) = self.selected.get_mut(index) {
            *flag = !*flag;
        }
    }

    /// 全选 / 取消全选
    pub fn toggle_select_all(&mut self) {
        if self.printing {
            return;
        }
        if self.all_selected() {
            self.selected = vec![false; self.reports.len()];
        } else {
            self.selected = (0..self.reports.len())
                .map(|i| self.selectable_indexes().contains(&i))
                .collect();
        }
    }

    /// 已选中报告数量
    pub fn selected_count(&self) -> usize {
        self.selected.iter().filter(|s| **s).count()
    }

    /// 打开管理员验证弹窗（gpui-component Dialog + AdminView）
    pub fn open_admin(&mut self, cx: &mut gpui::Context<Self>) {
        self.admin_open = true;
        self.admin_password.clear();
        self.admin_error.clear();
        self.admin_countdown = 30;
        let Some(handle) = self.window_handle else {
            return;
        };
        let kiosk = cx.entity();
        handle
            .update(cx, |_, window, cx| {
                crate::ui::admin::open_admin_dialog(window, cx, &kiosk);
            })
            .ok();
    }

    /// 管理员弹窗状态清理（纯状态；框架 Dialog 的关闭由调用方在
    /// 有 Window 上下文的地方直接 window.close_dialog，倒计时归零路径
    /// 则经 window_handle 中转）
    pub fn close_admin(&mut self) {
        self.admin_open = false;
        self.admin_password.clear();
        self.admin_error.clear();
    }

    /// 管理员密码确认（逻辑照抄 App.vue confirmPasswordAction：按密码决定动作）。
    /// 匹配成功只清理弹窗状态，Dialog 本身由调用方（持有 Window）负责关闭。
    pub fn confirm_admin(&mut self) -> Option<AdminMatch> {
        let cfg = self.cfg();
        let input = self.admin_password.clone();
        let settings_pw = if cfg.terminal.exit_password.is_empty() {
            "1200".to_string()
        } else {
            cfg.terminal.exit_password.clone()
        };
        let minimize_pw = if cfg.terminal.minimize_password.is_empty() {
            "9900".to_string()
        } else {
            cfg.terminal.minimize_password.clone()
        };
        let log_pw = if cfg.terminal.log_password.is_empty() {
            "1600".to_string()
        } else {
            cfg.terminal.log_password.clone()
        };

        let matched = if input == settings_pw {
            Some(AdminMatch::Settings)
        } else if input == minimize_pw {
            Some(AdminMatch::Minimize)
        } else if input == log_pw {
            Some(AdminMatch::Logs)
        } else {
            None
        };

        if let Some(matched) = matched {
            self.close_admin();
            return Some(matched);
        }
        self.admin_password.clear();
        self.admin_error = "密码错误，请重新输入".to_string();
        None
    }

    // ==== 运行日志弹窗 ====

    pub fn open_logs(&mut self, cx: &mut gpui::Context<Self>) {
        self.logs = crate::domain::log::list_logs().unwrap_or_default();
        self.logs_open = true;
        if !self.logs.is_empty() {
            self.load_log(0);
        } else {
            self.log_content = String::new();
        }
        let kiosk = cx.entity();
        // 推迟到 spawn 打开：原因同 show_error（派发中嵌套更新会被吞）
        cx.spawn(async move |this, cx| {
            let _ = this.update(cx, |state, cx| {
                if let Some(handle) = state.window_handle {
                    let kiosk = kiosk.clone();
                    handle
                        .update(cx, |_, window, cx| {
                            crate::ui::logs::open_logs_dialog(window, cx, &kiosk);
                        })
                        .ok();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// 关闭运行日志弹窗（gpui-component Dialog 需经窗口句柄关闭）
    pub fn close_logs(&mut self, cx: &mut gpui::Context<Self>) {
        self.logs_open = false;
        if let Some(handle) = self.window_handle {
            handle
                .update(cx, |_, window, cx| {
                    window.close_dialog(cx);
                })
                .ok();
        }
        cx.notify();
    }

    pub fn load_log(&mut self, index: usize) {
        if let Some(name) = self.logs.get(index) {
            self.log_current = index;
            match crate::domain::log::read_log(name) {
                Ok(content) => {
                    // 大文件只保留尾部，避免一次性渲染过长文本
                    let lines: Vec<&str> = content.lines().collect();
                    let start = lines.len().saturating_sub(500);
                    self.log_content = lines[start..].join("\n");
                }
                Err(e) => self.log_content = e,
            }
        }
    }

    pub fn refresh_logs(&mut self, cx: &mut gpui::Context<Self>) {
        let current = self.logs.get(self.log_current).cloned().unwrap_or_default();
        self.logs = crate::domain::log::list_logs().unwrap_or_default();
        if let Some(index) = self.logs.iter().position(|n| *n == current) {
            self.load_log(index);
        } else if !self.logs.is_empty() {
            self.load_log(0);
        }
        cx.notify();
    }

    // ==== 触控键盘 ====

    /// 触控键盘输入（查询页 / 管理弹窗共用）+ 按键音 + 短暂高亮
    pub fn press_keyword_key(&mut self, key: &str, cx: &mut gpui::Context<Self>) {
        self.play_click(cx);
        match key {
            "清空" => {
                self.keyword.clear();
                self.keyword_selected = false;
            }
            "退格" => {
                if self.keyword_selected {
                    self.keyword.clear();
                    self.keyword_selected = false;
                } else {
                    self.keyword.pop();
                }
            }
            _ => {
                if self.keyword_selected {
                    self.keyword = key.to_string();
                    self.keyword_selected = false;
                } else if self.keyword.len() < MAX_KEYWORD {
                    self.keyword.push_str(key);
                }
            }
        }
        self.pressed_key = Some(key.to_string());
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(300))
                .await;
            let _ = this.update(cx, |state, cx| {
                state.pressed_key = None;
                cx.notify();
            });
        })
        .detach();
    }

    /// 管理弹窗键盘输入
    pub fn press_admin_key(&mut self, key: &str, cx: &mut gpui::Context<Self>) {
        self.play_click(cx);
        match key {
            "清空" => self.admin_password.clear(),
            "退格" => {
                self.admin_password.pop();
            }
            _ => {
                if self.admin_password.len() < 4 {
                    self.admin_password.push_str(key);
                }
            }
        }
    }

    /// 按键音（配置 terminal.clickEnabled / clickVolume）
    pub fn play_click(&self, _cx: &gpui::Context<Self>) {
        let cfg = self.cfg();
        if cfg.terminal.click_enabled {
            crate::audio::play_click(cfg.terminal.click_volume);
        }
    }

    /// 大按钮按下反馈（与数字键盘一致的 300ms 按压高亮）
    pub fn press_action(&mut self, id: impl Into<String>, cx: &mut gpui::Context<Self>) {
        self.pressed_action = Some(id.into());
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(300))
                .await;
            let _ = this.update(cx, |state, cx| {
                state.pressed_action = None;
                cx.notify();
            });
        })
        .detach();
    }

    // ==== 查询 ====

    /// 查询可打印报告：后台线程执行真实 HTTP 请求，完成后更新状态
    pub fn submit_query(&mut self, cx: &mut gpui::Context<Self>) {
        if self.loading || self.keyword.trim().is_empty() {
            return;
        }
        self.play_click(cx);
        let cfg = self.cfg();
        if cfg.service.base_url.trim().is_empty() || cfg.service.org_id.trim().is_empty() {
            self.show_error(cx, "终端尚未完成配置，请联系工作人员！");
            self.reset_countdown();
            cx.notify();
            return;
        }
        self.loading = true;
        self.error = None;
        cx.notify();

        let keyword = self.keyword.trim().to_string();
        let task = cx
            .background_spawn(async move { crate::domain::query_reports_blocking(&cfg, &keyword) });
        // 加载遮罩至少展示 2 秒再切页，避免一闪而过
        let started = std::time::Instant::now();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let min_loading = std::time::Duration::from_secs(2);
            let elapsed = started.elapsed();
            if elapsed < min_loading {
                cx.background_executor()
                    .timer(min_loading - elapsed)
                    .await;
            }
            let _ = this.update(cx, |state, cx| {
                state.loading = false;
                match result {
                    Ok(list) => {
                        state.set_reports(list);
                        state.page = Page::Reports;
                        // 进入报告页：空闲倒计时重新开始；空态页固定 10 秒自动返回
                        if state.reports.is_empty() {
                            state.countdown = 10;
                        } else {
                            state.reset_countdown();
                        }
                        let cfg = state.cfg();
                        if !state.reports.is_empty() {
                            crate::audio::speak(crate::audio::VoiceKey::ReportsFound, &cfg);
                        }
                    }
                    Err(e) => {
                        // 仅查询出错时重置整体倒计时
                        state.show_error(cx, e);
                        state.reset_countdown();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ==== 打印 ====

    /// 打印选中的报告：后台线程真实打印，完成后回写状态
    pub fn print_selected(&mut self, cx: &mut gpui::Context<Self>) {
        if self.printing {
            return;
        }
        self.play_click(cx);
        let indexes: Vec<usize> = self
            .selected
            .iter()
            .enumerate()
            .filter(|(_, sel)| **sel)
            .map(|(i, _)| i)
            .collect();
        if indexes.is_empty() {
            return;
        }
        let cfg = self.cfg();
        let reports: Vec<ReportItem> = indexes
            .iter()
            .filter_map(|i| self.reports.get(*i).cloned())
            .collect();
        if reports.is_empty() {
            return;
        }
        self.printing = true;
        self.completed = false;
        crate::audio::stop_speaking();
        cx.notify();

        // 系统级「保存打印输出」对话框（XPS 等虚拟打印机会弹出）默认又小又偏角落，
        // 打印期间定时检测并放大居中；真实打印机不弹此对话框，循环为无害空转
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
                let printing = this.update(cx, |state, _| state.printing).unwrap_or(false);
                if !printing {
                    break;
                }
                let _ = crate::native_window::enlarge_system_save_dialog();
            }
        })
        .detach();

        let keyword = self.keyword.trim().to_string();
        let task = cx.background_spawn(async move {
            let mut last_err: Option<String> = None;
            let mut printed: Vec<ReportItem> = Vec::new();
            for report in reports {
                match crate::domain::print_report_blocking(&cfg, report.clone()) {
                    Ok(_) => {
                        printed.push(report.clone());
                        // 状态回写（结果只写日志）
                        crate::domain::update_print_status_blocking(
                            cfg.clone(),
                            vec![report.id.clone()],
                        );
                    }
                    Err(e) => {
                        last_err = Some(format!("{}：{e}", report.subject_name.clone()));
                        break;
                    }
                }
            }
            (printed, last_err, keyword, cfg)
        });
        cx.spawn(async move |this, cx| {
            let (printed, err, keyword, cfg) = task.await;
            let _ = this.update(cx, |state, cx| {
                state.printing = false;
                if let Some(e) = err {
                    // 用户在系统打印对话框取消：不算错误，静默返回报告页并重新计时
                    // （contains：取消标记可能被外层错误链包裹）
                    if e.contains(crate::domain::printer::PRINT_CANCELLED_ERR) {
                        crate::domain::log::info("print", "用户取消了打印");
                        state.reset_countdown();
                        cx.notify();
                        return;
                    }
                    state.show_error(cx, e);
                    state.reset_countdown();
                    cx.notify();
                    return;
                }
                if printed.is_empty() {
                    cx.notify();
                    return;
                }
                state.completed_count = printed.len() as u32;
                state.completed = true;
                state.success_countdown = 6;

                // 成功项从本地列表标记为已打印并取消选中
                let printed_ids: Vec<String> = printed.iter().map(|r| r.id.clone()).collect();
                for report in &mut state.reports {
                    if printed_ids.contains(&report.id) {
                        report.is_patient_print = Some(1);
                        report.patient_print_count += 1;
                    }
                }
                state.selected = vec![false; state.reports.len()];

                // 打印完成语音延迟 2 秒（留出出纸时间）
                cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(2))
                        .await;
                    let _ = this.update(cx, |state, _cx| {
                        crate::audio::speak(crate::audio::VoiceKey::PrintComplete, &state.cfg());
                    });
                })
                .detach();

                // 重新查询刷新列表状态（失败仅提示，不影响成功反馈）
                state.requery_reports(keyword, cfg, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// 打印完成后重新查询，刷新报告列表（对应 ReportsView printSelected 中的二次 queryReports）
    fn requery_reports(
        &mut self,
        keyword: String,
        cfg: crate::domain::config::AppConfig,
        cx: &mut gpui::Context<Self>,
    ) {
        let task = cx
            .background_spawn(async move { crate::domain::query_reports_blocking(&cfg, &keyword) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |state, cx| {
                match result {
                    Ok(list) => {
                        state.set_reports(list);
                    }
                    Err(e) => {
                        state.show_error(cx, format!("打印完成，但列表刷新失败：{e}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ==== 设置 ====

    /// 读取草稿文本字段
    pub fn draft_text(&self, key: &str) -> String {
        let cfg = &self.draft.config;
        match key {
            "hospital_name" => cfg.hospital_name.clone(),
            "terminal_code" => cfg.terminal_code.clone(),
            "report_notice" => cfg.terminal.report_notice.clone(),
            "input_hint" => cfg.terminal.input_hint.clone(),
            "base_url" => cfg.service.base_url.clone(),
            "org_id" => cfg.service.org_id.clone(),
            "api_key" => cfg.service.api_key.clone(),
            "secret_key" => cfg.service.secret_key.clone(),
            "exit_password" => cfg.terminal.exit_password.clone(),
            "minimize_password" => cfg.terminal.minimize_password.clone(),
            "log_password" => cfg.terminal.log_password.clone(),
            _ => String::new(),
        }
    }

    /// 写入草稿文本字段
    pub fn set_draft_text(&mut self, key: &str, value: String) {
        let cfg = &mut self.draft.config;
        match key {
            "hospital_name" => cfg.hospital_name = value,
            "terminal_code" => cfg.terminal_code = value,
            "report_notice" => cfg.terminal.report_notice = value,
            "input_hint" => cfg.terminal.input_hint = value,
            "base_url" => cfg.service.base_url = value,
            "org_id" => cfg.service.org_id = value,
            "api_key" => cfg.service.api_key = value,
            "secret_key" => cfg.service.secret_key = value,
            "exit_password" => cfg.terminal.exit_password = value,
            "minimize_password" => cfg.terminal.minimize_password = value,
            "log_password" => cfg.terminal.log_password = value,
            _ => {}
        }
    }

    /// 刷新系统打印机列表（后台线程）
    pub fn refresh_printers(&mut self, cx: &mut gpui::Context<Self>) {
        self.printers_loading = true;
        cx.notify();
        let task = cx.background_spawn(async move { crate::domain::list_printers() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |state, cx| {
                state.printers_loading = false;
                match result {
                    Ok(list) => state.printers = list,
                    Err(e) => {
                        crate::domain::log::warn("printer", &format!("打印机列表获取失败: {e}"));
                        state.printers = Vec::new();
                    }
                }
                // 设置页打开时，把新列表同步进框架 Select（set_items 需要
                // Window，故经窗口句柄在窗口上下文里执行）
                if state.page == Page::Settings {
                    if let (Some(handle), Some(select)) =
                        (state.window_handle, state.printer_select.clone())
                    {
                        let items = state.printer_select_items();
                        let index = state.printer_select_index();
                        handle
                            .update(cx, |_, window, cx| {
                                select.update(cx, |select, cx| {
                                    select.set_items(items, window, cx);
                                    select.set_selected_index(index, window, cx);
                                });
                            })
                            .ok();
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 选择默认打印机（下拉列表点击项 / 框架 Select 确认回调）
    pub fn select_printer(&mut self, name: Option<String>) {
        self.draft.config.print.default_printer = name.unwrap_or_default();
        self.reset_countdown();
    }

    /// 查询输入框全选 / 取消全选
    pub fn select_all_keyword(&mut self) {
        self.keyword_selected = !self.keyword.trim().is_empty();
    }

    /// 复制查询输入框内容（优先全选文本；未全选时复制整段查询编号）
    pub fn copy_keyword(&self, cx: &gpui::Context<Self>) {
        let text = self.keyword.trim().to_string();
        if !text.is_empty() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
    }

    /// 剪切查询输入框内容
    pub fn cut_keyword(&mut self, cx: &gpui::Context<Self>) {
        if self.keyword.trim().is_empty() {
            return;
        }
        self.copy_keyword(cx);
        self.keyword.clear();
        self.keyword_selected = false;
    }

    /// 粘贴系统剪贴板文本到查询输入框，沿用登记号 / 病历号字符限制
    pub fn paste_keyword(&mut self, cx: &mut gpui::Context<Self>) {
        let clipboard_text = cx
            .read_from_clipboard()
            .and_then(|item| item.text())
            .or_else(|| crate::clipboard::read_text());
        let Some(text) = clipboard_text else {
            return;
        };

        let mut value = if self.keyword_selected {
            String::new()
        } else {
            self.keyword.clone()
        };
        for ch in text.to_uppercase().chars() {
            if value.chars().count() >= MAX_KEYWORD {
                break;
            }
            if ch.is_ascii_alphanumeric() || ch == '-' {
                value.push(ch);
            }
        }
        self.keyword = value;
        self.keyword_selected = false;
    }

    /// 校验密码规则（对应 SettingsDialog save：留空用默认，填写须 4 位数字，三者不能相同）
    pub fn validate_passwords(&self) -> Result<(), String> {
        let terminal = &self.draft.config.terminal;
        let defaults = ("1200", "9900", "1600");
        for (label, value, _default) in [
            ("设置密码", &terminal.exit_password, defaults.0),
            ("最小化密码", &terminal.minimize_password, defaults.1),
            ("查看日志密码", &terminal.log_password, defaults.2),
        ] {
            if !value.is_empty() && (value.len() != 4 || !value.chars().all(|c| c.is_ascii_digit()))
            {
                return Err(format!("{label}留空时使用默认密码，填写时必须是 4 位数字"));
            }
        }
        let effective = [
            if terminal.exit_password.is_empty() {
                defaults.0
            } else {
                terminal.exit_password.as_str()
            },
            if terminal.minimize_password.is_empty() {
                defaults.1
            } else {
                terminal.minimize_password.as_str()
            },
            if terminal.log_password.is_empty() {
                defaults.2
            } else {
                terminal.log_password.as_str()
            },
        ];
        if effective[0] == effective[1]
            || effective[0] == effective[2]
            || effective[1] == effective[2]
        {
            return Err("设置密码、最小化密码和查看日志密码（含留空时的默认密码）不能相同".into());
        }
        Ok(())
    }

    /// 保存设置草稿（含密码校验与日志设置生效）
    pub fn save_draft(&mut self, cx: &mut gpui::Context<Self>) {
        if let Err(e) = self.validate_passwords() {
            self.show_error(cx, e);
            cx.notify();
            return;
        }
        let mut config = self.draft.config.clone();
        // 全屏设置下次启动生效（与 Vue 版一致），这里直接保存
        config.config_version = self.cfg().config_version;
        crate::domain::log::apply_settings(
            &config.terminal.log_dir,
            config.terminal.log_retention_days,
        );
        match self.config.set(config) {
            Ok(()) => {
                // 保存成功立即备份一份（内容相同则自动跳过）
                crate::domain::config::backup_config_file(&crate::paths::config_file_path(), 30);
                self.save_notice = Some("配置已保存".to_string());
                crate::domain::log::info("config", "终端配置已保存（GPUI 原型）");
            }
            Err(e) => {
                // 写入失败（如目录无写权限）：明确提示，避免"保存成功但重启丢失"
                self.show_error(cx, format!("配置保存失败：{e}"));
                crate::domain::log::error("config", &format!("终端配置保存失败: {e}"));
            }
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            let _ = this.update(cx, |state, cx| {
                state.save_notice = None;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// 打开设置页并装载草稿（重建 gpui-component 的 Input/Slider 实体）
    pub fn open_settings(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        self.draft = SettingsDraft {
            config: self.cfg(),
        };
        self.save_notice = None;
        self.page = Page::Settings;
        self.reset_countdown();

        // 文本字段：Input 实体（密码字段掩码），输入实时同步回 draft.config
        let mut inputs = std::collections::HashMap::new();
        for key in TEXT_FIELDS {
            let masked = is_password_field(key);
            let value = self.draft_text(key);
            let placeholder = if masked { "留空使用默认密码" } else { "" };
            inputs.insert(
                key,
                cx.new(|cx| {
                    InputState::new(window, cx)
                        .default_value(value)
                        .masked(masked)
                        .placeholder(placeholder)
                }),
            );
        }
        self.settings_inputs = inputs;

        // 滑动条：SliderState 实体（min/max/step 与配置档位一致）
        let cfg = &self.draft.config;
        let slider_specs: Vec<(&'static str, f32, f32, f32, f32)> = vec![
            (
                "request_timeout",
                1.,
                5.,
                1.,
                cfg.service.request_timeout_seconds as f32,
            ),
            ("voice_volume", 0., 100., 5., cfg.terminal.voice_volume as f32),
            (
                "voice_rate",
                0.6,
                1.3,
                0.1,
                ((cfg.terminal.voice_rate as f32) * 10.).round() / 10.,
            ),
            (
                "click_volume",
                0.,
                100.,
                5.,
                cfg.terminal.click_volume as f32,
            ),
            (
                "idle_timeout",
                30.,
                600.,
                10.,
                cfg.terminal.idle_timeout_seconds as f32,
            ),
            (
                "log_retention",
                1.,
                30.,
                1.,
                cfg.terminal.log_retention_days as f32,
            ),
        ];
        let mut sliders = std::collections::HashMap::new();
        for (key, min, max, step, value) in slider_specs {
            sliders.insert(
                key,
                cx.new(|_| {
                    SliderState::new()
                        .min(min)
                        .max(max)
                        .step(step)
                        .default_value(value)
                }),
            );
        }
        self.settings_sliders = sliders;

        // 打印机下拉：框架 Select 实体（首项=不指定，其余为检测到的打印机）
        let items = self.printer_select_items();
        let index = self.printer_select_index();
        self.printer_select = Some(cx.new(|cx| {
            gpui_component::select::SelectState::new(items, index, window, cx)
        }));

        // Logo 内置预设下拉：院徽 / 运营方各一个（选项 = 占位图 + 预设 + 自定义）
        let cfg = &self.draft.config;
        let logo_specs: Vec<(
            &'static str,
            &'static [crate::domain::logo::LogoPreset],
            String,
            String,
        )> = vec![
            (
                "hospital",
                crate::domain::logo::HOSPITAL_PRESETS,
                cfg.hospital_logo_preset.clone(),
                cfg.hospital_logo.clone(),
            ),
            (
                "footer",
                crate::domain::logo::OPERATOR_PRESETS,
                cfg.footer_logo_preset.clone(),
                cfg.footer_logo.clone(),
            ),
        ];
        for (kind, presets, preset, custom) in logo_specs {
            let items = crate::domain::logo::select_items(presets);
            let row = crate::domain::logo::select_row(presets, &preset, &custom);
            let entity = cx.new(|cx| {
                gpui_component::select::SelectState::new(
                    items,
                    Some(gpui_component::IndexPath::default().row(row)),
                    window,
                    cx,
                )
            });
            if kind == "hospital" {
                self.hospital_logo_select = Some(entity);
            } else {
                self.footer_logo_select = Some(entity);
            }
        }

        // 订阅：Input 变更 / Slider 拖动 → 实时写回草稿配置
        self.settings_subs.clear();

        // 订阅：打印机选择确认
        if let Some(select) = self.printer_select.clone() {
            self.settings_subs.push(cx.subscribe(
                &select,
                move |this, _entity, event: &gpui_component::select::SelectEvent<
                    Vec<String>,
                >,
                     _cx| {
                    match event {
                        gpui_component::select::SelectEvent::Confirm(Some(label)) => {
                            this.select_printer_by_label(label);
                        }
                        gpui_component::select::SelectEvent::Confirm(None) => {
                            this.select_printer(None);
                        }
                    }
                },
            ));
        }

        // 订阅：Logo 内置预设下拉确认（院徽 / 运营方共用一套写回语义）
        for (key, select, presets) in [
            (
                "logo",
                self.hospital_logo_select.clone(),
                crate::domain::logo::HOSPITAL_PRESETS,
            ),
            (
                "footer_logo",
                self.footer_logo_select.clone(),
                crate::domain::logo::OPERATOR_PRESETS,
            ),
        ] {
            let Some(select) = select else {
                continue;
            };
            self.settings_subs.push(cx.subscribe(
                &select,
                move |this, _entity, event: &gpui_component::select::SelectEvent<Vec<String>>, cx| {
                    if let gpui_component::select::SelectEvent::Confirm(Some(label)) = event {
                        this.play_click(cx);
                        match crate::domain::logo::apply_selection(presets, label) {
                            crate::domain::logo::LogoSelection::Default => match key {
                                "logo" => this.draft.config.hospital_logo_preset.clear(),
                                _ => this.draft.config.footer_logo_preset.clear(),
                            },
                            crate::domain::logo::LogoSelection::Preset(id) => match key {
                                "logo" => {
                                    this.draft.config.hospital_logo_preset = id.to_string()
                                }
                                _ => this.draft.config.footer_logo_preset = id.to_string(),
                            },
                            crate::domain::logo::LogoSelection::Custom => match key {
                                "logo" => this.draft.config.hospital_logo_preset.clear(),
                                _ => this.draft.config.footer_logo_preset.clear(),
                            },
                        }
                        cx.notify();
                    }
                },
            ));
        }
        for (key, entity) in self.settings_inputs.clone() {
            self.settings_subs.push(cx.subscribe(&entity, {
                move |this, entity, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = entity.read(cx).value().to_string();
                        this.set_draft_text(key, value);
                    }
                }
            }));
        }
        for (key, entity) in self.settings_sliders.clone() {
            self.settings_subs.push(cx.subscribe(&entity, {
                move |this, _entity, event: &SliderEvent, _cx| match event {
                    SliderEvent::Change(value) => this.apply_slider_value(key, *value),
                }
            }));
        }

        self.refresh_printers(cx);
        cx.notify();
    }

    /// 把滑动条数值写回草稿配置（按字段键名分发，档位与旧版一致）
    fn apply_slider_value(&mut self, key: &'static str, _state: SliderValue) {
        let value = _state.start();
        let cfg = &mut self.draft.config;
        match key {
            "request_timeout" => {
                cfg.service.request_timeout_seconds =
                    (value.round() as u32).clamp(1, 5);
            }
            "voice_volume" => {
                cfg.terminal.voice_volume = (value.round() as u32).clamp(0, 100);
            }
            "voice_rate" => {
                cfg.terminal.voice_rate =
                    (((value * 10.).round() / 10.) as f64).clamp(0.6, 1.3);
            }
            "click_volume" => {
                cfg.terminal.click_volume = (value.round() as u32).clamp(0, 100);
            }
            "idle_timeout" => {
                cfg.terminal.idle_timeout_seconds =
                    (value.round() as u32).clamp(30, 600);
            }
            "log_retention" => {
                cfg.terminal.log_retention_days = (value.round() as u32).clamp(1, 30);
            }
            _ => {}
        }
    }

    /// 打印机下拉条目：首项固定「不指定」，其余为检测结果展示名
    pub fn printer_select_items(&self) -> Vec<String> {
        let mut items = vec!["（不指定，使用系统默认打印机）".to_string()];
        items.extend(self.printers.iter().map(|p| {
            format!(
                "{}{}",
                p.display_name,
                if p.is_default { "（系统默认）" } else { "" }
            )
        }));
        items
    }

    /// 当前配置对应的下拉选中项（未指定 → 第 0 项）
    pub fn printer_select_index(&self) -> Option<gpui_component::IndexPath> {
        let current = &self.draft.config.print.default_printer;
        let row = if current.is_empty() {
            0
        } else {
            self.printers
                .iter()
                .position(|p| &p.name == current)
                .map(|i| i + 1)
                .unwrap_or(0)
        };
        Some(gpui_component::IndexPath::default().row(row))
    }

    /// 按下拉展示标签写回所选打印机
    pub fn select_printer_by_label(&mut self, label: &str) {
        if label.starts_with("（不指定") {
            self.select_printer(None);
            return;
        }
        if let Some(p) = self.printers.iter().find(|p| {
            format!(
                "{}{}",
                p.display_name,
                if p.is_default { "（系统默认）" } else { "" }
            ) == label
        }) {
            self.select_printer(Some(p.name.clone()));
        }
    }

    /// 清空设置页文本字段并同步输入框（语音/Logo「恢复默认」等）
    pub fn clear_settings_field(
        &mut self,
        key: &'static str,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.set_draft_text(key, String::new());
        if let Some(input) = self.settings_inputs.get(key) {
            input.update(cx, |state, cx| {
                state.set_value("", window, cx);
            });
        }
        cx.notify();
    }

    /// 将选择的文件复制到 app_data 指定子目录，返回保存的文件名
    pub fn import_app_file(&self, source: &std::path::Path, subdir: &str) -> Option<String> {
        let file_name = source.file_name()?.to_string_lossy().to_string();
        let dir = crate::paths::app_data_dir().join(subdir);
        std::fs::create_dir_all(&dir).ok()?;
        let target = dir.join(&file_name);
        std::fs::copy(source, &target).ok()?;
        Some(file_name)
    }

    /// 删除 app_data 指定子目录下的自定义文件（恢复默认）
    pub fn remove_app_file(&self, name: &str, subdir: &str) {
        if name.trim().is_empty() {
            return;
        }
        let _ = std::fs::remove_file(crate::paths::app_data_dir().join(subdir).join(name));
    }

    /// 文件选择（对应 window.api.logo/voice.select）：拷入 app_data 并写回草稿
    pub fn select_app_file(
        &mut self,
        subdir: &str,
        allowed: &[&str],
        cx: &mut gpui::Context<Self>,
    ) {
        let options = gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        };
        let receiver = cx.prompt_for_paths(options);
        let subdir = subdir.to_string();
        let allowed: Vec<String> = allowed.iter().map(|s| s.to_string()).collect();
        cx.spawn(async move |this, cx| {
            let picked = match receiver.await {
                Ok(Ok(Some(paths))) => Some(paths),
                _ => None,
            };
            let _ = this.update(cx, |state, cx| {
                if let Some(paths) = picked {
                    if let Some(path) = paths.first() {
                        let ext_ok = path
                            .extension()
                            .map(|e| {
                                let e = e.to_string_lossy().to_lowercase();
                                allowed.iter().any(|a| e == *a)
                            })
                            .unwrap_or(false);
                        if !ext_ok {
                            state.show_error(cx, format!("请选择 {} 格式的文件", allowed.join(" / ")));
                            cx.notify();
                            return;
                        }
                        // footer_logo 的文件同样存放在 logo/ 目录，仅字段不同
                        let import_dir = if subdir == "footer_logo" {
                            "logo".to_string()
                        } else {
                            subdir.clone()
                        };
                        match state.import_app_file(path, &import_dir) {
                            Some(name) => match subdir.as_str() {
                                // 自定义文件生效时清除内置预设（预设优先级更高）
                                "logo" => {
                                    state.draft.config.hospital_logo = name;
                                    state.draft.config.hospital_logo_preset.clear();
                                }
                                "footer_logo" => {
                                    state.draft.config.footer_logo = name;
                                    state.draft.config.footer_logo_preset.clear();
                                }
                                "voice" => match state.pending_voice_field {
                                    Some("voice_input") => {
                                        state.draft.config.terminal.voice_input = name
                                    }
                                    Some("voice_reports_found") => {
                                        state.draft.config.terminal.voice_reports_found = name
                                    }
                                    Some("voice_print_complete") => {
                                        state.draft.config.terminal.voice_print_complete = name
                                    }
                                    _ => {}
                                },
                                _ => {}
                            },
                            None => state.show_error(cx, "文件复制失败，请重试"),
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ==== 定时循环 ====

    /// 每秒循环：时钟刷新、空闲倒计时、各弹窗倒计时
    pub fn start_idle_timer(&self, cx: &mut gpui::Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                let _ = this.update(cx, |state, cx| {
                    state.now = chrono::Local::now();

                    // 错误弹窗 10 秒自动关闭（框架 Dialog 需经窗口句柄关闭）
                    if state.error.is_some() {
                        state.error_countdown = state.error_countdown.saturating_sub(1);
                        if state.error_countdown == 0 {
                            state.error = None;
                            if let Some(handle) = state.window_handle {
                                handle.update(cx, |_, window, cx| window.close_dialog(cx)).ok();
                            }
                        }
                    }

                    // 打印成功弹窗 6 秒自动关闭
                    if state.completed {
                        state.success_countdown = state.success_countdown.saturating_sub(1);
                        if state.success_countdown == 0 {
                            state.completed = false;
                            // 自动返回报告列表：空闲倒计时按配置值重新开始
                            state.reset_countdown();
                        }
                    }

                    if state.loading || state.printing {
                        cx.notify();
                        return;
                    }
                    if state.admin_open {
                        state.admin_countdown = state.admin_countdown.saturating_sub(1);
                        if state.admin_countdown == 0 {
                            state.close_admin();
                            if let Some(handle) = state.window_handle {
                                handle.update(cx, |_, window, cx| window.close_dialog(cx)).ok();
                            }
                        }
                        cx.notify();
                        return;
                    }
                    // 患者页面（查询/报告）无人操作自动回首页；
                    // 设置页是管理员配置界面，不参与空闲倒计时，避免配置中途被踢出
                    if matches!(state.page, Page::Search | Page::Reports) {
                        // 报告页打印中 / 打印成功反馈期间暂停空闲倒计时
                        //（打印可能耗时较长，不能把正在等待出纸的患者踢回首页）
                        let paused = state.page == Page::Reports
                            && (state.printing || state.completed);
                        if !paused {
                            state.countdown = state.countdown.saturating_sub(1);
                            if state.countdown == 0 {
                                if state.page == Page::Reports {
                                    // 报告页超时返回查询页（保留查询信息，方便重查），
                                    // 不直接回首页；查询页超时仍回首页
                                    state.page = Page::Search;
                                    state.reset_countdown();
                                } else {
                                    state.go_home();
                                }
                            }
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
}
