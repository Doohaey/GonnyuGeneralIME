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
  # A build artifact with the production bundle ID must not remain discoverable
  # beside the installed copy.  Build into private staging and remove it once
  # the signed app has been copied to its final Input Methods location.
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
# Do not leave the build/installer copy registered with the same bundle ID as
# the installed input method.  Text Services must resolve this identifier to
# exactly the copy in ~/Library/Input Methods when it launches the IMK server.
if [[ "$bundle" != "$target_bundle" && -x "$lsregister" ]]; then
  "$lsregister" -u "$bundle" >/dev/null 2>&1 || true
fi
if [[ -x "$lsregister" ]]; then
  "$lsregister" -f "$target_bundle" >/dev/null
fi
GANNYU_REGISTER_INPUT_SOURCE=1 GANNYU_IMK_SELFTEST=1 \
  "$target_bundle/Contents/MacOS/GannyuInputMethodHost"
bundle_id="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$target_bundle/Contents/Info.plist")"
mode_id="$bundle_id.Gan"
if ! defaults export com.apple.HIToolbox - \
  | plutil -extract AppleEnabledInputSources xml1 -o - - \
  | grep -Fq "<string>$mode_id</string>"; then
  defaults write com.apple.HIToolbox AppleEnabledInputSources -array-add \
    "<dict><key>Bundle ID</key><string>$bundle_id</string><key>Input Mode</key><string>$mode_id</string><key>InputSourceKind</key><string>Input Mode</string></dict>"
fi
trap - ERR
if [[ -d "$backup_bundle" ]]; then rm -rf "$backup_bundle"; fi
echo "installed $target_bundle"
echo "registered and enabled; reopen System Settings to use Gonnyu"
