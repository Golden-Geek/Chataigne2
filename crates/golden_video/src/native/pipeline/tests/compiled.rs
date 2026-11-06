use super::super::{SHADER, blob, shaders};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::core::s;

#[test]
fn embedded_programs_match_the_current_shader_source_byte_for_byte() {
    for (entry, profile, embedded) in [
        (s!("vs_main"), s!("vs_5_0"), shaders::VERTEX),
        (s!("ps_main"), s!("ps_5_0"), shaders::PIXEL),
        (s!("ps_mapped"), s!("ps_5_0"), shaders::MAPPED_PIXEL),
        (s!("ps_pattern"), s!("ps_5_0"), shaders::PATTERN_PIXEL),
        (s!("ps_overlay"), s!("ps_5_0"), shaders::OVERLAY_PIXEL),
        (s!("ps_import"), s!("ps_5_0"), shaders::IMPORT_PIXEL),
    ] {
        let mut code = None;
        let mut diagnostics = None;
        // SAFETY: immutable source and initialized outputs, with the former runtime's exact
        // source name, profiles, flags and no includes/macros. No adapter/device is created.
        unsafe {
            D3DCompile(
                SHADER.as_ptr().cast(),
                SHADER.len(),
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
        }
        .unwrap();
        let code = code.unwrap();
        assert_eq!(
            embedded,
            unsafe { blob(&code) },
            "build-time program differs from prior runtime compilation"
        );
    }
}
