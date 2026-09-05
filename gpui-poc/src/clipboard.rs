//! 系统剪贴板直读（绕过 gpui 0.2.2 Windows 下 read_from_clipboard
//! 偶发返回 None 的问题，见运行日志：Ctrl+V 事件正常但 gpui 读取失败）

/// 读取系统剪贴板中的纯文本（CF_UNICODETEXT）
///
/// 实测主线程（WndProc 内）调用 OpenClipboard 失败而外部线程成功，
/// 因此在独立线程读取；OpenClipboard 按官方建议在剪贴板被其他进程
/// 短暂占用时重试；每一步失败都记录 Win32 错误码便于诊断
#[cfg(target_os = "windows")]
pub fn read_text() -> Option<String> {
    std::thread::spawn(read_text_impl).join().ok().flatten()
}

#[cfg(target_os = "windows")]
fn read_text_impl() -> Option<String> {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EnumClipboardFormats, GetClipboardData, OpenClipboard,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    let mut opened = false;
    for attempt in 1..=5 {
        match unsafe { OpenClipboard(None) } {
            Ok(()) => {
                opened = true;
                break;
            }
            Err(e) => {
                crate::domain::log::warn(
                    "clipboard",
                    &format!("OpenClipboard 第 {attempt} 次失败: {e}"),
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
    if !opened {
        crate::domain::log::warn("clipboard", "OpenClipboard 连续 5 次失败，放弃");
        return None;
    }

    let result = (|| {
        // 直接取数据（不再先查格式：IsClipboardFormatAvailable 在本进程
        // 内曾出现误报 FALSE，GetClipboardData 结果才是权威）
        let handle = match unsafe { GetClipboardData(CF_UNICODETEXT.0 as u32) } {
            Ok(h) => h,
            Err(e) => {
                crate::domain::log::warn(
                    "clipboard",
                    &format!("GetClipboardData(CF_UNICODETEXT) 失败: {e}，枚举剪贴板实际格式"),
                );
                // 枚举剪贴板现有格式（返回 0 = 结束或出错）
                let mut fmt = 0u32;
                let mut seen = Vec::new();
                loop {
                    fmt = unsafe { EnumClipboardFormats(fmt) };
                    if fmt == 0 {
                        break;
                    }
                    seen.push(fmt);
                    if seen.len() > 64 {
                        break;
                    }
                }
                crate::domain::log::warn("clipboard", &format!("剪贴板现有格式: {:?}", seen));
                return None;
            }
        };
        let global = HGLOBAL(handle.0);
        let ptr = unsafe { GlobalLock(global) };
        if ptr.is_null() {
            crate::domain::log::warn("clipboard", "GlobalLock 返回空指针");
            return None;
        }
        let size = unsafe { GlobalSize(global) };
        let wide: Vec<u16> = unsafe { std::slice::from_raw_parts(ptr as *const u16, size / 2) }
            .iter()
            .take_while(|&&c| c != 0)
            .copied()
            .collect();
        let _ = unsafe { GlobalUnlock(global) };
        let text = String::from_utf16_lossy(&wide);
        crate::domain::log::info(
            "clipboard",
            &format!("直读成功：{} 字符", text.chars().count()),
        );
        Some(text)
    })();

    let _ = unsafe { CloseClipboard() };
    result.filter(|text| !text.is_empty())
}

/// 非 Windows 平台：无直读实现（返回 None，走 gpui 回退路径）
#[cfg(not(target_os = "windows"))]
pub fn read_text() -> Option<String> {
    None
}
