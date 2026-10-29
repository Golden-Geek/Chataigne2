//! Build script for `spout2-sys`.
//!
//! Compiles the vendored Spout2 SDK (the SpoutGL core sources, plus the SpoutDX
//! class when the `dx` feature is enabled) together with our flat C++ shim
//! (`shim/spout_shim.cpp`) into a single static library `spout2`, and emits the
//! Windows system-library link directives.
//!
//! ## CRT linkage (important)
//!
//! Spout's own CMake forces the *static* CRT (`/MT`) via `SPOUT_BUILD_CMT`.
//! Rust on MSVC links the *dynamic* CRT (`/MD`) by default, and the `cc` crate
//! already matches Rust's CRT selection automatically. We therefore must NOT
//! pass `/MT` or define `SPOUT_BUILD_CMT` — doing so would mix CRTs and cause
//! duplicate-symbol link errors (LNK2005 / LNK4098). If a user opts into
//! `-C target-feature=+crt-static`, `cc` switches to `/MT` to match, and that is
//! correct with no special handling here.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=shim/spout_shim.cpp");
    println!("cargo:rerun-if-changed=shim/spout_shim.h");
    // Local SDK fixes must invalidate the static archive even when the shim is unchanged.
    println!("cargo:rerun-if-changed=vendor/Spout2/SPOUTSDK");

    // The native build is Windows + MSVC only. Skip it entirely on other
    // platforms and on docs.rs so `cargo check` / `cargo doc` succeed without a
    // C++ toolchain. The Rust source is `#[cfg(windows)]`-gated to match.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" || std::env::var_os("DOCS_RS").is_some() {
        return;
    }

    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_env != "msvc" {
        panic!(
            "spout2-sys requires the MSVC toolchain (a *-pc-windows-msvc target); \
             found target_env = {target_env:?}. The vendored Spout2 C++ uses \
             MSVC-only facilities (strncpy_s, #pragma comment(lib, ...), <direct.h>)."
        );
    }

    // Windows on ARM is not supported: the vendored Spout SIMD sources
    // (e.g. SpoutCopy.cpp) use x86 SSE2 intrinsics, which upstream only adapts to
    // ARM via the `sse2neon` shim (the SPOUT_BUILD_ARM CMake path). This build
    // does not wire that up, so fail fast with a clear message rather than a
    // confusing C++ compile error.
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_arch == "aarch64" {
        panic!(
            "spout2-sys does not support Windows on ARM (aarch64): the vendored \
             Spout SSE2 SIMD sources require the upstream `sse2neon` shim, which \
             this build does not configure. Use an x86_64 (x64) target."
        );
    }

    let dx = std::env::var_os("CARGO_FEATURE_DX").is_some();
    let dx12 = std::env::var_os("CARGO_FEATURE_DX12").is_some();
    let gl = std::env::var_os("CARGO_FEATURE_GL").is_some();

    let sdk = PathBuf::from("vendor/Spout2/SPOUTSDK");
    let gl_dir = sdk.join("SpoutGL");
    let dx_dir = sdk.join("SpoutDirectX/SpoutDX");
    let dx12_dir = dx_dir.join("SpoutDX12");

    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .warnings(false)
        .flag_if_supported("/EHsc") // C++ exception handling model
        .include(&gl_dir)
        .include(&dx_dir) // for SpoutDX.h (resolved via __has_include in the DX shim)
        .include("shim");

    // The 11 core SpoutGL sources. Both backends depend on these; compile once.
    for f in [
        "Spout.cpp",
        "SpoutCopy.cpp",
        "SpoutDirectX.cpp",
        "SpoutFrameCount.cpp",
        "SpoutGL.cpp",
        "SpoutGLextensions.cpp",
        "SpoutReceiver.cpp",
        "SpoutSender.cpp",
        "SpoutSenderNames.cpp",
        "SpoutSharedMemory.cpp",
        "SpoutUtils.cpp",
    ] {
        build.file(gl_dir.join(f));
    }

    // The DirectX 11 backend adds the `spoutDX` class. The DX12 backend inherits
    // from spoutDX and also requires SpoutDX.cpp.
    if dx || dx12 {
        build.file(dx_dir.join("SpoutDX.cpp"));
    }
    if dx {
        build.define("SPOUT2_SHIM_DX", None);
    }
    if dx12 {
        build.file(dx12_dir.join("SpoutDX12.cpp"));
        build.include(&dx12_dir);
        build.define("SPOUT2_SHIM_DX12", None);
    }
    if gl {
        build.define("SPOUT2_SHIM_GL", None);
    }

    build.file("shim/spout_shim.cpp");
    build.compile("spout2"); // emits cargo:rustc-link-lib=static=spout2

    // System libraries Spout links against on Windows. Some are also pulled in
    // by `#pragma comment(lib, ...)` in the C++ sources; listing them is
    // harmless and covers the ones added only via CMake target_link_libraries.
    for lib in [
        "opengl32", "gdi32", "user32", "kernel32", "advapi32", "shell32", "ole32", "oleaut32",
        "uuid", "version", "winmm", "comdlg32", "comctl32", "d3d9", "d3d11", "d3d12", "dxgi",
        "psapi",
    ] {
        println!("cargo:rustc-link-lib=dylib={lib}");
    }
}
