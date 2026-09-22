# The Fcitx5 addon: a thin C++ shell over the Rust C ABI in
# ../crates/taigi-linux-ffi (roadmap L1, revised 2026-09-23). Shape taken
# from fcitx5-rime's CMake (`add_fcitx5_addon`, the two .conf files under
# ${FCITX_INSTALL_PKGDATADIR}/{inputmethod,addon}).
#
# Build (Linux):
#   cargo build --release -p taigi-linux-ffi            # from ../ (linux/)
#   cmake -B build -S fcitx5 -DCMAKE_BUILD_TYPE=Release \
#         -DTAIGI_FFI_LIB=../target/release/libtaigi_linux_ffi.a
#   cmake --build build && sudo cmake --install build
# `make -C linux build-fcitx5` does exactly this.

cmake_minimum_required(VERSION 3.16)
project(fcitx5-taigikeyboard VERSION 3.6.9 LANGUAGES CXX)

set(REQUIRED_FCITX_VERSION 5.1.0)

find_package(ECM 1.0.0 REQUIRED)
set(CMAKE_MODULE_PATH ${ECM_MODULE_PATH} ${CMAKE_MODULE_PATH})
include(FeatureSummary)
include(GNUInstallDirs)

find_package(Fcitx5Core ${REQUIRED_FCITX_VERSION} REQUIRED)
find_package(Threads REQUIRED)

# The Rust static library and its header. The library is built by cargo
# first (see the header comment); this file never invokes cargo so a
# packager can build the two halves separately.
set(TAIGI_FFI_LIB "${CMAKE_CURRENT_SOURCE_DIR}/../target/release/libtaigi_linux_ffi.a"
    CACHE FILEPATH "libtaigi_linux_ffi.a from `cargo build --release -p taigi-linux-ffi`")
set(TAIGI_FFI_INCLUDE "${CMAKE_CURRENT_SOURCE_DIR}/../crates/taigi-linux-ffi/include"
    CACHE PATH "the directory holding taigikeyboard.h")
if(NOT EXISTS "${TAIGI_FFI_LIB}")
    message(FATAL_ERROR "TAIGI_FFI_LIB not found: ${TAIGI_FFI_LIB} — run `cargo build --release -p taigi-linux-ffi` in linux/ first")
endif()

add_definitions(-DFCITX_GETTEXT_DOMAIN=\"fcitx5-taigikeyboard\")
add_definitions(-DTAIGIKEYBOARD_VERSION=\"${PROJECT_VERSION}\")

include("${FCITX_INSTALL_CMAKECONFIG_DIR}/Fcitx5Utils/Fcitx5CompilerSettings.cmake")

add_subdirectory(src)

feature_summary(WHAT ALL FATAL_ON_MISSING_REQUIRED_PACKAGES)
