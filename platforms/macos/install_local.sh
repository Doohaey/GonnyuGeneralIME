#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
target_dir="${HOME}/Library/Input Methods"
target_bundle="$target_dir/GonnyuInputMethod.app"
backup_bundle="$target_dir/.GonnyuInputMethod.previous.app"
lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
staging_dir=""

if [[ -n "${GANNYU_MACOS_APP_BUNDLE:-}" ]]; then
  bundle="$GANNYU_MACOS_APP_BUNDLE"
else
  # Build in staging before copying the signed app to Input Methods.
  staging_dir="$(mktemp -d "${TMPDIR:-/private/tmp}/gonnyu-imk-install.XXXXXX")"
  bundle="$staging_dir/GonnyuInputMethod.app"
  export GANNYU_MACOS_APP_BUNDLE="$bundle"
  trap 'rm -rf "$staging_dir"' EXIT
fi

if [[ "${GANNYU_MACOS_INSTALL_SKIP_BUILD:-0}" != "1" ]]; then
  bash "$script_dir/build.sh"
fi
[[ -d "$bundle" ]] || { echo "missing built app bundle: $bundle" >&2; exit 1; }
codesign --verify --deep --strict "$bundle"
incoming_version="$(/usr/libexec/PlistBuddy -c 'Print :GannyuVersion' "$bundle/Contents/Info.plist")"
bash "$script_dir/check_installer_version.sh" "$incoming_version" "$target_bundle/Contents/Info.plist"
mkdir -p "$target_dir"
[[ ! -e "$backup_bundle" ]] || { echo "recover previous installation before retrying: $backup_bundle" >&2; exit 1; }
rollback_install() {
  if [[ -d "$backup_bundle" ]]; then
    if [[ -d "$target_bundle" ]]; then rm -rf "$target_bundle"; fi
    mv "$backup_bundle" "$target_bundle"
    if [[ -x "$lsregister" ]]; then "$lsregister" -f "$target_bundle" >/dev/null 2>&1 || true; fi
  fi
}
trap rollback_install ERR
if [[ -d "$target_bundle" ]]; then mv "$target_bundle" "$backup_bundle"; fi
ditto "$bundle" "$target_bundle"
codesign --verify --deep --strict "$target_bundle"
# Register the installed copy as the Input Methods implementation.
if [[ "$bundle" != "$target_bundle" && -x "$lsregister" ]]; then
  "$lsregister" -u "$bundle" >/dev/null 2>&1 || true
fi
if [[ -x "$lsregister" ]]; then
  "$lsregister" -f "$target_bundle" >/dev/null
fi
GANNYU_REGISTER_INPUT_SOURCE=1 GANNYU_IMK_SELFTEST=1 \
  "$target_bundle/Contents/MacOS/GannyuInputMethodHost"
bundle_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$target_bundle/Contents/Info.plist")"
source_id="$bundle_id.Gan"
# Replace the legacy ".Gan" source entry with the current primary source.
preferences_dir="$(mktemp -d "${TMPDIR:-/private/tmp}/gonnyu-input-sources.XXXXXX")"
preferences_plist="$preferences_dir/HIToolbox.plist"
defaults export com.apple.HIToolbox "$preferences_plist" >/dev/null
for array_key in AppleEnabledInputSources AppleSelectedInputSources AppleInputSourceHistory; do
  for ((index = 99; index >= 0; index--)); do
    entry_bundle="$(/usr/libexec/PlistBuddy -c "Print :$array_key:$index:'Bundle ID'" "$preferences_plist" 2>/dev/null || true)"
    entry_mode="$(/usr/libexec/PlistBuddy -c "Print :$array_key:$index:'Input Mode'" "$preferences_plist" 2>/dev/null || true)"
    if [[ "$entry_bundle" == "$bundle_id" && ( "$entry_mode" == "$bundle_id.Gan" || -z "$entry_mode" ) ]]; then
      /usr/libexec/PlistBuddy -c "Delete :$array_key:$index" "$preferences_plist"
    fi
  done
done
defaults import com.apple.HIToolbox "$preferences_plist"
rm -rf "$preferences_dir"
if ! defaults export com.apple.HIToolbox - \
  | plutil -extract AppleEnabledInputSources xml1 -o - - \
  | grep -Fq "<string>$source_id</string>"; then
  defaults write com.apple.HIToolbox AppleEnabledInputSources -array-add \
    "<dict><key>Bundle ID</key><string>$bundle_id</string><key>Input Mode</key><string>$source_id</string><key>InputSourceKind</key><string>Input Mode</string></dict>"
fi
trap - ERR
if [[ -d "$backup_bundle" ]]; then rm -rf "$backup_bundle"; fi
echo "installed $target_bundle"
echo "registered and enabled; reopen System Settings to use Gonnyu"
