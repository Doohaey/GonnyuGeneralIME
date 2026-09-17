#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
output_root="${GANNYU_IOS_BUILD_ROOT:-$repo_root/build/ios}"
signing_config="$script_dir/Config/Signing.xcconfig"
product_version="$(awk -F '"' '/^version[[:space:]]*=/ { print $2; exit }' "$repo_root/Cargo.toml")"
build_number="${GANNYU_IOS_BUILD_NUMBER:-1}"

[[ "$(uname -s)" == "Darwin" ]] || { echo "this builder must run on macOS" >&2; exit 1; }
[[ -f "$signing_config" ]] || { echo "copy Config/Signing.xcconfig.example to Config/Signing.xcconfig and set your Apple team and App Group" >&2; exit 1; }
[[ "$product_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "workspace version must be a stable semver: $product_version" >&2; exit 1; }
[[ "$build_number" =~ ^[1-9][0-9]*$ ]] || { echo "GANNYU_IOS_BUILD_NUMBER must be a positive integer" >&2; exit 1; }
grep -q '^DEVELOPMENT_TEAM = [^Y]' "$signing_config" || { echo "Signing.xcconfig must set DEVELOPMENT_TEAM" >&2; exit 1; }
grep -q '^GANNYU_APP_GROUP = group\.' "$signing_config" || { echo "Signing.xcconfig must set a valid GANNYU_APP_GROUP" >&2; exit 1; }
grep -q '^GANNYU_APP_BUNDLE_IDENTIFIER = ' "$signing_config" || { echo "Signing.xcconfig must set GANNYU_APP_BUNDLE_IDENTIFIER" >&2; exit 1; }
grep -q '^GANNYU_KEYBOARD_BUNDLE_IDENTIFIER = ' "$signing_config" || { echo "Signing.xcconfig must set GANNYU_KEYBOARD_BUNDLE_IDENTIFIER" >&2; exit 1; }
command -v xcodebuild >/dev/null || { echo "Xcode not found" >&2; exit 1; }

cd "$repo_root"

python_bin="${PYTHON_BIN:-python3}"
if ! "$python_bin" -c 'import tomllib' >/dev/null 2>&1; then
  for candidate in /opt/homebrew/bin/python3 python3.14 python3.13 python3.12 python3.11; do
    if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import tomllib' >/dev/null 2>&1; then
      python_bin="$candidate"
      break
    fi
  done
fi
"$python_bin" -c 'import tomllib' >/dev/null 2>&1 || {
  echo "Python 3.11+ with tomllib is required; set PYTHON_BIN" >&2
  exit 1
}

PYTHON_BIN="$python_bin" bash "$repo_root/platforms/rime/mobile/prepare_resources.sh"
if [[ -e "$output_root/rime" || -L "$output_root/rime" ]]; then
  rm -rf -- "$output_root/rime"
fi
ln -sfn ../rime-mobile/mobile-resources "$output_root/rime"
GANNYU_IOS_BUILD_ROOT="$output_root" PYTHON_BIN="$python_bin" \
  bash "$repo_root/platforms/rime/mobile/build_ios_xcframework.sh"

xcodebuild \
  -project "$script_dir/GonnyuInput.xcodeproj" \
  -scheme GonnyuInput \
  -configuration Release \
  -sdk iphoneos \
  -destination 'generic/platform=iOS' \
  -derivedDataPath "$output_root/DerivedData" \
  -xcconfig "$signing_config" \
  MARKETING_VERSION="$product_version" \
  CURRENT_PROJECT_VERSION="$build_number" \
  -archivePath "$output_root/GonnyuInput.xcarchive" \
  archive

archive_app="$output_root/GonnyuInput.xcarchive/Products/Applications/GonnyuInputMethod.app"
archive_keyboard="$archive_app/PlugIns/GonnyuKeyboard.appex"
[[ -d "$archive_app" ]] || { echo "archive is missing the host app: $archive_app" >&2; exit 1; }
[[ -d "$archive_keyboard" ]] || { echo "archive is missing the keyboard extension: $archive_keyboard" >&2; exit 1; }
[[ -f "$archive_app/PrivacyInfo.xcprivacy" ]] || { echo "archive is missing the app privacy manifest" >&2; exit 1; }
[[ -f "$archive_keyboard/PrivacyInfo.xcprivacy" ]] || { echo "archive is missing the keyboard privacy manifest" >&2; exit 1; }
echo "created App Store archive: $output_root/GonnyuInput.xcarchive"
echo "version: $product_version ($build_number)"
