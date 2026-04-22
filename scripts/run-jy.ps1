param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CliArgs
)

# 统一从发布包根目录定位 `bin`，这样无论是手工双击脚本还是后端调用脚本，
# 都不需要再单独关心 PATH 或当前工作目录。
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir
$BinDir = Join-Path $RootDir "bin"
$CliPath = Join-Path $BinDir "jy_cli.exe"

if (-not (Test-Path $CliPath)) {
    Write-Error "未找到 jy_cli.exe，请确认发布目录中的 bin 结构完整。"
    exit 1
}

# 把 bin 注入 PATH，确保 CLI 内部调用 ffprobe 时优先使用同包内的版本。
$env:PATH = "$BinDir;$env:PATH"

& $CliPath @CliArgs
exit $LASTEXITCODE
