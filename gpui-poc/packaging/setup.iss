; 病理全流程自助打印终端 - Windows 安装包脚本（Inno Setup 6）
; 编译：ISCC.exe packaging\setup.iss
; 产物：target\release\dist\PISReportKiosk-<版本>-setup.exe

#define MyAppName "病理全流程自助打印"
#define MyAppNameEn "PISReportKiosk"
#ifndef MyAppVersion
#define MyAppVersion "1.0.0"
#endif
#define MyAppExeName "pisSelfServicePrinter.exe"

[Setup]
AppId={{6D8C1A52-93F0-4B4E-9C2A-1E5F7A3B9D01}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
DefaultDirName={autopf}\{#MyAppNameEn}
DisableProgramGroupPage=yes
OutputDir=..\target\release\dist
OutputBaseFilename=PISReportKiosk-{#MyAppVersion}-setup
; 版本号可由 CI 传入：ISCC /DMyAppVersion=<版本>
SetupIconFile=..\resources\app.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
; 卸载时保留用户数据（%APPDATA%\com.pis.report.kiosk 与安装目录 logs）

[Languages]
; 语言文件随仓库分发（packaging/Languages/），CI 环境无需预装
Name: "chinesesimplified"; MessagesFile: "Languages\ChineseSimplified.isl"

[Tasks]
Name: "desktopicon"; Description: "创建桌面快捷方式(&D)"; GroupDescription: "附加任务:"; Flags: checkedonce

[Dirs]
; 安装时预创建运行日志目录（应用运行也会自动创建，这里保证装完即有）
Name: "{app}\logs"

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
; PDFium 随应用分发，避免目标机 Windows.Data.Pdf 异常时无法打印
Source: "..\target\release\pdfium.dll"; DestDir: "{app}"; Flags: ignoreversion
; 注意：app-config.json 即运行配置，升级重装前请先手动备份
Source: "..\..\resources\config\app-config.json"; DestDir: "{app}\config"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "立即运行 {#MyAppName}"; Flags: nowait postinstall skipifsilent
