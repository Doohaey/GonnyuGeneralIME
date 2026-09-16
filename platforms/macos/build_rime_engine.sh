#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../.." && pwd)"
mobile_dir="$repo_root/platforms/rime/mobile"
source_root="${GANNYU_RIME_MOBILE_SOURCE_ROOT:-$repo_root/build/rime-mobile/sources}"
build_root="${GANNYU_MACOS_RIME_BUILD_ROOT:-$repo_root/build/rime-macos}"
prefix="$build_root/prefix"
architectures="${GANNYU_MACOS_ARCHITECTURES:-arm64;x86_64}"
deployment_target="${GANNYU_MACOS_DEPLOYMENT_TARGET:-13.0}"
python_bin="${PYTHON_BIN:-python3}"

command -v cmake >/dev/null || { echo "cmake is required" >&2; exit 2; }
command -v ninja >/dev/null || { echo "ninja is required" >&2; exit 2; }
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
  exit 2
}

if [[ ! -d "$source_root/librime/.git" ]]; then
  "$python_bin" "$mobile_dir/fetch_sources.py" >/dev/null
fi
librime_root="$source_root/librime"
for component in librime librime_lua librime_lua_thirdparty; do
  expected="$("$python_bin" -c 'import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]]["commit"])' "$mobile_dir/engine-lock.json" "$component")"
  case "$component" in
    librime) checkout="$librime_root" ;;
    librime_lua) checkout="$librime_root/plugins/librime-lua" ;;
    librime_lua_thirdparty) checkout="$librime_root/plugins/librime-lua/thirdparty" ;;
  esac
  actual="$(git -C "$checkout" rev-parse HEAD)"
  [[ "$actual" == "$expected" ]] || { echo "locked $component source mismatch: $actual" >&2; exit 2; }
done
boost_include="${GANNYU_RIME_BOOST_INCLUDE:-$source_root/boost}"
if [[ ! -f "$boost_include/boost/version.hpp" && -z "${GANNYU_RIME_BOOST_INCLUDE:-}" ]]; then
  "$python_bin" "$mobile_dir/fetch_sources.py" --boost-only >/dev/null
fi
[[ -f "$boost_include/boost/version.hpp" ]] || {
  echo "set GANNYU_RIME_BOOST_INCLUDE to a Boost include directory" >&2
  exit 2
}

common_cmake=(
  -G Ninja
  -DCMAKE_BUILD_TYPE=Release
  -DCMAKE_INSTALL_PREFIX="$prefix"
  -DCMAKE_OSX_ARCHITECTURES="$architectures"
  -DCMAKE_OSX_DEPLOYMENT_TARGET="$deployment_target"
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON
)

rm -rf "$build_root"
mkdir -p "$build_root"

run_with_heartbeat() {
  local label="$1"
  shift
  local pid

  echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] starting: $label"
  "$@" &
  pid=$!
  while kill -0 "$pid" 2>/dev/null; do
    sleep 60
    if kill -0 "$pid" 2>/dev/null; then
      echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] still running: $label (pid $pid)"
    fi
  done
  wait "$pid"
  echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] completed: $label"
}

build_dependency() {
  local name="$1"
  shift
  run_with_heartbeat "$name configure" \
    cmake -S "$librime_root/deps/$name" -B "$build_root/deps/$name" "${common_cmake[@]}" "$@"
  run_with_heartbeat "$name build" \
    cmake --build "$build_root/deps/$name" --target install
}

build_dependency glog -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DWITH_GFLAGS=OFF
build_dependency leveldb -DBUILD_SHARED_LIBS=OFF -DLEVELDB_BUILD_BENCHMARKS=OFF -DLEVELDB_BUILD_TESTS=OFF -DHAVE_CRC32C=OFF -DHAVE_SNAPPY=OFF -DHAVE_TCMALLOC=OFF
build_dependency marisa-trie -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DENABLE_TOOLS=OFF
build_dependency opencc -DBUILD_SHARED_LIBS=OFF -DENABLE_GTEST=OFF -DENABLE_BENCHMARK=OFF -DBUILD_PYTHON=OFF
build_dependency yaml-cpp -DBUILD_SHARED_LIBS=OFF -DYAML_CPP_BUILD_CONTRIB=OFF -DYAML_CPP_BUILD_TESTS=OFF -DYAML_CPP_BUILD_TOOLS=OFF

boost_build="$build_root/boost-regex"
cmake -S "$mobile_dir/boost_regex" -B "$boost_build" "${common_cmake[@]}" \
  -DGANNYU_BOOST_ROOT="$boost_include"
run_with_heartbeat "boost-regex build" \
  cmake --build "$boost_build" --target install
boost_regex_library="$prefix/lib/libboost_regex.a"
[[ -f "$boost_regex_library" ]] || { echo "Boost.Regex was not built: $boost_regex_library" >&2; exit 2; }

env RIME_PLUGINS="librime-lua" cmake -S "$librime_root" -B "$build_root/librime" "${common_cmake[@]}" \
  -DCMAKE_PREFIX_PATH="$prefix" \
  -DBoost_NO_BOOST_CMAKE=ON \
  -DBoost_NO_SYSTEM_PATHS=ON \
  -DBoost_INCLUDE_DIR="$boost_include" \
  -DBoost_INCLUDE_DIRS="$boost_include" \
  -DBoost_LIBRARY_DIRS="$prefix/lib" \
  -DBoost_REGEX_LIBRARY_RELEASE="$boost_regex_library" \
  -DBoost_LIBRARIES="$boost_regex_library" \
  -DBUILD_SHARED_LIBS=OFF \
  -DBUILD_STATIC=ON \
  -DBUILD_MERGED_PLUGINS=ON \
  -DENABLE_EXTERNAL_PLUGINS=OFF \
  -DBUILD_TEST=OFF \
  -DINSTALL_PRIVATE_HEADERS=ON \
  -DGlog_INCLUDE_PATH="$prefix/include" -DGlog_LIBRARY="$prefix/lib/libglog.a" \
  -DYamlCpp_INCLUDE_PATH="$prefix/include" -DYamlCpp_NEW_API="$prefix/include" -DYamlCpp_LIBRARY="$prefix/lib/libyaml-cpp.a" \
  -DLevelDb_INCLUDE_PATH="$prefix/include" -DLevelDb_LIBRARY="$prefix/lib/libleveldb.a" \
  -DMarisa_INCLUDE_PATH="$prefix/include" -DMarisa_LIBRARY="$prefix/lib/libmarisa.a" \
  -DOpencc_INCLUDE_PATH="$prefix/include" -DOpencc_LIBRARY="$prefix/lib/libopencc.a"
run_with_heartbeat "librime build" cmake --build "$build_root/librime"
run_with_heartbeat "librime install" cmake --install "$build_root/librime"

cmake -S "$repo_root/engines/rime" -B "$build_root/adapter" "${common_cmake[@]}" \
  -DCMAKE_PREFIX_PATH="$prefix" \
  -DRIME_INCLUDE_DIR="$prefix/include" \
  -DRIME_LIBRARY="$prefix/lib/librime.a" \
  -DRIME_DEPENDENCY_LIBRARIES="$prefix/lib/libleveldb.a;$prefix/lib/libmarisa.a;$prefix/lib/libopencc.a;$prefix/lib/libyaml-cpp.a;$prefix/lib/libglog.a;$prefix/lib/libboost_regex.a" \
  -DGANNYU_RIME_BUILD_PROBES=ON
run_with_heartbeat "Gannyu Rime adapter build" cmake --build "$build_root/adapter"

for library in "$build_root/adapter/libgannyu_rime_engine.a" "$prefix/lib/librime.a"; do
  [[ -f "$library" ]] || { echo "missing macOS Rime artifact: $library" >&2; exit 2; }
  IFS=';' read -r -a architecture_list <<< "$architectures"
  for architecture in "${architecture_list[@]}"; do
    lipo "$library" -verify_arch "$architecture"
  done
done

echo "built macOS universal Rime engine: $build_root"
