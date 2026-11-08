cmake_minimum_required(VERSION 3.20)
project(fcitx5-gonnyu LANGUAGES CXX)

file(READ "${CMAKE_CURRENT_SOURCE_DIR}/../../../Cargo.toml" WORKSPACE_MANIFEST)
string(REGEX MATCH "workspace\\.package\\][\r\n]+version[ \t]*=[ \t]*\"([0-9]+\\.[0-9]+\\.[0-9]+(-[0-9A-Za-z.-]+)?)\"" VERSION_MATCH "${WORKSPACE_MANIFEST}")
if(NOT CMAKE_MATCH_1)
    message(FATAL_ERROR "Missing workspace package version")
endif()
set(GONNYU_VERSION "${CMAKE_MATCH_1}")
set(GONNYU_FFI_LIBRARY "${CMAKE_CURRENT_SOURCE_DIR}/../../../target/release/libgannyu_input_ffi.a")
set(GONNYU_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../../crates/ffi/include")

set(CMAKE_CXX_STANDARD 20)
find_package(Fcitx5Core REQUIRED)

function(gonnyu_addon addon_id scheme native_name label addon_name)
    set(GONNYU_ADDON_ID "${addon_id}")
    set(GONNYU_ADDON_NAME "${addon_name}")
    set(GONNYU_NATIVE_NAME "${native_name}")
    set(GONNYU_LABEL "${label}")
    configure_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/gonnyu-addon.conf.in"
        "${CMAKE_CURRENT_BINARY_DIR}/${addon_id}-addon.conf"
        @ONLY
    )
    configure_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/gonnyu.conf.in"
        "${CMAKE_CURRENT_BINARY_DIR}/${addon_id}.conf"
        @ONLY
    )
    add_library(fcitx5-${addon_id} MODULE gannyu_fcitx5.cpp)
    target_include_directories(fcitx5-${addon_id} PRIVATE "${GONNYU_INCLUDE_DIR}")
    target_compile_definitions(fcitx5-${addon_id} PRIVATE
        GANNYU_TRANSCRIPTION_SCHEME=${scheme}
        GANNYU_ADDON_ID="${addon_id}"
    )
    set_target_properties(fcitx5-${addon_id} PROPERTIES OUTPUT_NAME "fcitx5-${addon_id}")
    target_link_libraries(fcitx5-${addon_id} PRIVATE
        Fcitx5::Core
        "${GONNYU_FFI_LIBRARY}"
    )
endfunction()

gonnyu_addon("gonnyu" 0 "赣语（一式·汉拼式）" "赣" "GonnyuGeneralIME")
gonnyu_addon("gonnyu_II" 1 "赣语（二式·组合式）" "赣二" "GonnyuGeneralIME (Compositional)")
