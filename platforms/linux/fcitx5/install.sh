#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
REPO_ROOT="$( cd "$SCRIPT_DIR/../../.." && pwd )"

ADDONS=()
if [ -d "$SCRIPT_DIR/payload" ]; then
  PAYLOAD_DIR="$SCRIPT_DIR/payload"
  for addon in gonnyu gonnyu-ipa; do
    if [ -f "$PAYLOAD_DIR/libfcitx5-$addon.so" ]; then
      ADDONS+=("$addon")
    fi
  done
else
  PAYLOAD_DIR="${GANNYU_PACKAGE_ROOT:-$REPO_ROOT/build/linux-fcitx5}"
  if [ "$#" -gt 0 ]; then
    ADDONS=("$@")
  else
    ADDONS=(gonnyu gonnyu-ipa)
  fi
fi

if [ "${#ADDONS[@]}" -eq 0 ]; then
  echo "未找到可安装的 fcitx5 addon" >&2
  exit 1
fi

for addon in "${ADDONS[@]}"; do
  for required in "$PAYLOAD_DIR/libfcitx5-$addon.so" "$PAYLOAD_DIR/$addon.conf" "$PAYLOAD_DIR/$addon-addon.conf"; do
    if [ ! -f "$required" ]; then
      echo "未找到安装文件 $required" >&2
      exit 1
    fi
  done
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
for addon in "${ADDONS[@]}"; do
  install_file 755 "$PAYLOAD_DIR/libfcitx5-$addon.so" "$ADDON_LIBDIR/fcitx5/libfcitx5-$addon.so"
  install_file 644 "$PAYLOAD_DIR/$addon.conf" "$PREFIX/share/fcitx5/inputmethod/$addon.conf"
  install_file 644 "$PAYLOAD_DIR/$addon-addon.conf" "$PREFIX/share/fcitx5/addon/$addon.conf"
done

echo "[2/2] 安装完成"
echo "重启 fcitx5: fcitx5 -r 或注销重登。"
echo "fcitx5-configtool → 添加输入法 → 选「GonnyuGeneralIME / 赣语（一式）」或「GonnyuGeneralIME IPA / 赣语（二式）」"
