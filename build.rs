// build.rs — drives the vendored VolcanoPlot C++ build when the
// `volcano` feature is enabled, and links the static FFI library.
//
// Requires: cmake, a C++23 compiler, and the Vulkan SDK files vendored
// at vendor/vulkan-sdk (extracted from the LunarG SDK tarball) plus
// vendor/Vulkan-Headers. See docs/volcano-integration.md.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    if std::env::var("CARGO_FEATURE_VOLCANO").is_err() {
        return;
    }

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let volcano = manifest.join("vendor/VolcanoPlot");
    let sdk = manifest.join("vendor/vulkan-sdk");
    // Pick the SDK version dir (single entry under x86_64).
    let sdk_lib = std::fs::read_dir(&sdk)
        .ok()
        .and_then(|mut d| d.next())
        .and_then(|e| e.ok())
        .map(|e| e.path().join("x86_64"));
    let vk_include = sdk_lib
        .as_ref()
        .map(|p| p.join("include"))
        .unwrap_or_else(|| manifest.join("vendor/Vulkan-Headers/include"));
    let shaderc_lib = sdk_lib
        .as_ref()
        .map(|p| p.join("lib/libshaderc_combined.a"))
        .unwrap_or_default();
    let build_dir = volcano.join("build");

    // Configure once (CMakeCache presence = configured).
    if !build_dir.join("CMakeCache.txt").exists() {
        let status = Command::new("cmake")
            .args([
                "-B",
                build_dir.to_str().unwrap(),
                "-S",
                volcano.to_str().unwrap(),
                "-DCMAKE_BUILD_TYPE=Release",
                &format!("-DVulkan_INCLUDE_DIR={}", vk_include.display()),
                "-DVulkan_LIBRARY=/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
                &format!("-DSHADERC_INCLUDE_DIR={}", vk_include.display()),
                &format!("-DSHADERC_LIBRARY={}", shaderc_lib.display()),
                "-DVOLCANO_BUILD_SCREEN_BACKEND=OFF",
                "-DVOLCANO_BUILD_TESTS=OFF",
                "-DVOLCANO_BUILD_EXAMPLES=OFF",
                "-DVOLCANO_BUILD_CLI=OFF",
                "-DVOLCANO_BUILD_PYTHON=OFF",
            ])
            .status()
            .expect("cmake configure failed to start");
        assert!(status.success(), "cmake configure failed");
    }

    // Build the FFI component (incremental).
    let status = Command::new("cmake")
        .args([
            "--build",
            build_dir.to_str().unwrap(),
            "-j4",
            "--target",
            "volcano_ffi_static",
        ])
        .status()
        .expect("cmake build failed to start");
    assert!(status.success(), "cmake build failed");

    // Link: left-to-right resolution order.
    println!("cargo:rustc-link-search=native={}", build_dir.display());
    println!(
        "cargo:rustc-link-search=native={}",
        build_dir.join("third_party/freetype2").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        build_dir.join("third_party/harfbuzz").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        build_dir.join("third_party/libpng").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        build_dir.join("third_party/zlib").display()
    );
    if let Some(lib) = &sdk_lib {
        println!("cargo:rustc-link-search=native={}", lib.join("lib").display());
    }
    for lib in [
        "volcano_ffi_static",
        "volcano_render_static",
        "volcano_backend_static",
        "volcano_encode_static",
        "volcano_plot_static",
        "volcano_text_static",
        "glyb_static",
        "volcano_core_static",
        "harfbuzz",
        "freetype",
        "png",
        "z",
        // Second pass: single-pass archive resolution can skip members
        // that only become needed later (e.g. hb-ft.cc inside glyb is
        // required by font.cc which appears after it in the archive).
        "volcano_text_static",
        "glyb_static",
        "volcano_core_static",
        "shaderc_combined",
    ] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    // libvulkan.so.1 exists (runtime loader) but no dev symlink — pass
    // the full path as a link arg.
    println!("cargo:rustc-link-arg=/usr/lib/x86_64-linux-gnu/libvulkan.so.1");
    for lib in ["stdc++", "m", "dl", "pthread", "bz2", "brotlidec", "brotlicommon"] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }

    println!("cargo:rerun-if-changed=vendor/VolcanoPlot/src/ffi");
    println!("cargo:rerun-if-changed=vendor/VolcanoPlot/include/volcano/ffi");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_VOLCANO");
}
