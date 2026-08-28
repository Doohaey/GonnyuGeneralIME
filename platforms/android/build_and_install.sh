#!/usr/bin/env bash
# Gannyu Android 输入法编译+安装脚本；依赖 Android SDK/NDK、Rust toolchain 与 adb。
# 使用 build.sh 和 install.sh 分别执行编译和安装步骤。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bash "$SCRIPT_DIR/build.sh"
bash "$SCRIPT_DIR/install.sh"
exit 0
