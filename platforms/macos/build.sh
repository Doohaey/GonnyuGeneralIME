#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
test_env="$script_dir/test_local.env"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "this builder must run on macOS" >&2
  exit 1
fi

if [[ -f "$test_env" ]]; then
  set -a
  source "$test_env"
  set +a
fi

command -v cargo >/dev/null || { echo "cargo not found; install rustup first" >&2; exit 1; }
command -v swift >/dev/null || { echo "swift not found; install Xcode Command Line Tools first" >&2; exit 1; }

cd "$repo_root"
cargo build -p gannyu-input-ffi --release
swift build --package-path "$script_dir" -c release
