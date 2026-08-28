#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
REPO_ROOT="$( cd "$SCRIPT_DIR/../../.." && pwd )"

if [ -d "$SCRIPT_DIR/payload" ]; then
  PAYLOAD_DIR="$SCRIPT_DIR/payload"
else
  PAYLOAD_DIR="${GANNYU_PACKAGE_ROOT:-$REPO_ROOT/build/linux-ibus}"
fi

if [ ! -f "$PAYLOAD_DIR/ibus-engine-gannyu" ]; then
  echo "未找到构建制品 $PAYLOAD_DIR/ibus-engine-gannyu" >&2
  exit 1
fi

install_file() {
  local mode="$1" source="$2" destination="$3"
  if [ -n "${DESTDIR:-}" ] || [ "${EUID:-$(id -u)}" -eq 0 ]; then
    install -Dm"$mode" "$source" "${DESTDIR:-}$destination"
  else
    sudo install -Dm"$mode" "$source" "$destination"
  fi
}

echo "[1/2] 安装 ibus engine 到系统"
install_file 755 "$PAYLOAD_DIR/ibus-engine-gannyu" /usr/libexec/ibus-engine-gannyu
install_file 644 "$PAYLOAD_DIR/gannyu.xml" /usr/share/ibus/component/gannyu.xml

echo "[2/2] 安装完成"
echo "重启 ibus: ibus-daemon -drx 或 ibus restart"
echo "ibus-setup → 输入法 → 添加 → 选「Gannyu Gan」"
