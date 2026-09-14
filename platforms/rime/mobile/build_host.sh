#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
source_root="${GANNYU_RIME_MOBILE_SOURCE_ROOT:-$repo_root/build/rime-mobile/sources}"
build_root="${GANNYU_RIME_MOBILE_BUILD_ROOT:-$repo_root/build/rime-mobile/host}"
prefix="$build_root/prefix"
generator_args=()
python_bin="${PYTHON_BIN:-python3}"

if ! "$python_bin" -c 'import tomllib' >/dev/null 2>&1; then
  for candidate in python3.13 python3.12 python3.11 /opt/homebrew/bin/python3; do
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

if command -v ninja >/dev/null 2>&1; then
  generator_args=(-G Ninja)
fi

if [[ ! -d "$source_root/librime/.git" ]]; then
  source_root="$("$python_bin" "$script_dir/fetch_sources.py")"
fi

librime_root="$source_root/librime"
[[ -d "$librime_root" ]] || { echo "missing librime source tree: $librime_root" >&2; exit 1; }
[[ -f "$librime_root/Makefile" ]] || { echo "missing librime Makefile: $librime_root" >&2; exit 1; }

mkdir -p "$build_root"

make -C "$librime_root" deps prefix="$prefix" build=build-host-deps

env RIME_PLUGINS="librime-lua" cmake "${generator_args[@]}" "$librime_root" \
  -B"$build_root/build" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$prefix" \
  -DCMAKE_PREFIX_PATH="$prefix" \
  -DBUILD_MERGED_PLUGINS=ON \
  -DENABLE_EXTERNAL_PLUGINS=OFF \
  -DBUILD_TEST=OFF \
  -DINSTALL_PRIVATE_HEADERS=ON

cmake --build "$build_root/build"
cmake --install "$build_root/build"

resources_dir="$build_root/gannyu-data"
"$python_bin" "$repo_root/platforms/rime/build.py" --region all --display-name apple --output "$resources_dir"

python3 - "$script_dir/engine-lock.json" "$build_root/build-summary.json" "$prefix" "$resources_dir" <<'PYTHON'
import json
import sys
from pathlib import Path

lock_path = Path(sys.argv[1])
summary_path = Path(sys.argv[2])
prefix = Path(sys.argv[3])
resources = Path(sys.argv[4])
lock = json.loads(lock_path.read_text(encoding="utf-8"))
summary = {
    "librime": lock["librime"],
    "librime_lua": lock["librime_lua"],
    "install_prefix": str(prefix),
    "resource_manifest": str(resources / "resource-manifest.json"),
}
summary_path.write_text(
    json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)
PYTHON

echo "built host Rime validation tree in $build_root"
