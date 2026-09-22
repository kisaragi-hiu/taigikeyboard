set(TAIGI_SOURCES
    engine.cpp
)

add_fcitx5_addon(taigikeyboard ${TAIGI_SOURCES})
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
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/taigikeyboard.conf"
        DESTINATION "${FCITX_INSTALL_PKGDATADIR}/inputmethod" COMPONENT config)
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/taigikeyboard-addon.conf" RENAME taigikeyboard.conf
        DESTINATION "${FCITX_INSTALL_PKGDATADIR}/addon" COMPONENT config)
