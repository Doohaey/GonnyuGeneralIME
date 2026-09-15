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
  "$python_bin" "$script_dir/fetch_sources.py" >/dev/null
fi

librime_root="$source_root/librime"
[[ -d "$librime_root" ]] || { echo "missing librime source tree: $librime_root" >&2; exit 1; }
[[ -f "$librime_root/Makefile" ]] || { echo "missing librime Makefile: $librime_root" >&2; exit 1; }
boost_include="${GANNYU_RIME_BOOST_INCLUDE:-$source_root/boost}"
[[ -f "$boost_include/boost/version.hpp" ]] || { echo "missing pinned Boost headers: $boost_include" >&2; exit 1; }

# This directory contains only derived host tools and must never make a newer
# resource or source revision appear to have been rebuilt successfully.
rm -rf "$build_root"
mkdir -p "$build_root"

build_dependency() {
  local name="$1"
  shift
  cmake -S "$librime_root/deps/$name" -B "$build_root/deps/$name" "${generator_args[@]}" \
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" -DCMAKE_POSITION_INDEPENDENT_CODE=ON "$@"
  cmake --build "$build_root/deps/$name" --target install
}

# Do not invoke or rewrite librime's Makefile.  These explicit dependency
# builds are platform-neutral and leave the pinned source checkout untouched.
build_dependency glog -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DWITH_GFLAGS=OFF
build_dependency leveldb -DBUILD_SHARED_LIBS=OFF -DLEVELDB_BUILD_BENCHMARKS=OFF -DLEVELDB_BUILD_TESTS=OFF -DHAVE_CRC32C=OFF -DHAVE_SNAPPY=OFF -DHAVE_TCMALLOC=OFF
build_dependency marisa-trie -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DENABLE_TOOLS=OFF
build_dependency opencc -DBUILD_SHARED_LIBS=OFF -DENABLE_GTEST=OFF -DENABLE_BENCHMARK=OFF -DBUILD_PYTHON=OFF
build_dependency yaml-cpp -DBUILD_SHARED_LIBS=OFF -DYAML_CPP_BUILD_CONTRIB=OFF -DYAML_CPP_BUILD_TESTS=OFF -DYAML_CPP_BUILD_TOOLS=OFF

boost_build="$build_root/boost-regex"
cmake -S "$script_dir/boost_regex" -B "$boost_build" "${generator_args[@]}" \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$prefix" -DGANNYU_BOOST_ROOT="$boost_include"
cmake --build "$boost_build" --target install
boost_regex_library="$prefix/lib/libboost_regex.a"
[[ -f "$boost_regex_library" ]] || { echo "Boost.Regex was not built: $boost_regex_library" >&2; exit 1; }

env RIME_PLUGINS="librime-lua" cmake "${generator_args[@]}" "$librime_root" \
  -B"$build_root/build" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$prefix" \
  -DCMAKE_PREFIX_PATH="$prefix" \
  -DBoost_NO_BOOST_CMAKE=ON \
  -DBoost_NO_SYSTEM_PATHS=ON \
  -DBoost_INCLUDE_DIR="$boost_include" \
  -DBoost_INCLUDE_DIRS="$boost_include" \
  -DBoost_LIBRARY_DIRS="$prefix/lib" \
  -DBoost_REGEX_LIBRARY_RELEASE="$boost_regex_library" \
  -DBoost_LIBRARIES="$boost_regex_library" \
  -DBUILD_MERGED_PLUGINS=ON \
  -DENABLE_EXTERNAL_PLUGINS=OFF \
  -DBUILD_TEST=OFF \
  -DINSTALL_PRIVATE_HEADERS=ON

cmake --build "$build_root/build"
cmake --install "$build_root/build"

resources_dir="$build_root/gannyu-data"
"$python_bin" "$repo_root/platforms/rime/build.py" --region all --display-name apple --output "$resources_dir"

"$python_bin" - "$script_dir/engine-lock.json" "$build_root/build-summary.json" "$prefix" "$resources_dir" <<'PYTHON'
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
