cmake_minimum_required(VERSION 3.20)
project(GannyuTextService LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(GANNYU_FFI_LIBRARY
    "${CMAKE_SOURCE_DIR}/../../../target/release/gannyu_input_ffi.lib"
    CACHE FILEPATH "Path to the architecture-matched gannyu_input_ffi static library"
)
if (NOT EXISTS "${GANNYU_FFI_LIBRARY}")
    message(FATAL_ERROR "Missing architecture-matched FFI library: ${GANNYU_FFI_LIBRARY}")
endif()

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
    "${GANNYU_FFI_LIBRARY}"
    advapi32
    gdi32
    ntdll
    ole32
    oleaut32
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
