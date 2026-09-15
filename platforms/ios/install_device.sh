#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
app="${GANNYU_IOS_APP_BUNDLE:-$repo_root/build/ios/DerivedData/Build/Products/Release-iphoneos/GonnyuInputMethod.app}"
device="${GANNYU_IOS_DEVICE_ID:?set GANNYU_IOS_DEVICE_ID to the paired device identifier}"
profiles_dir="${GANNYU_IOS_PROFILES_DIR:-$HOME/Library/Developer/Xcode/UserData/Provisioning Profiles}"
identity="${GANNYU_IOS_SIGN_IDENTITY:-}"

[[ -d "$app" ]] || { echo "iOS app bundle not found: $app" >&2; exit 2; }
[[ -d "$profiles_dir" ]] || { echo "Xcode provisioning profile directory not found: $profiles_dir" >&2; exit 2; }
command -v security >/dev/null || { echo "security is required" >&2; exit 2; }
command -v codesign >/dev/null || { echo "codesign is required" >&2; exit 2; }
command -v xcrun >/dev/null || { echo "xcrun is required" >&2; exit 2; }

bundle_id() { /usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$1/Info.plist"; }

profile_for_bundle() {
  local wanted="$1" profile app_id
  for profile in "$profiles_dir"/*.mobileprovision; do
    [[ -f "$profile" ]] || continue
    app_id="$(security cms -D -i "$profile" 2>/dev/null | plutil -extract Entitlements.application-identifier raw -o - - 2>/dev/null || true)"
    [[ "$app_id" == *".$wanted" ]] && { printf '%s\n' "$profile"; return 0; }
  done
  echo "no provisioning profile found for $wanted" >&2
  return 1
}

if [[ -z "$identity" ]]; then
  # Xcode may leave several development teams in the login keychain. The
  # newest matching development identity is listed last by security.
  identity="$(security find-identity -v -p codesigning 2>/dev/null | sed -n '/"Apple Development:/s/.* \([0-9A-F]\{40\}\) ".*/\1/p' | tail -n 1)"
fi
[[ -n "$identity" ]] || { echo "no Apple Development signing identity found" >&2; exit 2; }

extension="$app/PlugIns/GonnyuKeyboard.appex"
[[ -d "$extension" ]] || { echo "keyboard extension not found: $extension" >&2; exit 2; }
app_profile="$(profile_for_bundle "$(bundle_id "$app")")"
extension_profile="$(profile_for_bundle "$(bundle_id "$extension")")"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/gonnyu-ios-sign.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

embed_and_extract() {
  local profile="$1"
  local target="$2"
  local entitlements="$3"
  local decoded="$tmp_dir/$(basename "$target").plist"
  cp "$profile" "$target/embedded.mobileprovision"
  security cms -D -i "$profile" > "$decoded"
  plutil -extract Entitlements xml1 -o "$entitlements" "$decoded"
}

embed_and_extract "$extension_profile" "$extension" "$tmp_dir/keyboard-entitlements.plist"
embed_and_extract "$app_profile" "$app" "$tmp_dir/app-entitlements.plist"
codesign --force --sign "$identity" --entitlements "$tmp_dir/keyboard-entitlements.plist" --timestamp=none "$extension"
codesign --force --sign "$identity" --entitlements "$tmp_dir/app-entitlements.plist" --timestamp=none "$app"
codesign --verify --deep --strict "$app"
xcrun devicectl device install app --device "$device" "$app"
