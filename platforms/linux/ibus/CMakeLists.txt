cmake_minimum_required(VERSION 3.20)
project(ibus-gannyu LANGUAGES C)

file(READ "${CMAKE_CURRENT_SOURCE_DIR}/../../../Cargo.toml" WORKSPACE_MANIFEST)
string(REGEX MATCH "workspace\\.package\\][\r\n]+version[ \t]*=[ \t]*\"([0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z.-]+)?)\"" VERSION_MATCH "${WORKSPACE_MANIFEST}")
if(NOT CMAKE_MATCH_1)
    message(FATAL_ERROR "Missing workspace package version")
endif()
set(GONNYU_VERSION "${CMAKE_MATCH_1}")

configure_file(gannyu.xml.in gannyu.xml @ONLY)

find_package(PkgConfig REQUIRED)
pkg_check_modules(IBUS REQUIRED ibus-1.0)

add_executable(ibus-engine-gannyu main.c)
target_include_directories(ibus-engine-gannyu PRIVATE
    ${IBUS_INCLUDE_DIRS}
    ${CMAKE_SOURCE_DIR}/../../../crates/ffi/include
)
target_link_directories(ibus-engine-gannyu PRIVATE ${IBUS_LIBRARY_DIRS})
target_compile_definitions(ibus-engine-gannyu PRIVATE GONNYU_VERSION="${GONNYU_VERSION}")
target_link_libraries(ibus-engine-gannyu PRIVATE
    ${IBUS_LIBRARIES}
    ${CMAKE_SOURCE_DIR}/../../../target/release/libgannyu_input_ffi.a
)
