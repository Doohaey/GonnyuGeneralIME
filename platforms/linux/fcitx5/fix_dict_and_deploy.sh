#!/usr/bin/env bash
# fix_dict_and_deploy.sh — 词典整理 + 本地 Fcitx5 完整部署测试
#
# 执行顺序：
#   1. 整理官话词属性（fix_official_mandarin_field.py）
#   2. 编译并安装 Fcitx5 插件（build_and_install.sh，需 sudo）
#   3. 重启 Fcitx5
#
# 测试环境：Ubuntu + Fcitx5
# 用法：bash platforms/linux/fcitx5/fix_dict_and_deploy.sh
set -euo pipefail

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
REPO_ROOT="$( cd "$SCRIPT_DIR/../../.." && pwd )"

# ── 颜色辅助 ─────────────────────────────────────────────────────────────────
_green()  { printf '\033[0;32m%s\033[0m\n' "$*"; }
_yellow() { printf '\033[0;33m%s\033[0m\n' "$*"; }
_red()    { printf '\033[0;31m%s\033[0m\n' "$*"; }
_step()   { echo; printf '\033[1;34m=== %s ===\033[0m\n' "$*"; echo; }

# ── 步骤一：整理官话词属性 ────────────────────────────────────────────────────
_step "步骤 1/3  整理词典官话词属性（fix_official_mandarin_field.py）"
python3 "$REPO_ROOT/tools/fix_official_mandarin_field.py"

# ── 步骤二：编译 + 安装 ───────────────────────────────────────────────────────
_step "步骤 2/3  编译并安装 Fcitx5 插件（sudo 安装时将提示输入密码）"
bash "$SCRIPT_DIR/build_and_install.sh"

# ── 步骤三：重启 Fcitx5 ───────────────────────────────────────────────────────
_step "步骤 3/3  重启 Fcitx5"
if ! command -v fcitx5 >/dev/null 2>&1; then
    _yellow "警告：未找到 fcitx5 命令，请手动重启。"
elif fcitx5 -r 2>/dev/null; then
    _green "fcitx5 已接收重启信号。"
else
    _yellow "fcitx5 未在运行，正在后台启动..."
    fcitx5 -d --replace &>/dev/null &
    sleep 1
    if pgrep -x fcitx5 >/dev/null; then
        _green "fcitx5 已启动。"
    else
        _red "fcitx5 启动失败，请手动运行：fcitx5 -d"
    fi
fi

echo
_green "✓ 全流程完成。如需配置输入法，请打开 fcitx5-configtool → 添加输入法 → 选「赣语输入法」。"
