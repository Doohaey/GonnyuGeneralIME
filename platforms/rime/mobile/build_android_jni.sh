#!/usr/bin/env bash
# Link the already cross-compiled, pinned Rime engine into the Android JNI APK input.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
android_root="$repo_root/platforms/android"
app_root="$android_root/app"
abi="${GANNYU_ANDROID_ABI:-arm64-v8a}"
sdk_root="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Library/Android/sdk}}"

if [[ "$abi" != "arm64-v8a" ]]; then
  echo "Rime mobile JNI currently supports arm64-v8a only: $abi" >&2
  exit 2
fi
if [[ ! -d "$sdk_root" ]]; then
  echo "Android SDK not found; set ANDROID_SDK_ROOT or ANDROID_HOME." >&2
  exit 2
fi

ndk_root="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
if [[ -z "$ndk_root" && -d "$sdk_root/ndk" ]]; then
  ndk_root="$(find "$sdk_root/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)"
fi
if [[ -z "$ndk_root" || ! -f "$ndk_root/build/cmake/android.toolchain.cmake" ]]; then
  echo "Android NDK not found; set ANDROID_NDK_HOME or ANDROID_NDK_ROOT." >&2
  exit 2
fi

rime_root="$repo_root/build/rime-mobile/android/$abi"
for required in \
  "$rime_root/adapter/libgannyu_rime_engine.a" \
  "$rime_root/librime/lib/librime.a" \
  "$rime_root/prefix/lib/libleveldb.a" \
  "$rime_root/prefix/lib/libmarisa.a" \
  "$rime_root/prefix/lib/libopencc.a" \
  "$rime_root/prefix/lib/libyaml-cpp.a" \
  "$rime_root/prefix/lib/libglog.a"; do
  if [[ ! -f "$required" ]]; then
    echo "Missing pinned Android Rime library: $required" >&2
    echo "Build the Rime Android dependency tree before packaging the app." >&2
    exit 2
  fi
done

cmake_bin="$(command -v cmake || true)"
if [[ -z "$cmake_bin" ]]; then
  echo "cmake is required to link Android Rime JNI." >&2
  exit 2
fi

native_build="$rime_root/jni"
"$cmake_bin" -S "$app_root/src/main/cpp" -B "$native_build" \
  -DCMAKE_TOOLCHAIN_FILE="$ndk_root/build/cmake/android.toolchain.cmake" \
  -DANDROID_ABI="$abi" \
  -DANDROID_PLATFORM=android-24 \
  -DCMAKE_BUILD_TYPE=Release
"$cmake_bin" --build "$native_build"

output="$native_build/libgannyu_input_jni.so"
if [[ ! -f "$output" ]]; then
  echo "Rime JNI link completed without producing $output" >&2
  exit 1
fi

mkdir -p "$app_root/src/main/jniLibs/$abi"
cp "$output" "$app_root/src/main/jniLibs/$abi/libgannyu_input_jni.so"
echo "linked Rime JNI: $app_root/src/main/jniLibs/$abi/libgannyu_input_jni.so"
