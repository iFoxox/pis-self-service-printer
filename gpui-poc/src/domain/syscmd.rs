//! 子进程启动辅助（平移自 src-tauri/src/syscmd.rs）
//!
//! GPUI 应用是 GUI 程序（无控制台），在 Windows 上启动控制台子进程
//! （如 powershell.exe）时系统默认会创建并显示一个新的控制台窗口，
//! 导致界面上弹出黑框一闪而过。
//! 统一通过 `silent()` 启动子进程，附加 CREATE_NO_WINDOW 标志避免弹窗。

/// 启动子进程；Windows 下附加 CREATE_NO_WINDOW，不弹出控制台窗口
#[cfg(target_os = "windows")]
pub fn silent(program: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut cmd = std::process::Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// 非 Windows 平台直接启动
#[cfg(not(target_os = "windows"))]
pub fn silent(program: &str) -> std::process::Command {
    std::process::Command::new(program)
}
