cmake_minimum_required(VERSION 3.22)
project(gannyu_boost_regex LANGUAGES CXX)

set(GANNYU_BOOST_ROOT "" CACHE PATH "Root of the pinned Boost source tree")
if(NOT EXISTS "${GANNYU_BOOST_ROOT}/boost/regex.hpp")
  message(FATAL_ERROR "GANNYU_BOOST_ROOT must contain boost/regex.hpp")
endif()

add_library(gannyu_boost_regex STATIC
  "${GANNYU_BOOST_ROOT}/libs/regex/src/posix_api.cpp"
  "${GANNYU_BOOST_ROOT}/libs/regex/src/regex.cpp"
  "${GANNYU_BOOST_ROOT}/libs/regex/src/regex_debug.cpp"
  "${GANNYU_BOOST_ROOT}/libs/regex/src/static_mutex.cpp"
  "${GANNYU_BOOST_ROOT}/libs/regex/src/wide_posix_api.cpp")
set_target_properties(gannyu_boost_regex PROPERTIES
  OUTPUT_NAME boost_regex
  POSITION_INDEPENDENT_CODE ON)
target_include_directories(gannyu_boost_regex PUBLIC "${GANNYU_BOOST_ROOT}")

install(TARGETS gannyu_boost_regex ARCHIVE DESTINATION lib)
