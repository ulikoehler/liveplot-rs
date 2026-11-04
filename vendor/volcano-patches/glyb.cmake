# Component: glyb (third-party text rendering library)
# Builds glyb's core source files as a static library, linking against
# system FreeType and HarfBuzz, and glyb's bundled glm.

set(GLYB_SOURCES
    ${GLYB_PATH}/src/binpack.cc
    ${GLYB_PATH}/src/file.cc
    ${GLYB_PATH}/src/font.cc
    ${GLYB_PATH}/src/glyph.cc
    ${GLYB_PATH}/src/image.cc
    ${GLYB_PATH}/src/logger.cc
    ${GLYB_PATH}/src/utf8.cc
    ${VOLCANO_ROOT}/src/text/glyb_msdf_stub.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/Contour.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/EdgeHolder.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/Scanline.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/Shape.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/SignedDistance.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/Vector2.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/contour-combiners.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/edge-coloring.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/edge-segments.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/edge-selectors.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/equation-solver.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/estimate-sdf-error.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/msdfgen.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/rasterization.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/render-sdf.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/save-bmp.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/save-tiff.cpp
    ${GLYB_PATH}/third_party/msdfgen/core/shape-description.cpp
)

add_library(glyb_static STATIC ${GLYB_SOURCES})
target_compile_features(glyb_static PRIVATE cxx_std_17)

# glyb headers expect to be found via plain #include "glyph.h" etc.
target_include_directories(glyb_static PUBLIC
    ${GLYB_PATH}/src
    ${GLYB_PATH}/third_party/glm
    ${GLYB_PATH}/third_party/msdfgen
)

# libpng: prefer the system package; fall back to the vendored copy
# (with its vendored zlib) when PNG::PNG is unavailable.
if(TARGET PNG::PNG)
    set(GLYB_PNG_LINK PNG::PNG)
else()
    if(NOT TARGET zlibstatic)
        add_subdirectory(${GLYB_PATH}/third_party/zlib
                         ${CMAKE_BINARY_DIR}/third_party/zlib)
    endif()
    set(PNG_SHARED OFF CACHE BOOL "" FORCE)
    set(PNG_STATIC ON CACHE BOOL "" FORCE)
    set(PNG_TESTS OFF CACHE BOOL "" FORCE)
    set(ZLIB_ROOT ${GLYB_PATH}/third_party/zlib)
    add_subdirectory(${GLYB_PATH}/third_party/libpng
                     ${CMAKE_BINARY_DIR}/third_party/libpng)
    target_include_directories(glyb_static PUBLIC
        ${GLYB_PATH}/third_party/libpng
        ${CMAKE_BINARY_DIR}/third_party/libpng
        ${GLYB_PATH}/third_party/zlib
        ${CMAKE_BINARY_DIR}/third_party/zlib
    )
    set(GLYB_PNG_LINK png_static)
endif()

# FreeType / HarfBuzz: prefer system libs; fall back to the vendored
# copies under third_party when unavailable.
if(NOT VOLCANO_FREETYPE_LINK AND EXISTS ${GLYB_PATH}/third_party/freetype2/CMakeLists.txt)
    set(FT_DISABLE_ZLIB ON CACHE BOOL "" FORCE)
    set(FT_DISABLE_BZIP2 ON CACHE BOOL "" FORCE)
    set(FT_DISABLE_PNG ON CACHE BOOL "" FORCE)
    set(FT_DISABLE_HARFBUZZ ON CACHE BOOL "" FORCE)
    set(FT_DISABLE_BROTLI ON CACHE BOOL "" FORCE)
    if(NOT TARGET freetype)
        add_subdirectory(${GLYB_PATH}/third_party/freetype2
                         ${CMAKE_BINARY_DIR}/third_party/freetype2)
    endif()
    target_include_directories(glyb_static PUBLIC
        ${GLYB_PATH}/third_party/freetype2/include
    )
    set(VOLCANO_FREETYPE_LINK freetype)
endif()
if(NOT VOLCANO_HARFBUZZ_LINK AND EXISTS ${GLYB_PATH}/third_party/harfbuzz/CMakeLists.txt)
    set(HB_BUILD_TESTS OFF CACHE BOOL "" FORCE)
    set(HB_BUILD_UTILS OFF CACHE BOOL "" FORCE)
    # glyb's font code calls hb_ft_* — harfbuzz needs freetype interop.
    # Point it at the vendored freetype target from the block above.
    if(NOT TARGET harfbuzz)
        add_subdirectory(${GLYB_PATH}/third_party/harfbuzz
                         ${CMAKE_BINARY_DIR}/third_party/harfbuzz)
    endif()
    target_include_directories(glyb_static PUBLIC
        ${GLYB_PATH}/third_party/harfbuzz/src
        ${CMAKE_BINARY_DIR}/third_party/harfbuzz/src
    )
    set(VOLCANO_HARFBUZZ_LINK harfbuzz)
    # glyb's font code calls hb_ft_*; when a vendored harfbuzz is built
    # it can't find freetype via find_package. Compile hb-ft.cc directly
    # into glyb_static instead — it only needs harfbuzz's src + generated
    # headers (added above) plus freetype headers, which arrive via
    # VOLCANO_FREETYPE_LINK (system or vendored target interface).
    if(TARGET freetype OR VOLCANO_FREETYPE_LINK)
        target_sources(glyb_static PRIVATE
            ${GLYB_PATH}/third_party/harfbuzz/src/hb-ft.cc)
        # hb-ft.cc pulls in harfbuzz internals (hb.hh) which need the
        # same feature-test defines harfbuzz was built with.
        set_source_files_properties(
            ${GLYB_PATH}/third_party/harfbuzz/src/hb-ft.cc
            PROPERTIES COMPILE_DEFINITIONS
            "HAVE_FREETYPE=1;HAVE_ATEXIT;HAVE_GETPAGESIZE;HAVE_INTEL_ATOMIC_PRIMITIVES;HAVE_ISATTY;HAVE_MMAP;HAVE_MPROTECT;HAVE_NEWLOCALE;HAVE_ROUNDF;HAVE_STDBOOL_H;HAVE_STRTOD_L;HAVE_SYSCONF;HAVE_SYS_MMAN_H;HAVE_UNISTD_H")
    endif()
endif()

# System FreeType and HarfBuzz
target_link_libraries(glyb_static PUBLIC
    ${VOLCANO_FREETYPE_LINK}
    ${VOLCANO_HARFBUZZ_LINK}
    ${GLYB_PNG_LINK}
    Threads::Threads
)

# Suppress warnings from third-party code
if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    target_compile_options(glyb_static PRIVATE -w)
endif()

# Alias for consistency
add_library(glyb::glyb ALIAS glyb_static)
