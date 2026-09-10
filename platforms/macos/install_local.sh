#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
target_dir="${HOME}/Library/Input Methods"
bundle="${GANNYU_MACOS_APP_BUNDLE:-$repo_root/build/macos/GannyuInputMethod.app}"
target_bundle="$target_dir/GannyuInputMethod.app"

bash "$script_dir/build.sh"
mkdir -p "$target_dir"
rm -rf "$target_bundle"
cp -R "$bundle" "$target_bundle"
echo "installed $target_bundle"
echo "log out or restart the Text Input system before adding the input method in System Settings"
