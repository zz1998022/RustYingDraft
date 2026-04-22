#!/usr/bin/env bash
set -euo pipefail

# 统一从脚本位置反推发布包根目录，避免调用方依赖特定的当前工作目录。
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BIN_DIR="$ROOT_DIR/bin"
CLI_PATH="$BIN_DIR/jy_cli"

if [[ ! -x "$CLI_PATH" ]]; then
  echo "未找到可执行的 jy_cli，请确认发布目录中的 bin 结构完整。" >&2
  exit 1
fi

# 把 bin 注入 PATH，确保 CLI 内部调用 ffprobe 时优先命中同包内版本。
export PATH="$BIN_DIR:$PATH"

exec "$CLI_PATH" "$@"
