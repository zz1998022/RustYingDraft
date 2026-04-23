#!/usr/bin/env bash
set -euo pipefail

REPO="${YINGDRAFT_REPO:-zz1998022/RustYingDraft}"
VERSION="${YINGDRAFT_VERSION:-latest}"
INSTALL_DIR="${YINGDRAFT_INSTALL_DIR:-$HOME/.local/opt/YingDraft}"

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "缺少依赖命令: $1" >&2
    exit 1
  fi
}

detect_asset() {
  local os arch
  os="$(uname -s)"
  arch="$(uname -m)"

  case "$os" in
    Darwin)
      case "$arch" in
        arm64|aarch64) echo "yingdraft-macos-arm64.tar.gz" ;;
        x86_64|amd64) echo "yingdraft-macos-x64.tar.gz" ;;
        *) echo "暂不支持的 macOS 架构: $arch" >&2; exit 1 ;;
      esac
      ;;
    Linux)
      case "$arch" in
        x86_64|amd64) echo "yingdraft-linux-x64.tar.gz" ;;
        *) echo "暂不支持的 Linux 架构: $arch" >&2; exit 1 ;;
      esac
      ;;
    *)
      echo "当前安装脚本仅支持 macOS / Linux。Windows 请使用发布包中的 install-jy.ps1。" >&2
      exit 1
      ;;
  esac
}

download_url() {
  local asset="$1"
  if [[ "$VERSION" == "latest" ]]; then
    echo "https://github.com/$REPO/releases/latest/download/$asset"
  else
    echo "https://github.com/$REPO/releases/download/$VERSION/$asset"
  fi
}

need_cmd curl
need_cmd tar
need_cmd mktemp

asset="$(detect_asset)"
url="$(download_url "$asset")"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

echo "YingDraft CLI 安装器"
echo "仓库: $REPO"
echo "版本: $VERSION"
echo "平台包: $asset"
echo "安装目录: $INSTALL_DIR"
echo

echo "正在下载发布包..."
curl -fL --retry 3 --connect-timeout 15 "$url" -o "$tmp_dir/$asset"

echo "正在解压..."
tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"

bundle_dir="$(find "$tmp_dir" -maxdepth 2 -type f -path '*/scripts/install-jy.sh' -print -quit)"
if [[ -z "$bundle_dir" ]]; then
  echo "发布包结构不正确，未找到 scripts/install-jy.sh。" >&2
  exit 1
fi
bundle_dir="$(cd "$(dirname "$bundle_dir")/.." && pwd)"

chmod +x "$bundle_dir/scripts/install-jy.sh"
"$bundle_dir/scripts/install-jy.sh" "$INSTALL_DIR"

echo
echo "一条命令安装完成。"
echo "如果当前终端还不能直接执行 jy，请把下面这行加入 shell 配置："
echo '  export PATH="$HOME/.local/bin:$PATH"'
