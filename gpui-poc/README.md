# GPUI 桌面端（gpui-poc）

PIS 病理报告自助打印终端的 GPUI + Rust 桌面实现，基于 crates.io 上的 **gpui 0.2.2** 正式版构建，纯 Rust 原生 UI，无 WebView 依赖。

## 运行

在 gpui-poc 目录执行 cargo run（需 rustup stable >= 1.85）。全屏无边框窗口启动；配置复用 %APPDATA%\com.pis.report.kiosk\app-config.json；查询走真实 PIS 接口（HMAC 签名），打印走真实系统打印队列（实现位于 domain/ 模块）。

操作：首页开始查询 → 触控键盘/扫码枪输入 → 查询 → 勾选报告 → 打印。长按底部 Logo 2.5 秒或 Ctrl+Alt+S 进入管理验证（默认密码 1200）；Ctrl+Alt+F 切换全屏。

## Windows 打印

Windows 打印优先使用系统组件 `Windows.Data.Pdf`。若其加载、激活或渲染失败（例如目标机报 `0x80040154`），自动改用随安装包分发的 `pdfium.dll`（pdfium-render 0.9.3 绑定）光栅化，再通过 Win32 打印队列输出。开发调试时可将 DLL 放在应用可执行文件同目录，或用环境变量 `PIS_PDFIUM_PATH` 指向完整 DLL 路径。

## 已实现

全屏无边框窗口；页面状态机（首页/查询/报告/设置）；14 键触控键盘（18 位上限）；扫码枪/物理键盘拦截；空闲倒计时返回首页；真实查询（loading/错误弹窗）；报告多选 + 真实打印 + 成功态 + 状态回写；管理员验证（三套密码 + 30s 倒计时）；简化设置页（编辑保存）；按天日志与保留清理；Ctrl+Alt+F 全屏。

## 已知限制

1) 视觉简化复刻（无毛玻璃/渐变/跑马灯/动态字号/自绘光标/切换动画）；2) 未实现语音管理、日志查看页、Logo 管理、单实例、安装包；3) 中文依赖系统字体 fallback，无完整 IME；4) gpui executor 为 smol 系而 reqwest 需 tokio，domain/mod.rs 内置 tokio runtime 阻塞桥接；5) 点击用 on_mouse_down，无 hover 动效；6) gpui 0.2.2 交互事件需元素带 id 且 ElementId 不收动态 String，故用免 id 方案。

## 结构与依赖

main.rs（入口）/ paths.rs（数据目录）/ domain/（config/pis/printer/printer_win/report/log/syscmd + tokio 桥接门面）/ state.rs（KioskState）/ theme.rs / widgets.rs / icons.rs / audio.rs / ui/（home search reports settings admin）。

内置资源位于 resources/assets/（语音 mp3 与医院 Logo，经 include_bytes! 内嵌进二进制）。

依赖 gpui 0.2.2（crates.io 正式版）+ 领域层依赖（serde/reqwest/tokio/rodio/windows/pdfium-render）。未引入 gpui-component：依赖重且其 webview feature 会重新引入 WebView2，与「摆脱 WebView2」动因冲突。
