#!/usr/bin/env bash
set -euo pipefail

# 默认安装到当前用户目录，避免要求 sudo。
INSTALL_DIR="${1:-$HOME/.local/opt/YingDraft}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUNDLE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SOURCE_BIN_DIR="$BUNDLE_ROOT/bin"
SOURCE_SCRIPTS_DIR="$BUNDLE_ROOT/scripts"
SOURCE_DOCS_DIR="$BUNDLE_ROOT/docs"

if [[ ! -x "$SOURCE_BIN_DIR/jy_cli" ]]; then
  echo "未在发布包 bin 目录中找到可执行的 jy_cli，无法继续安装。" >&2
  exit 1
fi

echo "开始安装 YingDraft..."
echo "发布包目录: $BUNDLE_ROOT"
echo "安装目录: $INSTALL_DIR"

TARGET_BIN_DIR="$INSTALL_DIR/bin"
TARGET_SCRIPTS_DIR="$INSTALL_DIR/scripts"
TARGET_DOCS_DIR="$INSTALL_DIR/docs"
TARGET_WORK_DIR="$INSTALL_DIR/work"
TARGET_LOGS_DIR="$INSTALL_DIR/logs"

mkdir -p "$TARGET_BIN_DIR" "$TARGET_SCRIPTS_DIR" "$TARGET_DOCS_DIR" "$TARGET_WORK_DIR" "$TARGET_LOGS_DIR"

# 统一按目录复制，保持发布包结构稳定，后续脚本和文档都可以直接复用。
cp -R "$SOURCE_BIN_DIR/." "$TARGET_BIN_DIR/"
cp -R "$SOURCE_SCRIPTS_DIR/." "$TARGET_SCRIPTS_DIR/"
if [[ -d "$SOURCE_DOCS_DIR" ]]; then
  cp -R "$SOURCE_DOCS_DIR/." "$TARGET_DOCS_DIR/"
fi
if [[ -f "$BUNDLE_ROOT/README.md" ]]; then
  cp "$BUNDLE_ROOT/README.md" "$INSTALL_DIR/README.md"
fi

chmod +x "$TARGET_BIN_DIR/jy_cli" \
         "$TARGET_SCRIPTS_DIR/run-jy.sh" \
         "$TARGET_SCRIPTS_DIR/check-env.sh" \
         "$TARGET_SCRIPTS_DIR/install-jy.sh"

# 给当前用户放一个更短的命令入口，便于命令行和脚本复用。
mkdir -p "$HOME/.local/bin"
cat > "$HOME/.local/bin/jy" <<EOF
#!/usr/bin/env bash
export PATH="$TARGET_BIN_DIR:\$PATH"
exec "$TARGET_SCRIPTS_DIR/run-jy.sh" "\$@"
EOF
chmod +x "$HOME/.local/bin/jy"

echo
echo "安装完成。"
echo "建议接着执行环境检查："
echo "  \"$TARGET_SCRIPTS_DIR/check-env.sh\""
echo
echo "常用调用方式："
echo "  \"$TARGET_SCRIPTS_DIR/run-jy.sh\" --help"
echo "  jy --help"
echo
echo "如果还没有放入 ffprobe，请把它放到："
echo "  $TARGET_BIN_DIR"
echo
echo "如果当前 shell 还找不到 jy，请确认 ~/.local/bin 已在 PATH 中。"
