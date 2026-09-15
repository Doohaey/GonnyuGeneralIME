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

env GANNYU_IOS_SDK=iphoneos GANNYU_IOS_ARCH=arm64 bash "$script_dir/build_ios_engine.sh"
for architecture in arm64 x86_64; do
  env GANNYU_IOS_SDK=iphonesimulator GANNYU_IOS_ARCH="$architecture" \
    bash "$script_dir/build_ios_engine.sh"
done

combine_slice() {
  local sdk_name="$1"
  local architecture="$2"
  local output="$3"
  local root="$repo_root/build/rime-mobile/ios/$sdk_name-$architecture"
  local libraries=(
    "$root/adapter/libgannyu_rime_engine.a"
    "$root/prefix/lib/librime.a"
    "$root/prefix/lib/libleveldb.a"
    "$root/prefix/lib/libmarisa.a"
    "$root/prefix/lib/libopencc.a"
    "$root/prefix/lib/libyaml-cpp.a"
    "$root/prefix/lib/libglog.a"
    "$root/prefix/lib/libboost_regex.a"
  )
  for library in "${libraries[@]}"; do
    [[ -f "$library" ]] || { echo "missing Apple Rime library: $library" >&2; exit 2; }
  done
  mkdir -p "$(dirname "$output")"
  /usr/bin/libtool -static -o "$output" "${libraries[@]}"
  /usr/bin/ranlib "$output"
}

mkdir -p "$output_root"
device_library="$output_root/iphoneos-arm64/libgannyu_input_ffi.a"
simulator_arm64_library="$output_root/iphonesimulator-arm64/libgannyu_input_ffi.a"
simulator_x86_64_library="$output_root/iphonesimulator-x86_64/libgannyu_input_ffi.a"
simulator_library="$output_root/iphonesimulator-universal/libgannyu_input_ffi.a"
combine_slice iphoneos arm64 "$device_library"
combine_slice iphonesimulator arm64 "$simulator_arm64_library"
combine_slice iphonesimulator x86_64 "$simulator_x86_64_library"
mkdir -p "$(dirname "$simulator_library")"
lipo -create "$simulator_arm64_library" "$simulator_x86_64_library" -output "$simulator_library"

rm -rf "$headers_root" "$framework"
mkdir -p "$headers_root"
cp "$repo_root/crates/ffi/include/gannyu_input.h" "$headers_root/"
cp "$repo_root/platforms/ios/Sources/CGannyuInput/module.modulemap" "$headers_root/"

xcodebuild -create-xcframework \
  -library "$device_library" -headers "$headers_root" \
  -library "$simulator_library" -headers "$headers_root" \
  -output "$framework"

echo "built Rime XCFramework: $framework"
