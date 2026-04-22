$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir
$BinDir = Join-Path $RootDir "bin"
$CliPath = Join-Path $BinDir "jy_cli.exe"
$FfprobePath = Join-Path $BinDir "ffprobe.exe"

Write-Host "检查发布目录: $RootDir"

if (-not (Test-Path $CliPath)) {
    Write-Error "未找到 jy_cli.exe，请先确认 bin 目录中已放入 CLI 可执行文件。"
    exit 1
}

# 这里优先把发布包 bin 加入 PATH，尽量模拟真实部署时的运行环境。
$env:PATH = "$BinDir;$env:PATH"

Write-Host "jy_cli 路径: $CliPath"
& $CliPath --help | Select-Object -First 5

if (Test-Path $FfprobePath) {
    Write-Host "ffprobe 路径: $FfprobePath"
    & $FfprobePath -version | Select-Object -First 1
    exit 0
}

$ffprobeCmd = Get-Command ffprobe -ErrorAction SilentlyContinue
if ($null -ne $ffprobeCmd) {
    Write-Host "ffprobe 路径: $($ffprobeCmd.Source)"
    & $ffprobeCmd.Source -version | Select-Object -First 1
    exit 0
}

Write-Error "未找到 ffprobe。请将 ffprobe.exe 放到 bin 目录，或提前安装到系统 PATH 中。"
exit 1
