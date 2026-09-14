#!/usr/bin/env bash
# Cross-build the pinned Rime engine for iPhoneOS arm64.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
source_root="${GANNYU_RIME_MOBILE_SOURCE_ROOT:-$repo_root/build/rime-mobile/sources}"
build_root="${GANNYU_RIME_MOBILE_IOS_BUILD_ROOT:-$repo_root/build/rime-mobile/ios/iphoneos-arm64}"
sdk="$(xcrun --sdk iphoneos --show-sdk-path)"
prefix="$build_root/prefix"
librime_root="$source_root/librime"

command -v cmake >/dev/null || { echo "cmake is required" >&2; exit 2; }
command -v ninja >/dev/null || { echo "ninja is required" >&2; exit 2; }

if [[ ! -d "$librime_root/.git" ]]; then
  python3 "$script_dir/fetch_sources.py" >/dev/null
fi
[[ -d "$librime_root" ]] || { echo "missing pinned librime source tree: $librime_root" >&2; exit 2; }

# librime's Boost use is header-only for this target.  The include directory is
# explicit rather than silently taking arbitrary SDK paths.  A later release
# gate replaces this bootstrap include with a pinned target-built Boost tree.
boost_include="${GANNYU_RIME_BOOST_INCLUDE:-}"
if [[ -z "$boost_include" ]] && command -v brew >/dev/null 2>&1; then
  boost_include="$(brew --prefix boost 2>/dev/null || true)/include"
fi
[[ -f "$boost_include/boost/version.hpp" ]] || {
  echo "set GANNYU_RIME_BOOST_INCLUDE to a Boost include directory" >&2
  exit 2
}

common_cmake=(
  -G Ninja
  -DCMAKE_SYSTEM_NAME=iOS
  -DCMAKE_OSX_SYSROOT="$sdk"
  -DCMAKE_OSX_ARCHITECTURES=arm64
  -DCMAKE_TRY_COMPILE_TARGET_TYPE=STATIC_LIBRARY
  -DCMAKE_BUILD_TYPE=Release
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON
  -DCMAKE_INSTALL_PREFIX="$prefix"
)

build_dependency() {
  local name="$1"
  shift
  local source="$librime_root/deps/$name"
  local output="$build_root/deps/$name"
  cmake -S "$source" -B "$output" "${common_cmake[@]}" "$@"
  cmake --build "$output" --target install
}

mkdir -p "$build_root"
build_dependency glog \
  -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DWITH_GFLAGS=OFF
build_dependency leveldb \
  -DBUILD_SHARED_LIBS=OFF -DLEVELDB_BUILD_BENCHMARKS=OFF -DLEVELDB_BUILD_TESTS=OFF \
  -DHAVE_CRC32C=OFF -DHAVE_SNAPPY=OFF -DHAVE_TCMALLOC=OFF
build_dependency marisa-trie \
  -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DENABLE_TOOLS=OFF
# OpenCC unconditionally declares command-line tools.  iPhoneOS cannot install
# executable bundles from this dependency build, so build and stage its library
# and public headers directly instead of invoking its install target.
opencc_build="$build_root/deps/opencc"
opencc_source="$build_root/opencc-source"
opencc_patch="$script_dir/patches/opencc-ios-no-tools.patch"
rm -rf "$opencc_build"
rm -rf "$opencc_source"
cmake -E copy_directory "$librime_root/deps/opencc" "$opencc_source"
patch -d "$opencc_source" -p1 < "$opencc_patch"
cmake -S "$opencc_source" -B "$opencc_build" "${common_cmake[@]}" \
  -DBUILD_SHARED_LIBS=OFF -DENABLE_GTEST=OFF -DENABLE_BENCHMARK=OFF -DBUILD_PYTHON=OFF \
  -DUSE_SYSTEM_MARISA=ON -DLIBMARISA="$prefix/lib/libmarisa.a" \
  -DCMAKE_CXX_FLAGS="-I$prefix/include"
cmake --build "$opencc_build" --target libopencc
mkdir -p "$prefix/lib" "$prefix/include/opencc"
cp "$opencc_build/src/libopencc.a" "$prefix/lib/libopencc.a"
cp "$opencc_source"/src/*.hpp "$opencc_source/src/opencc.h" "$opencc_build/src/opencc_config.h" "$prefix/include/opencc/"
build_dependency yaml-cpp \
  -DBUILD_SHARED_LIBS=OFF -DYAML_CPP_BUILD_CONTRIB=OFF -DYAML_CPP_BUILD_TESTS=OFF -DYAML_CPP_BUILD_TOOLS=OFF

rm -rf "$build_root/librime" "$build_root/adapter"
env RIME_PLUGINS="librime-lua" cmake -S "$librime_root" -B "$build_root/librime" "${common_cmake[@]}" \
  -DCMAKE_PREFIX_PATH="$prefix" \
  -DBoost_NO_SYSTEM_PATHS=ON \
  -DBoost_INCLUDE_DIR="$boost_include" \
  -DBUILD_SHARED_LIBS=OFF \
  -DBUILD_STATIC=ON \
  -DBUILD_MERGED_PLUGINS=ON \
  -DCMAKE_C_FLAGS="-DLUA_USE_IOS" \
  -DENABLE_EXTERNAL_PLUGINS=OFF \
  -DBUILD_TEST=OFF \
  -DINSTALL_PRIVATE_HEADERS=ON \
  -DGlog_INCLUDE_PATH="$prefix/include" -DGlog_LIBRARY="$prefix/lib/libglog.a" \
  -DYamlCpp_INCLUDE_PATH="$prefix/include" -DYamlCpp_NEW_API="$prefix/include" -DYamlCpp_LIBRARY="$prefix/lib/libyaml-cpp.a" \
  -DLevelDb_INCLUDE_PATH="$prefix/include" -DLevelDb_LIBRARY="$prefix/lib/libleveldb.a" \
  -DMarisa_INCLUDE_PATH="$prefix/include" -DMarisa_LIBRARY="$prefix/lib/libmarisa.a" \
  -DOpencc_INCLUDE_PATH="$prefix/include" -DOpencc_LIBRARY="$prefix/lib/libopencc.a"
cmake --build "$build_root/librime"
cmake --install "$build_root/librime"

cmake -S "$repo_root/engines/rime" -B "$build_root/adapter" "${common_cmake[@]}" \
  -DCMAKE_PREFIX_PATH="$prefix" \
  -DRIME_INCLUDE_DIR="$prefix/include" \
  -DRIME_LIBRARY="$prefix/lib/librime.a" \
  -DGANNYU_RIME_BUILD_PROBES=OFF
cmake --build "$build_root/adapter"

echo "built iPhoneOS Rime engine: $build_root"
