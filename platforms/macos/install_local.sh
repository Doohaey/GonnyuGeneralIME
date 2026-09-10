#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
target_dir="${HOME}/Library/Input Methods"
bundle="${GANNYU_MACOS_APP_BUNDLE:-$repo_root/build/macos/GonnyuInputMethod.app}"
target_bundle="$target_dir/GonnyuInputMethod.app"

bash "$script_dir/build.sh"
mkdir -p "$target_dir"
rm -rf "$target_bundle"
cp -R "$bundle" "$target_bundle"
codesign --verify --deep --strict "$target_bundle"
GANNYU_REGISTER_INPUT_SOURCE=1 GANNYU_IMK_SELFTEST=1 \
  "$target_bundle/Contents/MacOS/GannyuInputMethodHost"
echo "installed $target_bundle"
echo "registered with Text Input Source Services; reopen System Settings to add the input method"
