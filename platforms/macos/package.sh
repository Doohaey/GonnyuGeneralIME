#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
bundle="$repo_root/build/macos/GonnyuInputMethod.app"
output_dir="${GANNYU_MACOS_PACKAGE_OUTPUT:-$repo_root/build/macos}"
package_id="org.doohaey.inputmethod.gonnyu.pkg"
app_identity="${GANNYU_MACOS_APP_SIGN_IDENTITY:?set GANNYU_MACOS_APP_SIGN_IDENTITY to a Developer ID Application identity}"
installer_identity="${GANNYU_MACOS_INSTALLER_SIGN_IDENTITY:?set GANNYU_MACOS_INSTALLER_SIGN_IDENTITY to a Developer ID Installer identity}"
notary_profile="${GANNYU_MACOS_NOTARY_PROFILE:?set GANNYU_MACOS_NOTARY_PROFILE to a notarytool keychain profile}"
stage_dir="$(mktemp -d)"
trap 'rm -rf "$stage_dir"' EXIT

[[ "$app_identity" == "Developer ID Application:"* ]] || { echo "GANNYU_MACOS_APP_SIGN_IDENTITY must be a Developer ID Application identity" >&2; exit 1; }
[[ "$installer_identity" == "Developer ID Installer:"* ]] || { echo "GANNYU_MACOS_INSTALLER_SIGN_IDENTITY must be a Developer ID Installer identity" >&2; exit 1; }
GANNYU_MACOS_SIGN_IDENTITY="$app_identity" bash "$script_dir/build.sh"
version="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$bundle/Contents/Info.plist")"
mkdir -p "$stage_dir/root/Library/Input Methods" "$output_dir"
ditto "$bundle" "$stage_dir/root/Library/Input Methods/GonnyuInputMethod.app"
find "$stage_dir/root" -name '._*' -type f -delete
pkgbuild \
  --root "$stage_dir/root" \
  --identifier "$package_id" \
  --version "$version" \
  --install-location / \
  --sign "$installer_identity" \
  "$output_dir/GonnyuInputMethod.pkg"
xcrun notarytool submit "$output_dir/GonnyuInputMethod.pkg" --keychain-profile "$notary_profile" --wait
xcrun stapler staple "$output_dir/GonnyuInputMethod.pkg"
spctl --assess --type install --verbose=4 "$output_dir/GonnyuInputMethod.pkg"

echo "packaged $output_dir/GonnyuInputMethod.pkg"
