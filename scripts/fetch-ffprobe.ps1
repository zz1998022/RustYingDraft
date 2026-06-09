# 下载固定版本的 Windows ffprobe.exe 并校验 SHA256，写入
# app/src-tauri/binaries/ffprobe-<rust-target-triple>.exe，供 Tauri externalBin 打包使用。
# 本地开发和 CI 构建前都先跑一次。
#
# 固定版本 + hash 校验是为了让 release 可复现、控制供应链风险。
# 来源、版本与许可证见 README「桌面端自带 ffprobe」一节，为 FFmpeg 的 GPL 静态构建。
$ErrorActionPreference = "Stop"

# gyan.dev FFmpeg 8.1.1 essentials 构建。
$Url = "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.1-essentials_build.zip"
$ExpectedSha256 = "0fde260f5abd35c9cafd96f594cc76365a780c1b73a90e35b6a3409ea1db1bf0"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir
$destDir = Join-Path $repoRoot "app/src-tauri/binaries"
New-Item -ItemType Directory -Force -Path $destDir | Out-Null

$hostLine = (& rustc -Vv | Select-String '^host: ')
if (-not $hostLine) { throw "无法解析 rustc host triple，请确认已安装 Rust。" }
$triple = $hostLine.ToString().Replace('host: ', '').Trim()

$work = Join-Path $env:TEMP ("ffprobe_" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $work | Out-Null
try {
  $zip = Join-Path $work "ffmpeg.zip"
  Invoke-WebRequest -Uri $Url -OutFile $zip
  Expand-Archive -Path $zip -DestinationPath $work -Force

  $src = Get-ChildItem -Path $work -Recurse -Filter ffprobe.exe | Select-Object -First 1
  if (-not $src) { throw "未能在下载内容中找到 ffprobe.exe" }

  $actual = (Get-FileHash -Algorithm SHA256 $src.FullName).Hash.ToLower()
  if ($actual -ne $ExpectedSha256) {
    throw "ffprobe SHA256 校验失败，已中止：`n  来源: $Url`n  期望: $ExpectedSha256`n  实际: $actual"
  }

  $out = Join-Path $destDir "ffprobe-$triple.exe"
  Copy-Item -Path $src.FullName -Destination $out -Force
  Write-Host "已写入 $out (sha256 校验通过)"
} finally {
  Remove-Item -Recurse -Force $work
}
