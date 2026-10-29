use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::core::s;

use super::marker_hlsl;

#[test]
fn fixture_marker_and_card_shaders_compile_without_a_gpu_device() {
    for body in [
        include_str!("../card.hlsl"),
        include_str!("../consumer.hlsl"),
        include_str!("single_source.hlsl"),
    ] {
        compile(body);
    }
}

fn compile(body: &str) {
    let shader = format!("{}\n{}", marker_hlsl(), body);
    for (entry, profile) in [(s!("vs_main"), s!("vs_5_0")), (s!("ps_main"), s!("ps_5_0"))] {
        let mut code = None;
        let mut errors = None;
        // SAFETY: fixed source and initialized owned blob outputs; compilation creates no GPU device.
        let result = unsafe {
            D3DCompile(
                shader.as_ptr().cast(),
                shader.len(),
                s!("fixture-compile-test"),
                None,
                None,
                entry,
                profile,
                0,
                0,
                &mut code,
                Some(&mut errors),
            )
        };
        let message = errors.map(|errors| {
            // SAFETY: the owned compiler blob retains its complete diagnostic byte range.
            let bytes =
                unsafe { std::slice::from_raw_parts(errors.GetBufferPointer().cast::<u8>(), errors.GetBufferSize()) };
            String::from_utf8_lossy(bytes).into_owned()
        });
        assert!(result.is_ok(), "fixture shader compilation failed: {message:?}");
        assert!(code.is_some());
    }
}
