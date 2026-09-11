#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
output_root="${GANNYU_IOS_BUILD_ROOT:-$repo_root/build/ios}"
signing_config="$script_dir/Config/Signing.xcconfig"

[[ "$(uname -s)" == "Darwin" ]] || { echo "this builder must run on macOS" >&2; exit 1; }
[[ -f "$signing_config" ]] || { echo "copy Config/Signing.xcconfig.example to Config/Signing.xcconfig and set your Apple team and App Group" >&2; exit 1; }
grep -q '^DEVELOPMENT_TEAM = [^Y]' "$signing_config" || { echo "Signing.xcconfig must set DEVELOPMENT_TEAM" >&2; exit 1; }
grep -q '^GANNYU_APP_GROUP = group\.' "$signing_config" || { echo "Signing.xcconfig must set a valid GANNYU_APP_GROUP" >&2; exit 1; }
grep -q '^GANNYU_APP_BUNDLE_IDENTIFIER = ' "$signing_config" || { echo "Signing.xcconfig must set GANNYU_APP_BUNDLE_IDENTIFIER" >&2; exit 1; }
grep -q '^GANNYU_KEYBOARD_BUNDLE_IDENTIFIER = ' "$signing_config" || { echo "Signing.xcconfig must set GANNYU_KEYBOARD_BUNDLE_IDENTIFIER" >&2; exit 1; }
command -v cargo >/dev/null || { echo "cargo not found" >&2; exit 1; }
command -v xcodebuild >/dev/null || { echo "Xcode not found" >&2; exit 1; }

if [[ -z "${GANNYU_RESOURCE_KEY:-}" && -r "${GANNYU_RESOURCE_KEY_FILE:-$HOME/.config/gonnyu/resource-key}" ]]; then
  IFS= read -r GANNYU_RESOURCE_KEY < "${GANNYU_RESOURCE_KEY_FILE:-$HOME/.config/gonnyu/resource-key}"
  export GANNYU_RESOURCE_KEY
fi

for target in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
  rustup target list --installed | grep -qx "$target" || {
    echo "missing Rust target: $target" >&2
    exit 1
  }
  cargo build -p gannyu-input-ffi --release --target "$target"
done

ffi_root="$output_root/GannyuInputFFI.xcframework"
headers_root="$output_root/ffi-headers"
simulator_library="$output_root/libgannyu_input_ffi-simulator.a"
rm -rf "$ffi_root"
rm -rf "$headers_root"
mkdir -p "$headers_root"
cp "$repo_root/crates/ffi/include/gannyu_input.h" "$headers_root/"
cp "$script_dir/Sources/CGannyuInput/module.modulemap" "$headers_root/"
lipo -create \
  "$repo_root/target/aarch64-apple-ios-sim/release/libgannyu_input_ffi.a" \
  "$repo_root/target/x86_64-apple-ios/release/libgannyu_input_ffi.a" \
  -output "$simulator_library"
xcodebuild -create-xcframework \
  -library "$repo_root/target/aarch64-apple-ios/release/libgannyu_input_ffi.a" -headers "$headers_root" \
  -library "$simulator_library" -headers "$headers_root" \
  -output "$ffi_root"

xcodebuild \
  -project "$script_dir/GannyuInput.xcodeproj" \
  -scheme GannyuInput \
  -configuration Release \
  -sdk iphoneos \
  -destination 'generic/platform=iOS' \
  -xcconfig "$signing_config" \
  -archivePath "$output_root/GannyuInput.xcarchive" \
  archive
