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
bundle_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$target_bundle/Contents/Info.plist")"
if ! defaults export com.apple.HIToolbox - \
  | plutil -extract AppleEnabledInputSources xml1 -o - - \
  | grep -Fq "<string>$bundle_id</string>"; then
  defaults write com.apple.HIToolbox AppleEnabledInputSources -array-add \
    "<dict><key>Bundle ID</key><string>$bundle_id</string><key>InputSourceKind</key><string>Keyboard Input Method</string></dict>"
  defaults write com.apple.HIToolbox AppleEnabledInputSources -array-add \
    "<dict><key>Bundle ID</key><string>$bundle_id</string><key>Input Mode</key><string>$bundle_id.Gan</string><key>InputSourceKind</key><string>Input Mode</string></dict>"
fi
echo "installed $target_bundle"
echo "registered and enabled; reopen System Settings to use Gonnyu"
