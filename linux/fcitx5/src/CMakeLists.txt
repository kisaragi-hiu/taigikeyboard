set(TAIGI_SOURCES
    engine.cpp
)

# `add_fcitx5_addon` only exists from fcitx5 5.1.12; Ubuntu 24.04 ships
# 5.1.7, so this is the pre-5.1.12 shape (fcitx5-rime 5.1.8 src/CMakeLists.txt).
# The default `lib` prefix stays: the loader resolves `Library=export:libX`
# in the addon .conf to `libX.so` (5.1.7 addonloader.cpp:23-29), so the file
# is `libtaigikeyboard.so`, as rime's is `librime.so`.
add_library(taigikeyboard MODULE ${TAIGI_SOURCES})
target_include_directories(taigikeyboard PRIVATE "${TAIGI_FFI_INCLUDE}")
# The Rust static library, then what its std links on Linux (the ones cargo
# prints with `--print native-static-libs`: dl, pthread, m; gcc_s / c come
# with the C++ toolchain).
target_link_libraries(taigikeyboard
    Fcitx5::Core
    "${TAIGI_FFI_LIB}"
    Threads::Threads
    ${CMAKE_DL_LIBS}
    m
)
install(TARGETS taigikeyboard DESTINATION "${CMAKE_INSTALL_LIBDIR}/fcitx5")

configure_file(taigikeyboard.conf.in taigikeyboard.conf @ONLY)
configure_file(taigikeyboard-addon.conf.in taigikeyboard-addon.conf @ONLY)
# Under the install prefix, not FCITX_INSTALL_PKGDATADIR: that one is fcitx5's
# own /usr/share/fcitx5 whatever CMAKE_INSTALL_PREFIX says, so a staged
# install (DESTDIR-less, another prefix — the CI smoke) would write to /usr.
# The two are the same path for PREFIX=/usr, the packaged case.
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/taigikeyboard.conf"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/inputmethod" COMPONENT config)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/taigikeyboard-addon.conf" RENAME taigikeyboard.conf
        DESTINATION "${CMAKE_INSTALL_DATADIR}/fcitx5/addon" COMPONENT config)
