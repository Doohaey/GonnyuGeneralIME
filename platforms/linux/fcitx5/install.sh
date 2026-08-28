#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
REPO_ROOT="$( cd "$SCRIPT_DIR/../../.." && pwd )"

if [ -d "$SCRIPT_DIR/payload" ]; then
  PAYLOAD_DIR="$SCRIPT_DIR/payload"
  INPUT_METHOD_CONFIG="$PAYLOAD_DIR/gannyu.conf"
  ADDON_CONFIG="$PAYLOAD_DIR/gannyu-addon.conf"
else
  PAYLOAD_DIR="${GANNYU_PACKAGE_ROOT:-$REPO_ROOT/build/linux-fcitx5}"
  INPUT_METHOD_CONFIG="$SCRIPT_DIR/gannyu.conf"
  ADDON_CONFIG="$PAYLOAD_DIR/gannyu-addon.conf"
fi

for required in "$PAYLOAD_DIR/libfcitx5-gannyu.so" "$INPUT_METHOD_CONFIG" "$ADDON_CONFIG"; do
  if [ ! -f "$required" ]; then
    echo "未找到安装文件 $required" >&2
    exit 1
  fi
done

PREFIX="${PREFIX:-/usr}"
ADDON_LIBDIR="$(pkg-config --variable=libdir Fcitx5Core 2>/dev/null || true)"
ADDON_LIBDIR="${ADDON_LIBDIR:-$PREFIX/lib}"

install_file() {
  local mode="$1" source="$2" destination="$3"
  if [ -n "${DESTDIR:-}" ] || [ "${EUID:-$(id -u)}" -eq 0 ]; then
    install -Dm"$mode" "$source" "${DESTDIR:-}$destination"
  else
    sudo install -Dm"$mode" "$source" "$destination"
  fi
}

echo "[1/2] 安装 fcitx5 addon 到系统"
install_file 755 "$PAYLOAD_DIR/libfcitx5-gannyu.so" "$ADDON_LIBDIR/fcitx5/libfcitx5-gannyu.so"
install_file 644 "$INPUT_METHOD_CONFIG" "$PREFIX/share/fcitx5/inputmethod/gannyu.conf"
install_file 644 "$ADDON_CONFIG" "$PREFIX/share/fcitx5/addon/gannyu.conf"

echo "[2/2] 安装完成"
echo "重启 fcitx5: fcitx5 -r 或注销重登。"
echo "fcitx5-configtool → 添加输入法 → 选「Gannyu Gan / 赣语」"
