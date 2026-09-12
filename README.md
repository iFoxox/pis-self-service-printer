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

- Windows: `%APPDATA%\com.pis.report.kiosk\config\`
- macOS: `~/Library/Application Support/com.pis.report.kiosk/config/`

首次运行优先迁移安装目录 `config/app-config.json`，其次迁移旧数据目录根部的 `app-config.json`；均不存在时使用安装目录 `config/app-config.example.json` 初始化。新配置已存在时不再导入旧文件，也不受模板更新影响。开发构建使用独立的 `debug-config` 目录，仅首次初始化读取开发模板。

### 手工编辑配置

配置页右上角提供“导入配置文件”，可选择 UTF-8 编码的 JSON 配置（支持 BOM）。导入按完整字段路径与当前表单草稿合并：已知项使用导入值，缺失或未知项不覆盖；空字符串、纯空白字符串和 `null` 保留原值，`false`、`0` 按有效值导入。类型错误、非法 JSON 或不支持的配置版本会拒绝整份导入，保留原草稿。导入后表单显示合并结果，点击“保存配置”才写入运行配置并沿用现有备份流程；未保存时点击“关闭页面”可放弃导入。配置中的图片和语音文件名只作为配置值导入，不复制资源文件。

设置页显示实际配置文件的完整路径，并提供“打开配置目录”按钮。安装包与 ZIP 的程序目录也提供 `打开配置目录.cmd`，以当前 Windows 账号打开同一个目录；软件运行时也可使用该入口。入口只打开目录，不启动终端业务或初始化配置，首次安装请先正常启动软件。

先打开目录，再退出终端，编辑真实 `app-config.json`，保存后重新启动。避免软件运行期间手工修改后又在设置页保存，覆盖外部修改。安装目录 `config/config-location.txt` 也说明文件用途：示例模板仅供初始化、迁移归档只供回退，均不是当前运行配置。入口使用 EXE 的 `--open-config-dir` 参数；macOS 开发构建会打开隔离的开发配置目录。

### 升级与恢复

- 使用同一个 Windows 终端账号运行新旧版本（配置及回写队列按账号保存）。等待打印结束，通过应用正常退出后运行新版安装器。安装器检测到终端仍在运行时要求先退出，不强制关闭。
- 安装包和 ZIP 仅分发 `app-config.example.json`，不会覆盖旧运行配置。ZIP 升级也必须先退出应用，再解压到原目录；不要先卸载旧版。
- 配置结构由 `configVersion` 管理，目前为 1；缺失字段使用默认值，已有现场值保留。未来结构变化需添加明确的逐版本迁移。损坏配置、类型错误或高于程序支持的版本会阻止启动，Windows 弹窗提示并记录日志，不回退默认值运行。
- 迁移及每次设置保存前，在新配置同目录的 `config-history` 保存原文件快照；失败则停止写入。新文件校验后使用同目录临时文件同步并原子替换，成功后才更新内存。新配置和备份写入成功后，旧配置改名为 `app-config.migrated.json`，并在旁边生成 `config-migration.txt` 标明真实配置路径；归档已存在或目录无写权限时保留旧文件、记录日志并弹窗提示，不覆盖已有归档。
- 周期备份仍位于数据目录 `config-backups`，最多 30 份；`config/config-history` 快照不自动清理，由运维按需归档。备份包含现场凭据，应按配置文件相同权限管理。
- 回写队列仍使用数据目录中的 `print-jobs.jsonl`，升级不移动或清空它。
- 恢复时先退出程序，保留故障配置，将需要的历史快照复制回 `config/app-config.json`。回退旧程序必须配套兼容的配置；切回使用安装目录配置的旧版时，应恢复到旧版路径。此版本不提供自动程序回滚。
- 卸载新版保留数据目录；旧版安装器可能删除它原先安装的配置，因此首次迁移前应覆盖升级并另行备份，避免先卸载。


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

## 打印结果与状态回写

- 批量任务按份提交，遇到失败或取消后停止；已提交项仍标为已打印并取消选中，重试只处理剩余项。
- “已提交”表示作业已交给系统打印队列，不表示设备已经出纸。Windows 保存作业编号并检查页面、作业结束结果；作业开始后的异常可能已有部分出纸，不再自动切换 PDF 引擎整份重打。固定两秒后播放的取报告语音已停用，语音配置仍保留兼容。
- 本地打印记录和待回写队列存放在应用数据目录的 `print-jobs.jsonl`，与安装目录配置分开。每次提交前先持久化记录；存储失败时停止新的打印。账本不保存 PDF、患者查询号码或接口密钥。
- 回写由独立线程执行，不阻塞下一份报告打印。接口每次调用都会累加次数，因此只有明确发生在发送前的连接失败才自动退避重试；HTTP 错误、响应丢失、发送后超时、未确认响应均转为 `Review`，禁止自动重发。HTTP 客户端自动重试和重定向也已禁用。
- 重启后恢复 `Pending`；中断的 `Reserved` / `Sending` 转为待核对。账本不完整或损坏时保留原文件并停止打印，不能通过删除账本来恢复使用，否则会丢失重复打印保护。
- 查询结果会与本地已提交记录合并，服务端旧状态不会重新开放打印。待回写或待核对项即使开启补打也暂时不可打印；回写已确认后遵循补打设置。正常查询若发现服务端次数已达到待核对回写记录的预期值，会自动确认该记录，不再调用累加接口。
- 日志中的 `print-outbox` 会记录需要人工核对的记录编号与报告 ID；Windows 打印异常日志还包含作业编号。若服务端次数仍未更新或设备输出不确定，应由工作人员核实实际出纸及 PIS 状态后处理，不要盲目重发或整批重打。

故障回归测试：`cargo test --manifest-path gpui-poc/Cargo.toml --locked`。新增测试覆盖批量部分成功/取消、落盘失败、网络发送前后失败、恢复、旧查询覆盖和机构隔离；Windows 真机还需验证多页打印、缺纸/断连、取消及驱动异常。

## 相关文档

- [GPUI 原型说明](gpui-poc/README.md) —— 已实现功能、已知限制与结构说明

## License

Private（内部项目）
