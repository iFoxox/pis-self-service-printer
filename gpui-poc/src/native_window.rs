//! 原生窗口辅助（Windows 无边框全屏 / macOS kiosk 隐藏 Dock 与菜单栏）
//!
//! 为什么不用 gpui 的真全屏（WindowBounds::Fullscreen / toggle_fullscreen）：
//! gpui 0.2.2 在高 DPI 显示器上真全屏时渲染缩放错乱——布局像素按 1:1 物理
//! 像素绘制，内容只占屏幕左上角 物理/DPI 区域（实测 2560×1440@150% 下
//! 内容仅 1707×960）。创建时与运行时切换两条路径均已实测复现。
//!
//! 方案：窗口保持"普通窗口"状态（gpui 走与最大化相同的正确渲染路径），
//! 用 Win32 直接把窗口调整为覆盖整个显示器的无边框尺寸；任务栏由
//! Windows 对前台全屏窗口的检测自动隐藏。

/// 把本进程的 gpui 主窗口设为覆盖所在显示器的无边框全屏
#[cfg(target_os = "windows")]
pub fn set_borderless_fullscreen() -> bool {
    let Some(hwnd) = find_main_window() else {
        crate::domain::log::warn("window", "未找到 gpui 主窗口（Zed::Window 类）");
        return false;
    };
    set_borderless_fullscreen_hwnd(hwnd)
}

/// 非 Windows 平台无 Win32，仅记日志（开发联调用，不影响其余功能）
#[cfg(not(target_os = "windows"))]
pub fn set_borderless_fullscreen() -> bool {
    crate::domain::log::warn("window", "无边框全屏仅支持 Windows，已忽略");
    false
}

/// 还原（脱离最大化）并调整为覆盖所在显示器的无边框尺寸
#[cfg(target_os = "windows")]
fn set_borderless_fullscreen_hwnd(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows::Win32::UI::WindowsAndMessaging::HWND_TOP;
    use windows::Win32::UI::WindowsAndMessaging::{
        SW_RESTORE, SWP_FRAMECHANGED, SWP_NOZORDER, SetWindowPos, ShowWindow,
    };

    unsafe {
        // 先还原出最大化状态，否则尺寸会被最大化布局接管（只覆盖工作区）
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            crate::domain::log::warn("window", "GetMonitorInfoW 失败");
            return false;
        }
        let rc = info.rcMonitor;
        let ok = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            rc.left,
            rc.top,
            rc.right - rc.left,
            rc.bottom - rc.top,
            SWP_NOZORDER | SWP_FRAMECHANGED,
        )
        .is_ok();
        if ok {
            crate::domain::log::info(
                "window",
                &format!(
                    "无边框全屏：{},{} - {},{}",
                    rc.left, rc.top, rc.right, rc.bottom
                ),
            );
        } else {
            crate::domain::log::warn("window", "SetWindowPos 全屏失败");
        }
        ok
    }
}

/// 把系统「保存打印输出」对话框（XPS Document Writer 等虚拟打印机打印时弹出）
/// 移到屏幕中央并放大到约 3/4 屏幕尺寸——系统默认把它丢在角落且很小。
/// 找到并调整成功返回 true（未找到返回 false，打印期间可反复调用）。
#[cfg(target_os = "windows")]
pub fn enlarge_system_save_dialog() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetSystemMetrics, HWND_TOP, SM_CXSCREEN, SM_CYSCREEN, SetWindowPos,
        SWP_NOZORDER,
    };

    // 中文 / 英文系统两种标题
    const TITLES: [&str; 2] = ["保存打印输出", "Save Print Output As"];
    let mut adjusted = false;
    for title in TITLES {
        let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            let hwnd = FindWindowW(PCWSTR::null(), PCWSTR(wide.as_ptr()));
            if let Ok(h) = hwnd {
                if h != HWND::default() {
                    let sw = GetSystemMetrics(SM_CXSCREEN);
                    let sh = GetSystemMetrics(SM_CYSCREEN);
                    let _ = SetWindowPos(
                        h,
                        Some(HWND_TOP),
                        sw / 8,
                        sh / 8,
                        sw - sw / 4,
                        sh - sh / 4,
                        SWP_NOZORDER,
                    );
                    adjusted = true;
                }
            }
        }
    }
    adjusted
}

/// 非 Windows 平台无系统保存对话框，静默忽略
#[cfg(not(target_os = "windows"))]
pub fn enlarge_system_save_dialog() -> bool {
    false
}

/// 按 gpui 的窗口类名 + 本进程 PID 定位主窗口
#[cfg(target_os = "windows")]
fn find_main_window() -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible,
    };
    use windows::core::BOOL;

    struct Ctx {
        pid: u32,
        hwnd: Option<HWND>,
    }
    let mut ctx = Ctx {
        pid: std::process::id(),
        hwnd: None,
    };

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = unsafe { &mut *(lparam.0 as *mut Ctx) };
        if ctx.hwnd.is_some() {
            return true.into();
        }
        let mut pid = 0u32;
        let _ = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid != ctx.pid {
            return true.into();
        }
        if !unsafe { IsWindowVisible(hwnd) }.as_bool() {
            return true.into();
        }
        let mut buf = [0u16; 64];
        let n = unsafe { GetClassNameW(hwnd, &mut buf) };
        let class = String::from_utf16_lossy(&buf[..n as usize]);
        if class == "Zed::Window" {
            ctx.hwnd = Some(hwnd);
        }
        true.into()
    }

    let lparam = LPARAM(&mut ctx as *mut Ctx as isize);
    unsafe { EnumWindows(Some(enum_proc), lparam) }.ok()?;
    ctx.hwnd
}

/// macOS kiosk：应用激活期间自动隐藏 Dock 与菜单栏。
///
/// gpui 的整屏只是"窗口尺寸覆盖显示器"的普通窗口，系统不感知 kiosk 全屏，
/// Dock/菜单栏会正常显示；这里用 AppKit 的 presentationOptions 声明自动隐藏
/// （等价于 kiosk 应用的标准做法）。选 auto-hide 而非 hide：鼠标推到屏幕边缘
/// 仍可呼出，不会把用户困死；本应用失活或退出后系统自动还原，不影响日常开发。
#[cfg(target_os = "macos")]
pub fn auto_hide_system_bars() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationPresentationOptions};

    let Some(mtm) = MainThreadMarker::new() else {
        return; // 不在主线程（理论上 main 调用时必在），静默跳过
    };
    let app = NSApplication::sharedApplication(mtm);
    app.setPresentationOptions(
        NSApplicationPresentationOptions::AutoHideDock
            | NSApplicationPresentationOptions::AutoHideMenuBar,
    );
    crate::domain::log::info("window", "macOS kiosk：Dock 与菜单栏已设为自动隐藏");
}

/// Windows kiosk 由前台全屏窗口让系统自动隐藏任务栏，无需额外处理。
#[cfg(not(target_os = "macos"))]
pub fn auto_hide_system_bars() {}

/// 单实例保护：进程级互斥量，进程退出时由系统自动释放
///
/// 返回 true = 获得实例权（可继续启动）；false = 已有实例在运行，应立即退出。
/// 互斥量创建失败时放行启动（降级为旧行为，不因保护机制本身阻断终端）。
#[cfg(target_os = "windows")]
pub fn acquire_single_instance() -> bool {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
    use windows::Win32::System::Threading::CreateMutexW;

    const MUTEX_NAME: &str = "PisSelfServicePrinterSingleInstance";

    unsafe {
        let name: Vec<u16> = MUTEX_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        let _handle = match CreateMutexW(None, false, PCWSTR(name.as_ptr())) {
            Ok(handle) => handle,
            Err(e) => {
                crate::domain::log::warn("main", &format!("单实例互斥量创建失败（{e}），跳过保护继续启动"));
                return true;
            }
        };
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return false;
        }
        // HANDLE 是 Copy 且无 Drop：只要进程存活期间不调用 CloseHandle，
        // 互斥量内核对象就保持占用，进程退出时由系统统一回收
        true
    }
}

/// 非 Windows 平台不做单实例保护（kiosk 目标平台为 Windows）
#[cfg(not(target_os = "windows"))]
pub fn acquire_single_instance() -> bool {
    true
}

/// `cargo run` 启动的是裸可执行文件，没有 `.app` bundle / Info.plist 图标。
/// macOS 开发环境需要在 AppKit 里显式设置运行时图标，Dock 才不会显示默认图形。
#[cfg(target_os = "macos")]
pub fn set_app_icon() {
    use objc2::rc::Retained;
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    const APP_ICON_PNG: &[u8] = include_bytes!("../resources/assets/app-icon.png");

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };

    unsafe {
        let data = NSData::dataWithBytes_length(
            APP_ICON_PNG.as_ptr().cast(),
            APP_ICON_PNG.len() as objc2_foundation::NSUInteger,
        );
        let icon: Option<Retained<NSImage>> =
            objc2::msg_send![NSImage::alloc(), initWithData: &*data];
        let Some(icon) = icon else {
            crate::domain::log::warn("window", "解析应用图标失败");
            return;
        };

        NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&icon));
        crate::domain::log::info("window", "macOS 运行时应用图标已设置");
    }
}
