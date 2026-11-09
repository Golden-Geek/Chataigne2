//! Fixed native programs are compiler inputs at build time, never per-device runtime work.
fn main() {
    #[cfg(feature = "ffmpeg")]
    compile_hap();
    println!("cargo:rerun-if-changed=src/native/shader.hlsl");
    if std::env::var_os("CARGO_FEATURE_D3D11").is_none()
        || std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
    {
        return;
    }
    compile_native_shaders();
}

#[cfg(feature = "ffmpeg")]
fn compile_hap() {
    let root = "vendor/hap";
    println!("cargo:rerun-if-changed={root}");
    let mut builder = cc::Build::new();
    builder.file(format!("{root}/hap.c")).include(root).std("c11");
    // Keep the dependency's C ABI private to this library; Snappy is implemented by snap.
    for name in ["compress", "uncompress", "max_compressed_length", "uncompressed_length"] {
        builder.define(&format!("snappy_{name}"), format!("golden_hap_snappy_{name}").as_str());
    }
    builder.compile("golden_hap_reference");
    let bindings = bindgen::Builder::default()
        .header(format!("{root}/hap.h"))
        .allowlist_function("Hap.*")
        .allowlist_type("Hap.*")
        .layout_tests(false)
        .generate_comments(false)
        .generate()
        .expect("generate public HAP reference ABI");
    bindings
        .write_to_file(std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("hap_bindings.rs"))
        .expect("write generated HAP ABI");
}

#[cfg(all(windows, feature = "d3d11"))]
fn compile_native_shaders() {
    use std::fmt::Write;
    use std::path::PathBuf;
    use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
    use windows::Win32::Graphics::Direct3D::ID3DBlob;
    use windows::core::s;

    fn bytes(blob: &ID3DBlob) -> &[u8] {
        // SAFETY: the caller retains this compiler-owned blob through its complete borrow.
        unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize()) }
    }

    use font8x8::UnicodeFonts;
    // font8x8's public-domain Basic Latin ROM; no runtime font/device allocation.
    let mut source = String::from("static const uint2 canvas_font[128] = {\n");
    for code in 0..128u8 {
        let rows = font8x8::BASIC_FONTS.get(char::from(code)).unwrap_or([0; 8]);
        writeln!(
            source,
            "uint2({}u, {}u),",
            u32::from_le_bytes(rows[..4].try_into().unwrap()),
            u32::from_le_bytes(rows[4..].try_into().unwrap())
        )
        .unwrap();
    }
    source.push_str("};\n");
    source.push_str(&std::fs::read_to_string("src/native/shader.hlsl").expect("read Golden Video HLSL"));
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    std::fs::write(output.join("native_shader.hlsl"), &source).expect("write combined shader source");
    let mut generated = String::from("// Generated fixed shader bytecode; no native object ownership.\n");
    for (name, entry, profile, file) in [
        ("VERTEX", s!("vs_main"), s!("vs_5_0"), "vs_main.dxbc"),
        ("PIXEL", s!("ps_main"), s!("ps_5_0"), "ps_main.dxbc"),
        ("MAPPED_PIXEL", s!("ps_mapped"), s!("ps_5_0"), "ps_mapped.dxbc"),
        ("PATTERN_PIXEL", s!("ps_pattern"), s!("ps_5_0"), "ps_pattern.dxbc"),
        ("OVERLAY_PIXEL", s!("ps_overlay"), s!("ps_5_0"), "ps_overlay.dxbc"),
        ("IMPORT_PIXEL", s!("ps_import"), s!("ps_5_0"), "ps_import.dxbc"),
        ("VIDEO_PIXEL", s!("ps_video"), s!("ps_5_0"), "ps_video.dxbc"),
        (
            "IMPORT_UINT_PIXEL",
            s!("ps_import_uint"),
            s!("ps_5_0"),
            "ps_import_uint.dxbc",
        ),
        (
            "IMPORT_SINT_PIXEL",
            s!("ps_import_sint"),
            s!("ps_5_0"),
            "ps_import_sint.dxbc",
        ),
    ] {
        let mut code = None;
        let mut diagnostics = None;
        // SAFETY: immutable UTF-8 HLSL, exact retained length, fixed entry/profile and no
        // include handler/macros. Flags/source name match the former runtime compilation.
        let result = unsafe {
            D3DCompile(
                source.as_ptr().cast(),
                source.len(),
                s!("golden-video"),
                None,
                None,
                entry,
                profile,
                0,
                0,
                &mut code,
                Some(&mut diagnostics),
            )
        };
        if let Err(error) = result {
            let message = diagnostics
                .as_ref()
                .map(|blob| String::from_utf8_lossy(bytes(blob)).into_owned())
                .unwrap_or_default();
            panic!("compile {file}: {error}: {message}");
        }
        let code = code.expect("successful compiler must return bytecode");
        std::fs::write(output.join(file), bytes(&code)).expect("write generated shader bytecode");
        writeln!(
            generated,
            "pub(in crate::native) const {name}: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{file}\"));"
        )
        .unwrap();
    }
    std::fs::write(output.join("native_shaders.rs"), generated).expect("write native shader module");
}

#[cfg(not(all(windows, feature = "d3d11")))]
fn compile_native_shaders() {
    panic!("Windows D3D11 shader generation requires a Windows build host; portable plans/null builds do not");
}
