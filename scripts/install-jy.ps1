param(
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "YingDraft"),
    [switch]$AddToUserPath = $true
)

$ErrorActionPreference = "Stop"

# 这个脚本假设自己运行在标准发布包的 `scripts/` 目录中，
# 因此统一通过脚本位置反推发布包根目录，避免依赖当前工作目录。
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BundleRoot = Split-Path -Parent $ScriptDir
$SourceBinDir = Join-Path $BundleRoot "bin"
$SourceScriptsDir = Join-Path $BundleRoot "scripts"
$SourceDocsDir = Join-Path $BundleRoot "docs"

if (-not (Test-Path (Join-Path $SourceBinDir "jy_cli.exe"))) {
    Write-Error "未在发布包 bin 目录中找到 jy_cli.exe，无法继续安装。"
    exit 1
}

Write-Host "开始安装 YingDraft..."
Write-Host "发布包目录: $BundleRoot"
Write-Host "安装目录: $InstallDir"

$TargetBinDir = Join-Path $InstallDir "bin"
$TargetScriptsDir = Join-Path $InstallDir "scripts"
$TargetDocsDir = Join-Path $InstallDir "docs"
$TargetWorkDir = Join-Path $InstallDir "work"
$TargetLogsDir = Join-Path $InstallDir "logs"

New-Item -ItemType Directory -Force -Path `
    $InstallDir, `
    $TargetBinDir, `
    $TargetScriptsDir, `
    $TargetDocsDir, `
    $TargetWorkDir, `
    $TargetLogsDir | Out-Null

# 统一按目录复制，保持发布包结构稳定，后续脚本和文档都可以直接复用。
Copy-Item (Join-Path $SourceBinDir "*") $TargetBinDir -Recurse -Force
Copy-Item (Join-Path $SourceScriptsDir "*") $TargetScriptsDir -Recurse -Force
if (Test-Path $SourceDocsDir) {
    Copy-Item (Join-Path $SourceDocsDir "*") $TargetDocsDir -Recurse -Force
}
if (Test-Path (Join-Path $BundleRoot "README.md")) {
    Copy-Item (Join-Path $BundleRoot "README.md") (Join-Path $InstallDir "README.md") -Force
}

# 给当前用户提供一个更短的命令入口，后续手工执行或脚本调用都更方便。
$ShimPath = Join-Path $InstallDir "jy.ps1"
@"
`$env:PATH = "$TargetBinDir;`$env:PATH"
& "$TargetScriptsDir\run-jy.ps1" @args
exit `$LASTEXITCODE
"@ | Set-Content $ShimPath -Encoding UTF8

if ($AddToUserPath) {
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $segments = @()
    if (-not [string]::IsNullOrWhiteSpace($userPath)) {
        $segments = $userPath.Split(';', [System.StringSplitOptions]::RemoveEmptyEntries)
    }

    # 只把安装目录加到用户 PATH，避免写入过多细粒度目录。
    if ($segments -notcontains $InstallDir) {
        $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) {
            $InstallDir
        } else {
            "$userPath;$InstallDir"
        }
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        Write-Host "已将安装目录加入当前用户 PATH: $InstallDir"
    } else {
        Write-Host "安装目录已存在于当前用户 PATH 中。"
    }

    $env:PATH = "$InstallDir;$env:PATH"
}

Write-Host ""
Write-Host "安装完成。"
Write-Host "建议接着执行环境检查："
Write-Host "  & '$TargetScriptsDir\\check-env.ps1'"
Write-Host ""
Write-Host "常用调用方式："
Write-Host "  & '$TargetScriptsDir\\run-jy.ps1' --help"
Write-Host "  & '$ShimPath' --help"
Write-Host ""
Write-Host "如果还没有放入 ffprobe.exe，请把它放到："
Write-Host "  $TargetBinDir"
