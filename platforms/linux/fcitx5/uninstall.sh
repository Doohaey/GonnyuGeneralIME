#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

if [ -f "$SCRIPT_DIR/test_local.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . "$SCRIPT_DIR/test_local.env"
  set +a
fi

PREFIX="${PREFIX:-/usr}"
SUPPORT="$HOME/.local/share/gannyu-input"
ADDON_LIBDIR="$(pkg-config --variable=libdir Fcitx5Core 2>/dev/null)"
ADDON_LIBDIR="${ADDON_LIBDIR:-$PREFIX/lib}"

echo "[1/2] 卸载系统文件 (sudo)"
sudo rm -f "$ADDON_LIBDIR/fcitx5/libfcitx5-gannyu.so"
sudo rm -f "$PREFIX/lib/fcitx5/libfcitx5-gannyu.so"
sudo rm -f "$PREFIX/share/fcitx5/inputmethod/gannyu.conf"
sudo rm -f "$PREFIX/share/fcitx5/addon/gannyu.conf"

echo "[2/2] 移除资源链接 $SUPPORT/resources"
[ -L "$SUPPORT/resources" ] && rm -f "$SUPPORT/resources" || true

echo "重启 fcitx5: fcitx5 -r 或注销重登。"
