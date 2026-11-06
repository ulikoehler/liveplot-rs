// build.rs — drives the vendored VolcanoPlot C++ build when the
// `volcano` feature is enabled, and links the static FFI library.
//
// The submodule is kept at stock upstream: the FFI shim lives in
// vendor/volcano-ffi and is compiled here via `cc`; the patched
// glyb.cmake in vendor/volcano-patches is copied over the submodule's
// copy before configure (adds vendored zlib/libpng/freetype/harfbuzz
// fallbacks when system libs are absent).
//
// Linux: uses the Vulkan SDK files vendored at vendor/vulkan-sdk
// (extracted from the LunarG tarball) + vendor/Vulkan-Headers.
// Windows: expects the LunarG Vulkan SDK installed with VULKAN_SDK
// set in the environment (headers + vulkan-1.lib + shaderc_combined.lib).
// See docs/volcano-integration.md.

use std::path::{Path, PathBuf};
use std::process::Command;

fn first_existing(dir: &Path, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find(|n| dir.join(format!("{n}.lib")).exists() || dir.join(format!("lib{n}.a")).exists())
        .map(|s| s.to_string())
}

fn main() {
    if std::env::var("CARGO_FEATURE_VOLCANO").is_err() {
        return;
    }

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let volcano = manifest.join("vendor/VolcanoPlot");
    let build_dir = volcano.join("build");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");

    // Vulkan/shaderc locations differ per platform.
    let (vk_include, vk_library, shaderc_lib) = if windows {
        // LunarG SDK on Windows exposes VULKAN_SDK.
        let sdk = std::env::var("VULKAN_SDK")
            .map(PathBuf::from)
            .expect(
                "volcano feature on Windows requires the LunarG Vulkan SDK \
                 (VULKAN_SDK env var). See docs/volcano-integration.md",
            );
        (
            sdk.join("Include"),
            sdk.join("Lib").join("vulkan-1.lib"),
            sdk.join("Lib").join("shaderc_combined.lib"),
        )
    } else {
        // Vendored Linux SDK: vendor/vulkan-sdk/<ver>/x86_64.
        let sdk = manifest.join("vendor/vulkan-sdk");
        let root = std::fs::read_dir(&sdk)
            .ok()
            .and_then(|mut d| d.next())
            .and_then(|e| e.ok())
            .map(|e| e.path().join("x86_64"));
        (
            root.as_ref()
                .map(|p| p.join("include"))
                .unwrap_or_else(|| manifest.join("vendor/Vulkan-Headers/include")),
            PathBuf::from("/usr/lib/x86_64-linux-gnu/libvulkan.so.1"),
            root.map(|p| p.join("lib/libshaderc_combined.a"))
                .unwrap_or_default(),
        )
    };

    // The submodule's `liveplot-ffi` branch carries the glyb patch
    // (vendored zlib/libpng/freetype/harfbuzz) and the ffi component.

    // Configure once (CMakeCache presence = configured).
    if !build_dir.join("CMakeCache.txt").exists() {
        let mut args = vec![
            "-B".to_string(),
            build_dir.to_str().unwrap().to_string(),
            "-S".to_string(),
            volcano.to_str().unwrap().to_string(),
            format!("-DVulkan_INCLUDE_DIR={}", vk_include.display()),
            format!("-DVulkan_LIBRARY={}", vk_library.display()),
            format!("-DSHADERC_INCLUDE_DIR={}", vk_include.display()),
            format!("-DSHADERC_LIBRARY={}", shaderc_lib.display()),
            "-DVOLCANO_BUILD_SCREEN_BACKEND=OFF".to_string(),
            "-DVOLCANO_BUILD_TESTS=OFF".to_string(),
            "-DVOLCANO_BUILD_EXAMPLES=OFF".to_string(),
            "-DVOLCANO_BUILD_CLI=OFF".to_string(),
            "-DVOLCANO_BUILD_PYTHON=OFF".to_string(),
        ];
        if !windows {
            args.push("-DCMAKE_BUILD_TYPE=Release".to_string());
        }
        let status = Command::new("cmake")
            .args(&args)
            .status()
            .expect("cmake configure failed to start");
        assert!(status.success(), "cmake configure failed");
    }

    // Build the static stack (incremental). MSVC is multi-config →
    // artifacts land in build/Release/.
    let mut build = vec![
        "--build".to_string(),
        build_dir.to_str().unwrap().to_string(),
        "-j4".to_string(),
        "--target".to_string(),
        "volcano_ffi_static".to_string(),
    ];
    if windows {
        build.push("--config".to_string());
        build.push("Release".to_string());
    }
    let status = Command::new("cmake")
        .args(&build)
        .status()
        .expect("cmake build failed to start");
    assert!(status.success(), "cmake build failed");

    // Link search paths — MSVC puts libs under <dir>/Release.
    let mut search = vec![
        build_dir.clone(),
        build_dir.join("third_party/freetype2"),
        build_dir.join("third_party/harfbuzz"),
        build_dir.join("third_party/libpng"),
        build_dir.join("third_party/zlib"),
    ];
    if windows {
        search.extend(
            search
                .clone()
                .iter()
                .map(|p| p.join("Release"))
                .collect::<Vec<_>>(),
        );
        if let Some(sdk) = shaderc_lib.parent() {
            println!("cargo:rustc-link-search=native={}", sdk.display());
        }
    }
    for d in &search {
        println!("cargo:rustc-link-search=native={}", d.display());
    }
    if !windows {
        if let Some(lib) = shaderc_lib.parent() {
            println!("cargo:rustc-link-search=native={}", lib.display());
        }
    }

    // Link: left-to-right resolution order. Third-party artifact names
    // differ per generator — pick whichever exists.
    let mut libs = vec![
        "volcano_ffi_static".to_string(),
        "volcano_render_static".to_string(),
        "volcano_backend_static".to_string(),
        "volcano_encode_static".to_string(),
        "volcano_plot_static".to_string(),
        "volcano_text_static".to_string(),
        "glyb_static".to_string(),
        "volcano_core_static".to_string(),
        "harfbuzz".to_string(),
        "freetype".to_string(),
    ];
    let pick = |names: &[&str]| -> Option<String> {
        search.iter().find_map(|d| first_existing(d, names))
    };
    libs.push(
        pick(&["png_static", "libpng16_static", "libpng", "png"]).expect("libpng build artifact"),
    );
    libs.push(pick(&["zlibstatic", "zlib", "z"]).expect("zlib build artifact"));
    // Second pass: single-pass archive resolution can skip members that
    // only become needed later (hb-ft.cc inside glyb is required by
    // font.cc which appears after it in the archive).
    libs.extend([
        "volcano_text_static".to_string(),
        "glyb_static".to_string(),
        "volcano_core_static".to_string(),
        "shaderc_combined".to_string(),
    ]);
    for lib in &libs {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    if windows {
        println!("cargo:rustc-link-lib=dylib=vulkan-1");
    } else {
        // libvulkan.so.1 exists (runtime loader) but no dev symlink —
        // pass the full path as a link arg.
        println!("cargo:rustc-link-arg={}", vk_library.display());
        for lib in ["stdc++", "m", "dl", "pthread"] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
        // bz2/brotli may exist only as runtime .so.1 on machines without
        // dev packages — fall back to full-path link args like vulkan.
        for lib in ["bz2", "brotlidec", "brotlicommon"] {
            let dev = format!("/usr/lib/x86_64-linux-gnu/lib{lib}.so");
            let rt = format!("/usr/lib/x86_64-linux-gnu/lib{lib}.so.1");
            if Path::new(&dev).exists() || !Path::new(&rt).exists() {
                println!("cargo:rustc-link-lib=dylib={lib}");
            } else {
                println!("cargo:rustc-link-arg={rt}");
            }
        }
    }

    println!("cargo:rerun-if-changed=vendor/VolcanoPlot/src/ffi");
    println!("cargo:rerun-if-changed=vendor/VolcanoPlot/include/volcano/ffi");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_VOLCANO");
    println!("cargo:rerun-if-env-changed=VULKAN_SDK");
}
