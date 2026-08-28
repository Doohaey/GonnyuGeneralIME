#!/usr/bin/env bash
set -euo pipefail
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
REPO_ROOT="$( cd "$SCRIPT_DIR/../../.." && pwd )"

if [ -z "${GANNYU_RESOURCE_KEY:-}" ]; then
  echo "GANNYU_RESOURCE_KEY is required for release builds" >&2
  exit 1
fi

for pkg in cargo cmake pkg-config; do
  command -v "$pkg" >/dev/null 2>&1 || { echo "缺 $pkg" >&2; exit 1; }
done
pkg-config --exists ibus-1.0 || { echo "缺 ibus 开发包: sudo apt install libibus-1.0-dev" >&2; exit 1; }

RESOURCE_BUILD_ROOT="$(mktemp -d)"
trap 'rm -rf "$RESOURCE_BUILD_ROOT"' EXIT
export GONNYU_RESOURCE_DIR="$RESOURCE_BUILD_ROOT/bundle"
export RUSTFLAGS="--remap-path-prefix=$(realpath "$REPO_ROOT")=. ${RUSTFLAGS:-}"
echo "[1/3] 构建运行资源"
(cd "$REPO_ROOT" && cargo run -p gonnyu-resource-build --release -- "$REPO_ROOT/resources" "$GONNYU_RESOURCE_DIR")

echo "[2/3] 编译 Rust FFI"
(cd "$REPO_ROOT" && cargo build -p gannyu-input-ffi --release)

echo "[3/3] CMake build ibus engine"
BUILD_DIR="$REPO_ROOT/build/linux-ibus"
cmake -S "$SCRIPT_DIR" -B "$BUILD_DIR" -DCMAKE_BUILD_TYPE=Release
cmake --build "$BUILD_DIR" -j

cargo run -p gannyu-sanitize-binary --release -- "$BUILD_DIR/ibus-engine-gannyu"
cargo run -p gannyu-sanitize-binary --release -- "$BUILD_DIR/ibus-engine-gannyu" --verify-release --repo-root "$REPO_ROOT"

echo "构建完成: $BUILD_DIR"
echo "运行 install.sh 将制品安装到系统。"

