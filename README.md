# PIS 病理报告自助打印终端（GPUI 版）

> 面向院内触控自助机的病理报告自助打印终端，基于 **GPUI（Rust 原生 UI 框架）** 构建，纯 Rust 实现，无 WebView 依赖。

<div align="center">

![Rust](https://img.shields.io/badge/Rust-2024-dea584?logo=rust)
![GPUI](https://img.shields.io/badge/GPUI-0.2-24c8db)
![License](https://img.shields.io/badge/License-Private-red)

</div>

## 功能特性

- **自助查询**：首页 → 查询页（触控数字键盘 / 扫码枪 / 实体键盘）→ 报告选择 → 打印，超时自动返回首页。
- **报告打印**：base64 PDF 解码 → 系统打印队列（Windows 直调 Win32 打印 API，macOS/Linux `lp`），支持 A4/A5、横纵向。
- **PIS 接口调用**：签名（HMAC-SHA256）、鉴权（Pis-Api-Key）与 HTTP 请求全部在 Rust 后端完成（tokio 运行时桥接）。
- **终端设置**：隐藏快捷键打开设置面板，配置 PIS 接口、打印机、语音、密码与全屏行为。
- **品牌可配置**：顶部院徽与页脚运营方 Logo 支持内置预设下拉选择（新增内置图见 `gpui-poc/src/domain/logo.rs`），也可从本地文件选择自定义图片，未配置时使用内置占位图。
- **管理员功能**：页脚 Logo 长按 2.5 秒进入验证，验证通过后可打开设置面板、查看运行日志、最小化窗口。
- **语音提示**：内置 MP3 语音（`gpui-poc/resources/assets/voice/`），支持开关与音量调节。
- **配置持久化**：`app-config.json`（系统应用数据目录），默认模板随安装包内置。
- **日志系统**：按天文件日志（`logs/app-YYYY-MM-DD.log`），启动时自动清理过期日志。

## 技术栈

| 类别      | 技术                     | 说明                                     |
| --------- | ------------------------ | ---------------------------------------- |
| 桌面框架  | GPUI 0.2                 | Rust 原生 UI（crates.io 正式版）         |
| 后端语言  | Rust 2024                | 配置 / 打印 / PIS 接口 / 日志全部在后端  |
| 异步      | tokio                    | 领域层 HTTP 请求（经内置 runtime 桥接）  |
| HTTP 客户端 | reqwest + rustls      | 调用 PIS 接口（HMAC-SHA256 签名）        |
| 音频      | rodio                    | 按键音效与 MP3 语音播报                  |
| Windows   | windows crate            | 打印（XPS/Win32）等系统能力              |

## 目录结构

```
pis-self-service-printer/
├── gpui-poc/                  # GPUI 桌面端（cargo 工程）
│   ├── src/
│   │   ├── main.rs            # 入口（窗口 + 页面状态机）
│   │   ├── paths.rs           # 数据目录 / 资源目录解析
│   │   ├── state.rs           # KioskState（页面/查询/打印状态）
│   │   ├── theme.rs / widgets.rs / icons.rs
│   │   ├── audio.rs           # 按键音效与语音播报
│   │   ├── domain/            # 领域层：config / pis / printer / report / log / syscmd
│   │   └── ui/                # home / search / reports / settings / admin
│   ├── resources/
│   │   └── assets/            # 内置语音 mp3 与医院 Logo（include_bytes! 内嵌）
│   ├── Cargo.toml
│   └── README.md              # GPUI 原型详细说明
├── resources/
│   └── config/app-config.json # 默认配置模板（随安装包内置）
└── .github/workflows/
    └── build-gpui.yml         # Windows x64 构建与 Release 发布
```

## 环境要求

- **Rust 工具链**：stable（≥ 1.85，Cargo.toml 为 edition 2024）
- **操作系统**：Windows 10+ / macOS / Linux（目标终端为 Windows x64）
- **打印机**：终端需安装并配置好系统打印机（报告打印走系统打印队列）

## 快速开始

```bash
cargo run --manifest-path gpui-poc/Cargo.toml   # 开发运行（全屏无边框窗口）
cargo build --release --manifest-path gpui-poc/Cargo.toml   # 生产构建
```

> 首次 `cargo build` 需要下载并编译数百个 Rust 依赖（含 GPUI），耗时较长属正常现象。

## 配置说明

配置在终端「设置」面板中维护，保存后写入系统应用数据目录下的 `app-config.json`：

- Windows: `%APPDATA%\com.pis.report.kiosk\`
- macOS: `~/Library/Application Support/com.pis.report.kiosk/`

首次运行若不存在配置文件，会以 `resources/config/app-config.json` 为模板初始化（开发模式回退到工程根 `resources/`）。

可配置项：

| 模块     | 字段                     | 说明                                   |
| -------- | ------------------------ | -------------------------------------- |
| 终端信息 | hospitalName / hospitalLogo / footerLogo / terminalCode | 页面展示名称、顶部院徽与页脚运营方 Logo（图片文件名）、终端编号 |
| PIS 接口 | baseUrl / orgId / apiKey / secretKey / requestTimeoutSeconds | 查询与状态回写接口参数 |
| 打印     | defaultPrinter / paper / orientation / allowReprint | 报告打印参数与重复打印开关 |
| 终端     | fullscreen / idleTimeoutSeconds / autoSelectReports / 三段密码 / 语音 / reportNotice / logDir / logRetentionDays | 运行与安全行为；提示音可替换，不配置用内置语音 |

## 快捷键

- 打开管理验证（设置）：`Ctrl + Alt + S`
- 最大化 / 还原窗口（全屏切换）：`Ctrl + Alt + F`
- 退出应用：`Ctrl + Alt + Q`
- 长按页脚运营方 Logo（2.5 秒）弹出管理员验证

## 接口实现

- 查询：`POST /{orgId}/query/patient/print`
- 状态回写：`POST /update/patient/print/status`
- 鉴权请求头：`Pis-Api-Key`
- 签名：过滤 `null`、排除 `pisDataSignature`、按 key 的 ASCII 字典序拼接，再以 Secret Key 执行 HMAC-SHA256 并输出 Base64（Rust 后端实现）

## 相关文档

- [GPUI 原型说明](gpui-poc/README.md) —— 已实现功能、已知限制与结构说明

## License

Private（内部项目）
