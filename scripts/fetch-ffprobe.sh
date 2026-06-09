#!/usr/bin/env bash
# 下载固定版本的 ffprobe 静态二进制并校验 SHA256，写入
# app/src-tauri/binaries/ffprobe-<rust-target-triple>，供 Tauri externalBin 打包使用。
# 本地开发和 CI 构建前都先跑一次。
#
# 固定版本 + hash 校验是为了让 release 可复现、控制供应链风险。
# 来源、版本与许可证见 README「桌面端自带 ffprobe」一节，均为 FFmpeg 的 GPL 静态构建。
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
dest_dir="$repo_root/app/src-tauri/binaries"
mkdir -p "$dest_dir"

triple="$(rustc -Vv | sed -n 's/^host: //p')"
if [[ -z "$triple" ]]; then
  echo "无法解析 rustc host triple，请确认已安装 Rust。" >&2
  exit 1
fi

case "$(uname -s)" in
  Darwin)
    # evermeet x86_64 静态构建，匹配 Intel 包 + Rosetta 的分发策略。
    url="https://evermeet.cx/ffmpeg/ffprobe-8.1.1.zip"
    expected_sha256="a976306bcb8c9c50b2ac4e91f5aac4e45395e1f9063c46aecf1e1213e41c631b"
    archive="ffprobe.zip"
    ;;
  Linux)
    # 用 BtbN 的 GPL 静态构建（GitHub CDN，CI 不会像 johnvansickle 那样拦截数据中心 IP）。
    url="https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-06-08-14-24/ffmpeg-n8.1.1-11-ge4c7fbf6c0-linux64-gpl-8.1.tar.xz"
    expected_sha256="bae7f38fe5dda21c35c168175795294eaa1005c36addcecee4b9b42c89d09e99"
    archive="ffmpeg.tar.xz"
    ;;
  *)
    echo "Windows 请改用 scripts/fetch-ffprobe.ps1" >&2
    exit 1
    ;;
esac

# macOS 默认没有 sha256sum，回退到 shasum。
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

curl -fsSL -o "$work/$archive" "$url"
case "$archive" in
  *.zip)    unzip -o -q "$work/$archive" -d "$work" ;;
  *.tar.xz) tar -xf "$work/$archive" -C "$work" ;;
esac

src="$(find "$work" -type f -name ffprobe | head -n1)"
if [[ -z "${src:-}" || ! -f "$src" ]]; then
  echo "未能在下载内容中找到 ffprobe 可执行文件。" >&2
  exit 1
fi

actual_sha256="$(sha256_of "$src")"
if [[ "$actual_sha256" != "$expected_sha256" ]]; then
  echo "ffprobe SHA256 校验失败，已中止：" >&2
  echo "  来源: $url" >&2
  echo "  期望: $expected_sha256" >&2
  echo "  实际: $actual_sha256" >&2
  exit 1
fi

out="$dest_dir/ffprobe-$triple"
install -m 0755 "$src" "$out"
echo "已写入 $out (sha256 校验通过)"
