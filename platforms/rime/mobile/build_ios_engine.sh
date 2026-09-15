#!/usr/bin/env bash
# Cross-build one architecture slice of the pinned Rime engine for Apple mobile.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
source_root="${GANNYU_RIME_MOBILE_SOURCE_ROOT:-$repo_root/build/rime-mobile/sources}"
sdk_name="${GANNYU_IOS_SDK:-iphoneos}"
architecture="${GANNYU_IOS_ARCH:-arm64}"
case "$sdk_name" in
  iphoneos|iphonesimulator) ;;
  *) echo "unsupported Apple SDK: $sdk_name" >&2; exit 2 ;;
esac
build_root="${GANNYU_RIME_MOBILE_IOS_BUILD_ROOT:-$repo_root/build/rime-mobile/ios/$sdk_name-$architecture}"
sdk="$(xcrun --sdk "$sdk_name" --show-sdk-path)"
prefix="$build_root/prefix"
librime_root="$source_root/librime"
python_bin="${PYTHON_BIN:-python3}"

if ! "$python_bin" -c 'import tomllib' >/dev/null 2>&1; then
  for candidate in /opt/homebrew/bin/python3 python3.14 python3.13 python3.12 python3.11; do
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

command -v cmake >/dev/null || { echo "cmake is required" >&2; exit 2; }
command -v ninja >/dev/null || { echo "ninja is required" >&2; exit 2; }

if [[ ! -d "$librime_root/.git" ]]; then
  "$python_bin" "$script_dir/fetch_sources.py" >/dev/null
fi
[[ -d "$librime_root" ]] || { echo "missing pinned librime source tree: $librime_root" >&2; exit 2; }

# librime uses Boost headers only.  The default tree comes from the checksummed
# release archive in engine-lock.json; an override is intended for CI mirrors.
boost_include="${GANNYU_RIME_BOOST_INCLUDE:-$source_root/boost}"
if [[ ! -f "$boost_include/boost/version.hpp" && -z "${GANNYU_RIME_BOOST_INCLUDE:-}" ]]; then
  "$python_bin" "$script_dir/fetch_sources.py" --boost-only >/dev/null
fi
[[ -f "$boost_include/boost/version.hpp" ]] || {
  echo "set GANNYU_RIME_BOOST_INCLUDE to a Boost include directory" >&2
  exit 2
}

common_cmake=(
  -G Ninja
  -DCMAKE_SYSTEM_NAME=iOS
  -DCMAKE_OSX_SYSROOT="$sdk"
  -DCMAKE_OSX_ARCHITECTURES="$architecture"
  -DCMAKE_OSX_DEPLOYMENT_TARGET=16.0
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

rm -rf "$build_root"
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

boost_build="$build_root/boost-regex"
cmake -S "$script_dir/boost_regex" -B "$boost_build" "${common_cmake[@]}" \
  -DGANNYU_BOOST_ROOT="$boost_include"
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
  -DCMAKE_C_FLAGS="-DLUA_USE_IOS -Wno-macro-redefined" \
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

echo "built $sdk_name $architecture Rime engine: $build_root"
