#!/usr/bin/env bash
# Cross-build the pinned Rime engine used only while packaging Android.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/../../.." && pwd)"
abi="${GANNYU_ANDROID_ABI:-arm64-v8a}"
sdk_root="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Library/Android/sdk}}"
ndk_root="${ANDROID_NDK_HOME:-${ANDROID_NDK_ROOT:-}}"
source_root="${GANNYU_RIME_MOBILE_SOURCE_ROOT:-$repo_root/build/rime-mobile/sources}"
build_root="$repo_root/build/rime-mobile/android/$abi"
prefix="$build_root/prefix"
host_build_root="${GANNYU_RIME_MOBILE_BUILD_ROOT:-$repo_root/build/rime-mobile/host}"
host_bin_dir="$host_build_root/prefix/bin"
python_bin="${PYTHON_BIN:-python3}"

[[ "$abi" == "arm64-v8a" ]] || { echo "only arm64-v8a is supported" >&2; exit 2; }
if [[ -z "$ndk_root" && -d "$sdk_root/ndk" ]]; then
  ndk_root="$(find "$sdk_root/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)"
fi
[[ -f "$ndk_root/build/cmake/android.toolchain.cmake" ]] || { echo "Android NDK not found" >&2; exit 2; }

if ! "$python_bin" -c 'import tomllib' >/dev/null 2>&1; then
  for candidate in /opt/homebrew/bin/python3 python3.14 python3.13 python3.12 python3.11; do
    if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import tomllib' >/dev/null 2>&1; then python_bin="$candidate"; break; fi
  done
fi
"$python_bin" -c 'import tomllib' >/dev/null 2>&1 || { echo "Python 3.11+ is required" >&2; exit 2; }

if [[ ! -d "$source_root/librime/.git" ]]; then "$python_bin" "$script_dir/fetch_sources.py" >/dev/null; fi
librime_root="$source_root/librime"
patched_librime_root="$build_root/librime-source"
boost_include="${GANNYU_RIME_BOOST_INCLUDE:-$source_root/boost}"
if [[ ! -f "$boost_include/boost/version.hpp" && -z "${GANNYU_RIME_BOOST_INCLUDE:-}" ]]; then
  "$python_bin" "$script_dir/fetch_sources.py" --boost-only >/dev/null
fi
[[ -f "$boost_include/boost/version.hpp" ]] || { echo "Boost headers not found" >&2; exit 2; }

common=(
  -G Ninja
  -DCMAKE_SYSTEM_NAME=Android
  -DCMAKE_TOOLCHAIN_FILE="$ndk_root/build/cmake/android.toolchain.cmake"
  -DANDROID_ABI="$abi" -DANDROID_PLATFORM=android-24 -DANDROID_STL=c++_shared
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_POSITION_INDEPENDENT_CODE=ON
  -DCMAKE_INSTALL_PREFIX="$prefix"
)
build_dependency() {
  local name="$1"; shift
  cmake -S "$librime_root/deps/$name" -B "$build_root/deps/$name" "${common[@]}" "$@"
  cmake --build "$build_root/deps/$name" --target install
}

build_dependency glog -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DWITH_GFLAGS=OFF
build_dependency leveldb -DBUILD_SHARED_LIBS=OFF -DLEVELDB_BUILD_BENCHMARKS=OFF -DLEVELDB_BUILD_TESTS=OFF -DHAVE_CRC32C=OFF -DHAVE_SNAPPY=OFF -DHAVE_TCMALLOC=OFF
build_dependency marisa-trie -DBUILD_SHARED_LIBS=OFF -DBUILD_TESTING=OFF -DENABLE_TOOLS=OFF
[[ -x "$host_bin_dir/opencc_dict" ]] || { echo "Missing host opencc_dict: $host_bin_dir/opencc_dict" >&2; exit 2; }
PATH="$host_bin_dir:$PATH" build_dependency opencc -DBUILD_SHARED_LIBS=OFF -DENABLE_GTEST=OFF -DENABLE_BENCHMARK=OFF -DBUILD_PYTHON=OFF
build_dependency yaml-cpp -DBUILD_SHARED_LIBS=OFF -DYAML_CPP_BUILD_CONTRIB=OFF -DYAML_CPP_BUILD_TESTS=OFF -DYAML_CPP_BUILD_TOOLS=OFF

rm -rf "$patched_librime_root" "$build_root/librime" "$build_root/adapter"
cmake -E copy_directory "$librime_root" "$patched_librime_root"
perl -0pi -e 's/if\\(LINUX\\)\\n  find_package\\(Boost 1\\.74\\.0 REQUIRED COMPONENTS regex\\)\\nelse\\(\\)\\n  find_package\\(Boost 1\\.77\\.0\\)\\nendif\\(\\)/if(GANNYU_MOBILE_USE_STD_REGEX)\\n  set(Boost_FOUND TRUE)\\n  set(Boost_INCLUDE_DIRS \\$\\{Boost_INCLUDE_DIR\\})\\n  set(Boost_LIBRARY_DIRS \"\")\\n  set(Boost_LIBRARIES \"\")\\nelseif(LINUX)\\n  find_package(Boost 1.74.0 REQUIRED COMPONENTS regex)\\nelse()\\n  find_package(Boost 1.77.0 REQUIRED)\\nendif()/s' "$patched_librime_root/CMakeLists.txt"
perl -0pi -e 's/boost::regex_error/std::regex_error/g' "$patched_librime_root/src/rime/algo/algebra.cc"
perl -0pi -e 's/boost::regex_replace/std::regex_replace/g; s/boost::regex_match/std::regex_match/g' "$patched_librime_root/src/rime/algo/calculus.cc"
perl -0pi -e 's/#include <boost\\/regex\\.hpp>/#include <regex>/; s/boost::regex/std::regex/g' "$patched_librime_root/src/rime/algo/calculus.h"
perl -0pi -e 's/boost::regex/std::regex/g; s/boost::regex_match/std::regex_match/g' "$patched_librime_root/src/rime/algo/encoder.cc"
perl -0pi -e 's/#include <boost\\/regex\\.hpp>/#include <regex>/; s/vector<boost::regex>/vector<std::regex>/g' "$patched_librime_root/src/rime/algo/encoder.h"
grep -q 'if(GANNYU_MOBILE_USE_STD_REGEX)' "$patched_librime_root/CMakeLists.txt" || { echo "Failed to rewrite Android librime Boost detection" >&2; exit 2; }
grep -q 'std::regex_error' "$patched_librime_root/src/rime/algo/algebra.cc" || { echo "Failed to rewrite Android librime regex usage" >&2; exit 2; }
sed -n '64,74p' "$patched_librime_root/CMakeLists.txt"

cmake -S "$patched_librime_root" -B "$build_root/librime" "${common[@]}" \
  -DCMAKE_PREFIX_PATH="$prefix" -DGANNYU_MOBILE_USE_STD_REGEX=ON -DBoost_NO_BOOST_CMAKE=ON -DBoost_NO_SYSTEM_PATHS=ON -DBoost_INCLUDE_DIR="$boost_include" \
  -DBUILD_SHARED_LIBS=OFF -DBUILD_STATIC=ON -DBUILD_MERGED_PLUGINS=ON -DENABLE_EXTERNAL_PLUGINS=OFF -DBUILD_TEST=OFF -DINSTALL_PRIVATE_HEADERS=ON \
  -DGlog_INCLUDE_PATH="$prefix/include" -DGlog_LIBRARY="$prefix/lib/libglog.a" \
  -DYamlCpp_INCLUDE_PATH="$prefix/include" -DYamlCpp_NEW_API="$prefix/include/yaml-cpp/node/node.h" -DYamlCpp_LIBRARY="$prefix/lib/libyaml-cpp.a" \
  -DLevelDb_INCLUDE_PATH="$prefix/include" -DLevelDb_LIBRARY="$prefix/lib/libleveldb.a" \
  -DMarisa_INCLUDE_PATH="$prefix/include" -DMarisa_LIBRARY="$prefix/lib/libmarisa.a" \
  -DOpencc_INCLUDE_PATH="$prefix/include" -DOpencc_LIBRARY="$prefix/lib/libopencc.a"
cmake --build "$build_root/librime"
cmake --install "$build_root/librime"
cmake -S "$repo_root/engines/rime" -B "$build_root/adapter" "${common[@]}" \
  -DCMAKE_PREFIX_PATH="$prefix" -DRIME_INCLUDE_DIR="$prefix/include" -DRIME_LIBRARY="$prefix/lib/librime.a" -DGANNYU_RIME_BUILD_PROBES=OFF
cmake --build "$build_root/adapter"

echo "built Android $abi Rime engine: $build_root"
