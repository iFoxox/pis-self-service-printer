//! 构建脚本：Windows 下嵌入应用图标资源
//!
//! gpui 0.2.2 Windows 平台从本 exe 加载资源 ID=1 的 ICON 作为窗口/任务栏
//! 图标（见 gpui platform.rs load_icon()），因此图标资源 ID 必须为 1。

fn main() {
    #[cfg(target_os = "windows")]
    {
        // 图标文件变化时重新编译资源
        println!("cargo:rerun-if-changed=resources/app.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon_with_id("resources/app.ico", "1");
        res.compile()
            .expect("嵌入应用图标失败（检查 resources/app.ico 是否存在）");
    }
}
