#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN_DIR="$ROOT_DIR/bin"
CLI_PATH="$BIN_DIR/jy_cli"
LOCAL_FFPROBE="$BIN_DIR/ffprobe"

echo "检查发布目录: $ROOT_DIR"

if [[ ! -x "$CLI_PATH" ]]; then
  echo "未找到可执行的 jy_cli，请先确认 bin 目录中已放入 CLI 可执行文件。" >&2
  exit 1
fi

# 这里优先把发布包 bin 加入 PATH，尽量模拟真实部署时的运行环境。
export PATH="$BIN_DIR:$PATH"

echo "jy_cli 路径: $CLI_PATH"
"$CLI_PATH" --help | sed -n '1,5p'

if [[ -x "$LOCAL_FFPROBE" ]]; then
  echo "ffprobe 路径: $LOCAL_FFPROBE"
  "$LOCAL_FFPROBE" -version | sed -n '1p'
  exit 0
fi

if command -v ffprobe >/dev/null 2>&1; then
  echo "ffprobe 路径: $(command -v ffprobe)"
  ffprobe -version | sed -n '1p'
  exit 0
fi

echo "未找到 ffprobe。请将 ffprobe 放到 bin 目录，或提前安装到系统 PATH 中。" >&2
exit 1
