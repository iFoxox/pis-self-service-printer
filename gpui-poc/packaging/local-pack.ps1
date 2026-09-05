# 本地打包脚本：镜像 .github/workflows/build-gpui.yml 的流程
#
# 用法（在仓库任意位置）：
#   powershell -ExecutionPolicy Bypass -File gpui-poc/packaging/local-pack.ps1 [-Version 1.0.2]
#
# 产物：
#   - gpui-poc/dist/gpui-poc-kiosk-<Version>-windows-x64.zip  （exe + pdfium.dll + config 模板）
#   - 安装包（可选，仅当本机安装了 Inno Setup 6 才生成）
#
# 说明：pdfium.dll 不入库，缺失时会自动从官方 pdfium-binaries 下载。

param(
    [string]$Version = "dev-local"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot  # gpui-poc/

Push-Location $root
try {
    # ==== 1. Release 构建 ====
    Write-Host "== cargo build --release =="
    # rustup 的 shim 可能不在 PATH，自动补全工具链路径
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        $cargoHome = Join-Path $env:USERPROFILE ".cargo"
        $rustupHome = Join-Path $env:USERPROFILE ".rustup"
        $dirs = @(
            (Join-Path $cargoHome "bin"),
            (Join-Path (Join-Path (Join-Path $rustupHome "toolchains") "stable-x86_64-pc-windows-msvc") "bin")
        )
        foreach ($dir in $dirs) {
            if (Test-Path $dir) { $env:PATH = "$dir;$env:PATH" }
        }
    }
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "Release 构建失败" }
    $exe = Join-Path $root "target/release/pisSelfServicePrinter.exe"
    if (-not (Test-Path $exe)) { throw "构建产物缺失: $exe" }

    # ==== 2. pdfium.dll（缺失时下载官方构建）====
    $dll = Join-Path $root "target/release/pdfium.dll"
    if (-not (Test-Path $dll)) {
        Write-Host "== 下载 pdfium.dll =="
        $url = "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7881/pdfium-win-x64.tgz"
        $archive = Join-Path $env:TEMP "pdfium-win-x64.tgz"
        Invoke-WebRequest -Uri $url -OutFile $archive
        $extract = Join-Path $env:TEMP "pdfium-win-x64"
        New-Item -ItemType Directory -Force -Path $extract | Out-Null
        tar -xzf $archive -C $extract
        Copy-Item (Join-Path $extract "bin/pdfium.dll") $dll -Force
    }

    # ==== 3. 组包（exe + pdfium.dll + 内置配置模板）====
    Write-Host "== 组包 =="
    $stage = Join-Path $root "dist"
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $stage | Out-Null
    Copy-Item $exe $stage -Force
    Copy-Item $dll $stage -Force
    # 只随包分发生产模板；app-config-dev.json（含真实密钥）严禁入包
    $configDir = Join-Path $root ".." | Join-Path -ChildPath "resources"
    $configFile = Join-Path (Join-Path $configDir "config") "app-config.json"
    if (Test-Path $configFile) {
        New-Item -ItemType Directory -Force -Path (Join-Path $stage "config") | Out-Null
        Copy-Item $configFile (Join-Path (Join-Path $stage "config") "app-config.json") -Force
    }
    $zipName = "gpui-poc-kiosk-$Version-windows-x64.zip"
    $zipPath = Join-Path $root $zipName
    if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
    Compress-Archive -Path "$stage/*" -DestinationPath $zipPath -Force
    Move-Item $zipPath (Join-Path $stage $zipName) -Force
    Write-Output "打包完成: $stage/$zipName"

    # ==== 4. 安装包（可选，需 Inno Setup 6）====
    $iscc = "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
    if (-not (Test-Path $iscc)) { $iscc = "C:\Program Files\Inno Setup 6\ISCC.exe" }
    if (Test-Path $iscc) {
        Write-Host "== Inno Setup 打包 =="
        & $iscc "/DMyAppVersion=$Version" (Join-Path $root "packaging/setup.iss")
        if ($LASTEXITCODE -ne 0) { throw "ISCC 打包失败" }
        $setup = Get-ChildItem (Join-Path $root "target/release/dist") -Filter "PISReportKiosk-*-setup.exe" |
            Select-Object -First 1
        Write-Output "安装包完成: $($setup.FullName)"
    } else {
        Write-Output "未检测到 Inno Setup 6，跳过安装包（仅生成 zip）"
    }
} finally {
    Pop-Location
}
