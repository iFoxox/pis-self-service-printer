//! GPUI 原型入口：全屏无边框窗口 + 初始化领域层 + 根视图（KioskState）
//! Windows 下一律隐藏控制台窗口（日志写 logs/ 目录文件，无需控制台输出）
#![windows_subsystem = "windows"]

use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};

mod audio;
mod clipboard;
mod domain;
mod icons;
mod native_window;
mod paths;
mod state;
#[allow(dead_code)] // gpui-0.2 部分自绘组件已被 gpui-component 取代，暂留对照
mod theme;
mod ui;
#[allow(dead_code)]
mod widgets;

use state::KioskState;

fn main() {
    // panic 钩子最先安装：release 为 panic=abort，不挂钩子则 panic 时
    // 进程静默消失、现场无任何线索；钩子在 abort 前把信息写入文件日志
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "非字符串 panic 载荷".to_string()
        };
        crate::domain::log::error("panic", &format!("{location} {payload}"));
    }));

    // 单实例保护：终端机上重复启动会叠出第二个全屏窗口，直接退出新进程
    #[cfg(target_os = "windows")]
    if !native_window::acquire_single_instance() {
        // 日志模块此时可用（写默认目录），仅提示一次便于现场排查
        crate::domain::log::warn("main", "检测到应用已在运行，本次启动退出");
        return;
    }

    Application::new()
        .with_assets(icons::Assets)
        .run(|cx: &mut App| {
            // macOS 下裸可执行文件没有 bundle 图标，Dock 需要运行时显式设置
            #[cfg(target_os = "macos")]
            crate::native_window::set_app_icon();

            // ==== 初始化领域层：配置存储 + 日志（平移自 src-tauri lib.rs setup） ====
            let data_dir = paths::app_data_dir();
            std::fs::create_dir_all(&data_dir).ok();
            // 单文件配置：安装目录 config\app-config.json（升级重装前需手动备份）
            let config_path = paths::config_file_path();
            std::fs::create_dir_all(config_path.parent().unwrap_or(&data_dir)).ok();

            // 内置模板：debug 联调读仓库 resources/config/app-config-dev.json
            // （dev 环境地址与测试凭据），release 包不传模板——安装器已把
            // app-config.json 放到安装目录 config/ 下作为用户配置直接读取
            let bundled_template = paths::bundled_template_path();
            let (store, load_info) = domain::config::ConfigStore::load(
                config_path.clone(),
                bundled_template.clone(),
                None,
            );
            let initial = store.get();

            // 首次运行把配置落盘（debug 已合并 dev 模板预置值），
            // 保证安装目录下始终存在 config\app-config.json
            if !config_path.exists() {
                let _ = store.save();
            }

            // 配置定时备份：启动时检查一次 + 每 30 分钟一次（内容变化才新增，
            // 滚动保留 30 份；备份在 %APPDATA% 下，重装/升级不触碰）
            let backup_src = config_path.clone();
            cx.spawn(async move |cx| {
                loop {
                    domain::config::backup_config_file(&backup_src, 30);
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(30 * 60))
                        .await;
                }
            })
            .detach();

            domain::log::apply_settings(
                &initial.terminal.log_dir,
                initial.terminal.log_retention_days,
            );
            if let Some(note) = load_info.warning {
                domain::log::warn("config", &note);
            }
            if let Some(fp) = load_info.managed_update {
                domain::log::info(
                    "config",
                    &format!(
                        "已应用内置配置模板 {}（指纹 {fp}）",
                        bundled_template
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    ),
                );
            }
            domain::log::info("main", "GPUI 原型启动（病理报告自助打印终端）");

            // 音频输出（按键音 + 语音播报，失败静默）
            audio::init();

            // gpui-component：组件初始化 + 医疗蓝主题（浅色）
            gpui_component::init(cx);
            theme::init_component_theme(cx);

            // 配置要求开机全屏：创建时就直接以整屏尺寸打开窗口
            // （gpui 0.2.2 Windows 高 DPI 下真全屏渲染缩放错乱——
            // WindowBounds::Fullscreen / toggle_fullscreen / 事后 Win32
            // SetWindowPos 三条路都实测失败，内容只渲染在左上角；
            // Windowed 整屏窗口由 gpui 自己初始化，与最大化同为正确路径）
            let restore_bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            let fullscreen_on_start = initial.terminal.fullscreen;
            let window_bounds = if fullscreen_on_start {
                cx.primary_display()
                    .map(|display| WindowBounds::Windowed(display.bounds()))
                    .unwrap_or(WindowBounds::Maximized(restore_bounds))
            } else {
                WindowBounds::Maximized(restore_bounds)
            };
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(window_bounds),
                    titlebar: None,
                    is_movable: false,
                    is_resizable: false,
                    ..Default::default()
                },
                |window, cx| {
                    // 空闲倒计时定时器在 KioskState::new 内启动
                    let view = cx.new(|cx| {
                        let mut state = KioskState::new(store.clone(), cx.focus_handle(), cx);
                        // 记录窗口句柄：框架 Dialog/Select 需要在 Window 上下文里开关
                        state.window_handle = Some(window.window_handle());
                        state
                    });
                    view.read(cx).focus_handle.focus(window);
                    // 窗口第一层视图必须是 gpui-component 的 Root（承载弹窗/通知层）
                    let root = cx.new(|cx| gpui_component::Root::new(view, window, cx));
                    root
                },
            )
            .expect("无法创建主窗口");
            cx.activate(true);
            // kiosk 配置要求全屏时，macOS 下同步隐藏 Dock 与菜单栏
            // （Windows 由 Win32 无边框全屏 + 前台全屏自动藏任务栏达成）
            if fullscreen_on_start {
                crate::native_window::auto_hide_system_bars();
            }
        });
}
