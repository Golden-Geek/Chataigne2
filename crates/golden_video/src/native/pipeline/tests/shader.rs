use golden_time::{PlaybackRate, ShowPosition, SystemClock, TimeAuthority};

use super::*;
use crate::native::tests::pixels::readback;
use crate::shader::FragmentProgram;

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_shader_uses_target_coordinates_time_and_premultiplied_alpha() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    let gpu = Device::new(adapter.id).unwrap();
    let pipeline = Pipeline::new(&gpu.device).unwrap();
    let target = Texture::new(
        &gpu.device,
        TextureDescriptor {
            extent: Extent2D::new(2, 2).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        },
    )
    .unwrap();
    let program = FragmentProgram::compile_glsl(
        r#"void main() {
            if (gl_FragCoord.y > iResolution.y / 2.0) discard;
            fragColor = vec4(step(1.0, iTime), step(1.0, gl_FragCoord.x), 1.0, 0.5);
        }"#,
    )
    .unwrap();
    let shader = ShaderDraw::new(&gpu.device, &program).unwrap();
    let mut time = TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::PAUSED).unwrap();
    pipeline.draw_shader(&gpu.context, &target, &shader, time.sample().unwrap());
    assert_eq!(
        readback(&gpu, &target),
        [[0, 0, 128, 128], [0, 128, 128, 128], [0; 4], [0; 4]]
    );

    let later = time
        .discontinuity(ShowPosition::from_nanos(2_000_000_000), PlaybackRate::PAUSED)
        .unwrap();
    pipeline.draw_shader(&gpu.context, &target, &shader, later);
    assert_eq!(
        readback(&gpu, &target),
        [[128, 0, 128, 128], [128, 128, 128, 128], [0; 4], [0; 4]]
    );
}
