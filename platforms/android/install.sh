#!/usr/bin/env bash
# Gannyu Android 输入法安装脚本；将已构建的 APK 安装到连接设备并启用输入法。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
PKG="io.gannyu.input"
PRODUCT_VERSION="$(awk -F '"' '/^version[[:space:]]*=/ { print $2; exit }' "$REPO_ROOT/Cargo.toml")"
OUT_APK="$REPO_ROOT/build/android/GonnyuGeneralIME-${PRODUCT_VERSION}-android.apk"

find_bin() {
  local name="$1"
  shift
  if command -v "$name" >/dev/null 2>&1; then
    command -v "$name"
    return 0
  fi
  local candidate
  for candidate in "$@"; do
    if [ -n "$candidate" ] && [ -x "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

find_sdk_root() {
  local candidate
  for candidate in \
    "${ANDROID_SDK_ROOT:-}" \
    "${ANDROID_HOME:-}" \
    "${GANNYU_ANDROID_CACHE:-}" \
    "$REPO_ROOT/.android-sdk" \
    "$REPO_ROOT/../../_Cache" \
    "$HOME/Library/Android/sdk" \
    "$HOME/Android/Sdk"
  do
    if [ -n "$candidate" ] && [ -d "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

if [ ! -f "$OUT_APK" ]; then
  echo "未找到 APK: $OUT_APK" >&2
  echo "请先运行 build.sh。" >&2
  exit 1
fi

SDK_ROOT="$(find_sdk_root)" || SDK_ROOT=""
ADB_BIN="$(find_bin adb "${SDK_ROOT:+$SDK_ROOT/platform-tools/adb}")" || {
  echo "缺 adb；请安装 Android platform-tools。" >&2
  exit 1
}

DEVICE_COUNT="$("$ADB_BIN" devices | awk 'NR>1 && $2=="device" {count++} END {print count+0}')"
if [ "$DEVICE_COUNT" -eq 0 ]; then
  echo "未检测到 Android 设备，请通过 USB 连接设备后重试。" >&2
  exit 1
fi

echo "[1/2] 安装 APK"
"$ADB_BIN" install -r "$OUT_APK"

echo "[2/2] 启用输入法"
"$ADB_BIN" shell settings put secure show_ime_with_hard_keyboard 1
CURRENT_ENABLED="$("$ADB_BIN" shell settings get secure enabled_input_methods 2>/dev/null || echo '')"
if [ -n "$CURRENT_ENABLED" ] && ! echo "$CURRENT_ENABLED" | grep -q "$PKG"; then
  "$ADB_BIN" shell "settings put secure enabled_input_methods '${CURRENT_ENABLED}:${PKG}/.GannyuInputMethodService'"
fi
"$ADB_BIN" shell ime set "$PKG/.GannyuInputMethodService" 2>/dev/null || \
  "$ADB_BIN" shell settings put secure default_input_method "$PKG/.GannyuInputMethodService"
"$ADB_BIN" shell am start -n "$PKG/.SetupActivity" >/dev/null 2>&1 || true

echo ""
echo "✅ 安装完成！"
echo "📱 打开任意输入框，输入法应已切换到「赣语输入法」。"
