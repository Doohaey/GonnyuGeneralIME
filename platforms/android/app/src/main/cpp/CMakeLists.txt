cmake_minimum_required(VERSION 3.22)
project(gannyu_input_jni LANGUAGES C)

get_filename_component(REPO_ROOT "${CMAKE_CURRENT_LIST_DIR}/../../../../../.." ABSOLUTE)

if(ANDROID_ABI STREQUAL "arm64-v8a")
    set(RUST_TARGET "aarch64-linux-android")
elseif(ANDROID_ABI STREQUAL "armeabi-v7a")
    set(RUST_TARGET "armv7-linux-androideabi")
elseif(ANDROID_ABI STREQUAL "x86_64")
    set(RUST_TARGET "x86_64-linux-android")
else()
    message(FATAL_ERROR "Unsupported Android ABI: ${ANDROID_ABI}")
endif()

set(RUST_LIB "${REPO_ROOT}/target/${RUST_TARGET}/release/libgannyu_input_ffi.a")
if(NOT EXISTS "${RUST_LIB}")
    message(FATAL_ERROR "Missing Rust static library: ${RUST_LIB}. Run platforms/android/build_and_install.sh first.")
endif()

add_library(gannyu_input_ffi STATIC IMPORTED)
set_target_properties(gannyu_input_ffi PROPERTIES IMPORTED_LOCATION "${RUST_LIB}")

add_library(gannyu_input_jni SHARED jni_bridge.c)
target_include_directories(gannyu_input_jni PRIVATE
    "${REPO_ROOT}/crates/ffi/include")
target_link_libraries(gannyu_input_jni PRIVATE gannyu_input_ffi log)
