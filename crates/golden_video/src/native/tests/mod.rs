use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::core::s;

#[cfg(feature = "qualification")]
pub(in crate::native) mod completion_gate;
mod contracts;
pub(in crate::native) mod initialization_gate;
pub(in crate::native) mod pixels;

#[test]
fn embedded_native_shader_compiles_all_real_entrypoints() {
    let source = include_str!("../shader.hlsl");
    for (entry, profile) in [
        (s!("vs_main"), s!("vs_5_0")),
        (s!("ps_main"), s!("ps_5_0")),
        (s!("ps_mapped"), s!("ps_5_0")),
        (s!("ps_pattern"), s!("ps_5_0")),
    ] {
        let mut bytecode = None;
        let mut diagnostics = None;
        // SAFETY: fixed source bytes, no includes/macros and initialized owned output slots.
        let result = unsafe {
            D3DCompile(
                source.as_ptr().cast(),
                source.len(),
                s!("golden-video-native-test"),
                None,
                None,
                entry,
                profile,
                0,
                0,
                &mut bytecode,
                Some(&mut diagnostics),
            )
        };
        assert!(
            result.is_ok(),
            "embedded shader compilation failed: {result:?}: {}",
            diagnostics.as_ref().map(blob_text).unwrap_or_default()
        );
        let bytecode = bytecode.expect("compiler returned no shader bytecode");
        // SAFETY: successful owned compiled blob.
        assert!(unsafe { bytecode.GetBufferSize() } > 0);
    }
}

fn blob_text(blob: &ID3DBlob) -> String {
    // SAFETY: blob retains its complete initialized diagnostics through this borrow.
    let bytes = unsafe { std::slice::from_raw_parts(blob.GetBufferPointer().cast::<u8>(), blob.GetBufferSize()) };
    String::from_utf8_lossy(bytes).trim_end_matches('\0').to_owned()
}
