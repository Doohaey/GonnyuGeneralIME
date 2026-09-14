#!/usr/bin/env bash
# Build device and simulator Rime slices and package the stable mobile C ABI.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
output_root="${GANNYU_IOS_BUILD_ROOT:-$repo_root/build/ios}"
headers_root="$output_root/ffi-headers"
framework="$output_root/GannyuInputFFI.xcframework"

command -v xcodebuild >/dev/null || { echo "Xcode is required" >&2; exit 2; }
[[ -x /usr/bin/libtool ]] || { echo "Apple libtool is required" >&2; exit 2; }

for sdk_name in iphoneos iphonesimulator; do
  env GANNYU_IOS_SDK="$sdk_name" GANNYU_IOS_ARCH=arm64 \
    bash "$script_dir/build_ios_engine.sh"
done

combine_slice() {
  local sdk_name="$1"
  local root="$repo_root/build/rime-mobile/ios/$sdk_name-arm64"
  local output="$output_root/$sdk_name-arm64/libgannyu_input_ffi.a"
  local libraries=(
    "$root/adapter/libgannyu_rime_engine.a"
    "$root/prefix/lib/librime.a"
    "$root/prefix/lib/libleveldb.a"
    "$root/prefix/lib/libmarisa.a"
    "$root/prefix/lib/libopencc.a"
    "$root/prefix/lib/libyaml-cpp.a"
    "$root/prefix/lib/libglog.a"
  )
  for library in "${libraries[@]}"; do
    [[ -f "$library" ]] || { echo "missing Apple Rime library: $library" >&2; exit 2; }
  done
  mkdir -p "$(dirname "$output")"
  /usr/bin/libtool -static -o "$output" "${libraries[@]}"
  /usr/bin/ranlib "$output"
}

mkdir -p "$output_root"
combine_slice iphoneos
combine_slice iphonesimulator

rm -rf "$headers_root" "$framework"
mkdir -p "$headers_root"
cp "$repo_root/crates/ffi/include/gannyu_input.h" "$headers_root/"
cp "$repo_root/platforms/ios/Sources/CGannyuInput/module.modulemap" "$headers_root/"

xcodebuild -create-xcframework \
  -library "$output_root/iphoneos-arm64/libgannyu_input_ffi.a" -headers "$headers_root" \
  -library "$output_root/iphonesimulator-arm64/libgannyu_input_ffi.a" -headers "$headers_root" \
  -output "$framework"

echo "built Rime XCFramework: $framework"
