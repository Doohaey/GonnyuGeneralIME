#!/usr/bin/env bash
# env_setup.sh — 设置 Android 构建环境变量
# 用法: source platforms/android/env_setup.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CACHE_ROOT="${GANNYU_ANDROID_CACHE:-$REPO_ROOT/../../_Cache}"

# 默认使用仓库旁的本地缓存；也可用 GANNYU_ANDROID_CACHE 覆盖。
export ANDROID_SDK_ROOT="${ANDROID_SDK_ROOT:-$CACHE_ROOT}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$CACHE_ROOT/android-ndk-r26b}"
export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$CACHE_ROOT/home-cache/gradle}"

# 使用项目 Gradle wrapper；其会自动选择兼容的 JDK。
export GRADLE_BIN="${GRADLE_BIN:-$SCRIPT_DIR/gradlew}"

echo "Android 构建环境已就绪（本机路径不会写入仓库）"
echo "  SDK: ready"
echo "  NDK: ready"
