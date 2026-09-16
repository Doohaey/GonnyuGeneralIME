#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
bundle="$repo_root/build/macos/GonnyuInputMethod.app"
output_dir="${GANNYU_MACOS_PACKAGE_OUTPUT:-$repo_root/build/macos}"
package_id="org.doohaey.inputmethod.gonnyu.pkg"
unsigned_test="${GANNYU_MACOS_UNSIGNED_TEST:-0}"
skip_notarization="${GANNYU_MACOS_PACKAGE_SKIP_NOTARIZATION:-0}"
if [[ "$unsigned_test" != "1" ]]; then
  app_identity="${GANNYU_MACOS_APP_SIGN_IDENTITY:?set GANNYU_MACOS_APP_SIGN_IDENTITY to a Developer ID Application identity}"
  installer_identity="${GANNYU_MACOS_INSTALLER_SIGN_IDENTITY:?set GANNYU_MACOS_INSTALLER_SIGN_IDENTITY to a Developer ID Installer identity}"
  if [[ "$skip_notarization" != "1" ]]; then
    notary_profile="${GANNYU_MACOS_NOTARY_PROFILE:?set GANNYU_MACOS_NOTARY_PROFILE to a notarytool keychain profile}"
  fi
  [[ "$app_identity" == "Developer ID Application:"* ]] || { echo "GANNYU_MACOS_APP_SIGN_IDENTITY must be a Developer ID Application identity" >&2; exit 1; }
  [[ "$installer_identity" == "Developer ID Installer:"* ]] || { echo "GANNYU_MACOS_INSTALLER_SIGN_IDENTITY must be a Developer ID Installer identity" >&2; exit 1; }
fi
stage_dir="$(mktemp -d)"
trap 'rm -rf "$stage_dir"' EXIT

if [[ "${GANNYU_MACOS_PACKAGE_SKIP_BUILD:-0}" != "1" ]]; then
  if [[ "$unsigned_test" == "1" ]]; then
    bash "$script_dir/build.sh"
  else
    GANNYU_MACOS_SIGN_IDENTITY="$app_identity" bash "$script_dir/build.sh"
  fi
fi
codesign --verify --deep --strict "$bundle"
version="$(/usr/libexec/PlistBuddy -c 'Print :GannyuVersion' "$bundle/Contents/Info.plist")"
package_version="$(python3 "$script_dir/installer_version.py" "$version")"
actual_build="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$bundle/Contents/Info.plist")"
[[ "$actual_build" == "$package_version" ]] || { echo "bundle and installer build versions disagree" >&2; exit 1; }
mkdir -p "$stage_dir/root/Library/Input Methods" "$stage_dir/scripts" "$output_dir"
ditto "$bundle" "$stage_dir/root/Library/Input Methods/GonnyuInputMethod.app"
find "$stage_dir/root" -name '._*' -type f -delete
cp "$script_dir/check_installer_version.sh" "$stage_dir/scripts/check_installer_version.sh"
cp "$script_dir/Scripts/postinstall" "$stage_dir/scripts/postinstall"
sed "s/@VERSION@/$version/g" "$script_dir/Scripts/preinstall.template" > "$stage_dir/scripts/preinstall"
chmod 0755 "$stage_dir/scripts/"*
pkgbuild_args=(
  --root "$stage_dir/root" --scripts "$stage_dir/scripts"
  --identifier "$package_id" --version "$package_version" --install-location /
)
if [[ "$unsigned_test" != "1" ]]; then pkgbuild_args+=(--sign "$installer_identity"); fi
pkgbuild "${pkgbuild_args[@]}" "$output_dir/GonnyuInputMethod.pkg"
if [[ "$unsigned_test" != "1" && "$skip_notarization" != "1" ]]; then
  notary_args=(--keychain-profile "$notary_profile" --wait)
  if [[ -n "${GANNYU_MACOS_NOTARY_KEYCHAIN:-}" ]]; then
    notary_args+=(--keychain "$GANNYU_MACOS_NOTARY_KEYCHAIN")
  fi
  xcrun notarytool submit "$output_dir/GonnyuInputMethod.pkg" "${notary_args[@]}"
  xcrun stapler staple "$output_dir/GonnyuInputMethod.pkg"
  spctl --assess --type install --verbose=4 "$output_dir/GonnyuInputMethod.pkg"
fi

echo "packaged $output_dir/GonnyuInputMethod.pkg (workspace $version, installer $package_version)"
