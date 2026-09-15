cmake_minimum_required(VERSION 3.22)
project(gannyu_input_jni LANGUAGES C CXX)

get_filename_component(REPO_ROOT "${CMAKE_CURRENT_LIST_DIR}/../../../../../.." ABSOLUTE)

if(NOT ANDROID_ABI STREQUAL "arm64-v8a")
    message(FATAL_ERROR "The Rime mobile build currently supports arm64-v8a only: ${ANDROID_ABI}")
endif()

# The mobile toolchain writes all cross-compiled libraries below this ignored
# directory.  Keeping it outside the APK source tree prevents prebuilt binaries
# and large dictionaries from entering Git.
set(RIME_MOBILE_ROOT "${REPO_ROOT}/build/rime-mobile/android/${ANDROID_ABI}" CACHE PATH
    "Cross-compiled mobile Rime dependency root")
set(RIME_SOURCE_ROOT "${REPO_ROOT}/build/rime-mobile/sources/librime" CACHE PATH
    "Pinned librime source root used for public C headers")

set(RIME_ADAPTER_LIBRARY "${RIME_MOBILE_ROOT}/adapter/libgannyu_rime_engine.a")
set(RIME_LIBRARY "${RIME_MOBILE_ROOT}/librime/lib/librime.a")
set(RIME_PREFIX "${RIME_MOBILE_ROOT}/prefix")

foreach(required_path
    "${RIME_ADAPTER_LIBRARY}"
    "${RIME_LIBRARY}"
    "${RIME_PREFIX}/lib/libleveldb.a"
    "${RIME_PREFIX}/lib/libmarisa.a"
    "${RIME_PREFIX}/lib/libopencc.a"
    "${RIME_PREFIX}/lib/libyaml-cpp.a"
    "${RIME_PREFIX}/lib/libglog.a"
    "${RIME_PREFIX}/lib/libboost_regex.a"
    "${RIME_SOURCE_ROOT}/src/rime_api.h")
    if(NOT EXISTS "${required_path}")
        message(FATAL_ERROR "Missing mobile Rime build input: ${required_path}. Run platforms/rime/mobile/build_android.sh first.")
    endif()
endforeach()

add_library(gannyu_rime_engine STATIC IMPORTED)
set_target_properties(gannyu_rime_engine PROPERTIES IMPORTED_LOCATION "${RIME_ADAPTER_LIBRARY}")

add_library(librime STATIC IMPORTED)
set_target_properties(librime PROPERTIES IMPORTED_LOCATION "${RIME_LIBRARY}")

add_library(gannyu_input_jni SHARED jni_bridge.c)
target_include_directories(gannyu_input_jni PRIVATE
    "${REPO_ROOT}/crates/ffi/include"
    "${RIME_SOURCE_ROOT}/src")

# librime uses static registrars for its core modules and merged Lua plugin.
# Pull the complete archive so those registrars are retained in the final JNI
# library; ordinary archive linking silently drops them and leaves no schemas at
# runtime.
target_link_libraries(gannyu_input_jni PRIVATE
    "-Wl,--whole-archive"
    gannyu_rime_engine
    librime
    "-Wl,--no-whole-archive"
    "${RIME_PREFIX}/lib/libleveldb.a"
    "${RIME_PREFIX}/lib/libmarisa.a"
    "${RIME_PREFIX}/lib/libopencc.a"
    "${RIME_PREFIX}/lib/libyaml-cpp.a"
    "${RIME_PREFIX}/lib/libglog.a"
    "${RIME_PREFIX}/lib/libboost_regex.a"
    log
    android
    dl
    z)
set_target_properties(gannyu_input_jni PROPERTIES LINKER_LANGUAGE CXX)
target_link_options(gannyu_input_jni PRIVATE "-Wl,--exclude-libs,ALL")
