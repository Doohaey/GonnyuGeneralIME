cmake_minimum_required(VERSION 3.20)
project(fcitx5-gannyu LANGUAGES CXX)

file(READ "${CMAKE_CURRENT_SOURCE_DIR}/../../../Cargo.toml" WORKSPACE_MANIFEST)
string(REGEX MATCH "workspace\\.package\\][\r\n]+version[ \t]*=[ \t]*\"([0-9]+\\.[0-9]+\\.[0-9]+)\"" VERSION_MATCH "${WORKSPACE_MANIFEST}")
if(NOT CMAKE_MATCH_1)
    message(FATAL_ERROR "Missing workspace package version")
endif()
set(GONNYU_VERSION "${CMAKE_MATCH_1}")

configure_file(gannyu-addon.conf.in gannyu-addon.conf @ONLY)
set(CMAKE_CXX_STANDARD 20)

find_package(Fcitx5Core REQUIRED)

add_library(fcitx5-gannyu MODULE gannyu_fcitx5.cpp)
target_include_directories(fcitx5-gannyu PRIVATE
    ${CMAKE_SOURCE_DIR}/../../../crates/ffi/include
)
target_link_libraries(fcitx5-gannyu PRIVATE
    Fcitx5::Core
    ${CMAKE_SOURCE_DIR}/../../../target/release/libgannyu_input_ffi.a
)
