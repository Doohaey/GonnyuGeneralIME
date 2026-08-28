#!/usr/bin/env bash
# Gannyu Android 输入法编译脚本；依赖 Android SDK/NDK 与 Rust toolchain。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
APK="$SCRIPT_DIR/app/build/outputs/apk/release/app-release.apk"
OUT_DIR="$REPO_ROOT/build"
SHIM_DIR="/tmp/ndk-shim"

find_bin() {
  local name="$1"
  shift
  if command -v "$name" >/dev/null 2>&1; then
    command -v "$name"
    return 0
  fi
  local candidate
  for candidate in "$@"; do
    if [ -n "$candidate" ] && [ -x "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

find_sdk_root() {
  local candidate
  for candidate in \
    "${ANDROID_SDK_ROOT:-}" \
    "${ANDROID_HOME:-}" \
    "${GANNYU_ANDROID_CACHE:-}" \
    "$REPO_ROOT/.android-sdk" \
    "$REPO_ROOT/../../_Cache" \
    "$HOME/Library/Android/sdk" \
    "$HOME/Android/Sdk"
  do
    if [ -n "$candidate" ] && [ -d "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

find_ndk_root() {
  local sdk_root="$1"
  if [ -n "${ANDROID_NDK_HOME:-}" ] && [ -d "${ANDROID_NDK_HOME}" ]; then
    printf '%s\n' "${ANDROID_NDK_HOME}"
    return 0
  fi
  if [ -n "${ANDROID_NDK_ROOT:-}" ] && [ -d "${ANDROID_NDK_ROOT}" ]; then
    printf '%s\n' "${ANDROID_NDK_ROOT}"
    return 0
  fi
  if [ -d "$sdk_root/ndk" ]; then
    ls -1d "$sdk_root"/ndk/* 2>/dev/null | sort -V | tail -n 1
    return 0
  fi
  return 1
}

setup_ndk_shim() {
  # NDK 26 可能缺少 clang -> clang-17 / ld.lld -> lld 软链接。
  # 创建 shim 目录补齐这些工具。
  if [ -d "$SHIM_DIR" ] && [ -L "$SHIM_DIR/clang" ] && [ -L "$SHIM_DIR/ld.lld" ]; then
    return 0
  fi
  mkdir -p "$SHIM_DIR"
  local ndk_bin="$TOOLCHAIN_DIR/bin"
  for tool in "$ndk_bin"/*; do
    local name
    name="$(basename "$tool")"
    [ -e "$SHIM_DIR/$name" ] || ln -sf "$tool" "$SHIM_DIR/$name"
  done
  [ -e "$SHIM_DIR/clang" ] || ln -sf clang-17 "$SHIM_DIR/clang"
  [ -e "$SHIM_DIR/clang++" ] || ln -sf clang++-17 "$SHIM_DIR/clang++"
  [ -e "$SHIM_DIR/ld.lld" ] || ln -sf lld "$SHIM_DIR/ld.lld"
  echo "[shim] NDK 工具链修复完成: $SHIM_DIR"
}

build_rust_target() {
  local abi="$1"
  local rust_target
  local clang_bin
  case "$abi" in
    arm64-v8a)
      rust_target="aarch64-linux-android"
      clang_bin="aarch64-linux-android24-clang"
      ;;
    armeabi-v7a)
      rust_target="armv7-linux-androideabi"
      clang_bin="armv7a-linux-androideabi24-clang"
      ;;
    x86_64)
      rust_target="x86_64-linux-android"
      clang_bin="x86_64-linux-android24-clang"
      ;;
    *)
      echo "Unsupported ABI: $abi" >&2
      exit 1
      ;;
  esac

  local upper_target
  upper_target="$(printf '%s' "$rust_target" | tr '[:lower:]-' '[:upper:]_')"

  echo "[rust] $abi -> $rust_target"
  "$RUSTUP_BIN" target add "$rust_target" >/dev/null
  env \
    "CC_${rust_target//-/_}=$SHIM_DIR/$clang_bin" \
    "AR_${rust_target//-/_}=$TOOLCHAIN_DIR/bin/llvm-ar" \
    "CARGO_TARGET_${upper_target}_LINKER=$SHIM_DIR/$clang_bin" \
    "$CARGO_BIN" build -p gannyu-input-ffi --release --target "$rust_target"
}

build_jni_libs() {
  echo "[jni] 编译 JNI 共享库"
  local sysroot="$TOOLCHAIN_DIR/sysroot"

  for abi in arm64-v8a; do
    local target rust_target out_dir
    case "$abi" in
      arm64-v8a)
        target="aarch64-linux-android24"
        rust_target="aarch64-linux-android"
        ;;
      *)
        continue
        ;;
    esac
    out_dir="$SCRIPT_DIR/app/src/main/jniLibs/$abi"
    mkdir -p "$out_dir"

    # --exclude-libs,ALL hides the Rust static library's internal symbols from
    # the shared library's dynamic symbol table, so only the JNI entry points
    # (Java_io_gannyu_...) are exported. Without this, the full Rust module and
    # function names (e.g. gannyu_input_ffi::whitebox::derive_master_key) leak
    # into .dynsym and are trivially readable by an attacker.
    "$SHIM_DIR/clang-17" \
      --target="$target" \
      --sysroot="$sysroot" \
      -I"$REPO_ROOT/crates/ffi/include" \
      -shared -o "$out_dir/libgannyu_input_jni.so" \
      "$SCRIPT_DIR/app/src/main/cpp/jni_bridge.c" \
      "$REPO_ROOT/target/$rust_target/release/libgannyu_input_ffi.a" \
      -llog -fPIC -O2 -Wall -Wl,--exclude-libs,ALL 2>&1 | sed 's/^/  /'
    echo "  ✅ $abi"
  done
}

SDK_ROOT="$(find_sdk_root)" || {
  echo "缺 Android SDK；请设置 ANDROID_SDK_ROOT / ANDROID_HOME 或安装到默认目录。" >&2
  exit 1
}
NDK_ROOT="$(find_ndk_root "$SDK_ROOT")" || {
  echo "缺 Android NDK；请设置 ANDROID_NDK_HOME / ANDROID_NDK_ROOT，或安装到 $SDK_ROOT/ndk。" >&2
  exit 1
}

TOOLCHAIN_DIR="$NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64"
if [ ! -d "$TOOLCHAIN_DIR" ]; then
  TOOLCHAIN_DIR="$NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64"
fi
if [ ! -d "$TOOLCHAIN_DIR" ]; then
  TOOLCHAIN_DIR="$NDK_ROOT/toolchains/llvm/prebuilt/darwin-arm64"
fi
if [ ! -d "$TOOLCHAIN_DIR" ]; then
  echo "未找到 NDK LLVM toolchain 目录。" >&2
  exit 1
fi

setup_ndk_shim

CARGO_BIN="$(find_bin cargo "$HOME/.cargo/bin/cargo")" || {
  echo "缺 cargo；请先安装 Rust 工具链。" >&2
  exit 1
}
RUSTUP_BIN="$(find_bin rustup "$HOME/.cargo/bin/rustup")" || {
  echo "缺 rustup；请先安装 Rust 工具链。" >&2
  exit 1
}

GRADLE_BIN="$SCRIPT_DIR/gradlew"
if [ ! -x "$GRADLE_BIN" ]; then
  echo "缺 Gradle wrapper: $GRADLE_BIN" >&2
  exit 1
fi

if [ -z "${GANNYU_RESOURCE_KEY:-}" ]; then
  echo "GANNYU_RESOURCE_KEY is required for release builds" >&2
  exit 1
fi

for signing_var in ANDROID_KEYSTORE_BASE64 ANDROID_KEYSTORE_PASSWORD ANDROID_KEY_ALIAS ANDROID_KEY_PASSWORD; do
  if [ -z "${!signing_var:-}" ]; then
    echo "$signing_var is required for a signed Android release" >&2
    exit 1
  fi
done

RESOURCE_BUILD_ROOT="$(mktemp -d)"
trap 'rm -rf "$RESOURCE_BUILD_ROOT"' EXIT
export GONNYU_RESOURCE_DIR="$RESOURCE_BUILD_ROOT/bundle"
export RUSTFLAGS="--remap-path-prefix=$(realpath "$REPO_ROOT")=. ${RUSTFLAGS:-}"
echo "[pre] 构建运行资源"
"$CARGO_BIN" run --manifest-path "$REPO_ROOT/Cargo.toml" -p gonnyu-resource-build --release -- "$REPO_ROOT/resources" "$GONNYU_RESOURCE_DIR"

echo "[1/3] 编译 Rust Android 静态库"
for abi in arm64-v8a; do
  build_rust_target "$abi"
done

echo "[2/3] 编译 JNI 共享库"
build_jni_libs

STRIP_BIN="$NDK_ROOT/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-objcopy"
JNI_SO="$SCRIPT_DIR/app/src/main/jniLibs/arm64-v8a/libgannyu_input_jni.so"
echo "[strip] 清理 .so 符号与敏感字符串"
if [ -x "$STRIP_BIN" ]; then
    "$CARGO_BIN" run --manifest-path "$REPO_ROOT/Cargo.toml" -p gannyu-sanitize-binary --release -- "$JNI_SO" --strip-tool "$STRIP_BIN"
    echo "  ✅ Sanitized $(du -h "$JNI_SO" | cut -f1)"
else
    echo "  ⚠️  llvm-objcopy not found, skip strip"
fi

echo "[verify] 验证加密资源已嵌入 .so"
if grep -abq "GNYE" "$JNI_SO"; then
    GNYE_COUNT=$(grep -abo "GNYE" "$JNI_SO" | wc -l)
    echo "  ✅ 加密资源已确认 (GNYE magic × ${GNYE_COUNT})"
else
    echo "  ❌ 错误: .so 中未检测到加密资源 (GNYE magic)！"
    echo "     请确认 crates/ffi/build.rs 正常执行了资源加密。"
    exit 1
fi

"$CARGO_BIN" run --manifest-path "$REPO_ROOT/Cargo.toml" -p gannyu-sanitize-binary --release -- "$JNI_SO" --verify-release --repo-root "$REPO_ROOT"

echo "[3/3] 组装 Android release APK"
(
  cd "$SCRIPT_DIR"
  ANDROID_SDK_ROOT="$SDK_ROOT" PATH="$SHIM_DIR:$PATH" "$GRADLE_BIN" --no-daemon assembleRelease
)

if [ ! -f "$APK" ]; then
  echo "未生成 APK: $APK" >&2
  exit 1
fi

mkdir -p "$OUT_DIR/android"
PRODUCT_VERSION="$(awk -F '"' '/^version[[:space:]]*=/ { print $2; exit }' "$REPO_ROOT/Cargo.toml")"
OUT_APK="$OUT_DIR/android/GonnyuGeneralIME-${PRODUCT_VERSION}-android.apk"
cp "$APK" "$OUT_APK"

echo ""
echo "✅ APK 已生成: $OUT_APK"
echo "运行 install.sh 将 APK 安装到已连接的 Android 设备。"
