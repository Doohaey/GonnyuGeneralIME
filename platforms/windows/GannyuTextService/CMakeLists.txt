cmake_minimum_required(VERSION 3.20)
project(GannyuTextService LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_library(GannyuTextService SHARED
    GannyuTextService.cpp
    GannyuTextService.def
)

target_include_directories(GannyuTextService PRIVATE
    ${CMAKE_SOURCE_DIR}/../../../crates/ffi/include
)

target_compile_definitions(GannyuTextService PRIVATE
    UNICODE
    _UNICODE
    WIN32_LEAN_AND_MEAN
    NOMINMAX
)

if (MSVC)
    target_compile_options(GannyuTextService PRIVATE /utf-8)
    set_property(TARGET GannyuTextService PROPERTY
        MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    )
endif()

target_link_libraries(GannyuTextService PRIVATE
    ${CMAKE_SOURCE_DIR}/../../../target/release/gannyu_input_ffi.lib
    advapi32
    gdi32
    ntdll
    ole32
    shlwapi
    user32
    # Rust std on Windows links against these system libraries.
    ws2_32
    userenv
    bcrypt
)

set_target_properties(GannyuTextService PROPERTIES
    OUTPUT_NAME "GannyuTextService"
    SUFFIX ".dll"
)
