use std::rc::Rc;

use golden_time::{PlaybackRate, ShowPosition, SystemClock, TimeAuthority};

use super::*;
use crate::native::tests::pixels::readback;
use crate::shader::{FragmentFormat, FragmentProgram};

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
    let shader = ShaderPass::new(Rc::new(ShaderDraw::new(&gpu.device, &program).unwrap()));
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

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_shadertoy_and_isf_use_bottom_left_coordinates_and_per_pass_frames() {
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
    for (format, code) in [
        (
            FragmentFormat::ShaderToy,
            r#"void mainImage(out vec4 color, in vec2 pixel) {
            color = vec4(float(pixel.y > iResolution.y * 0.5), float(iFrame),
                         float(iTimeDelta > 0.0 && iFrameRate > 0.0), 1.0);
        }"#,
        ),
        (
            FragmentFormat::Isf,
            r#"/*{"ISFVSN":"2","INPUTS":[{"NAME":"threshold","TYPE":"float","DEFAULT":0.5}]}*/
        void main() {
            if (PASSINDEX != 0 || RENDERSIZE.y != 2.0) discard;
            gl_FragColor = vec4(float(isf_FragNormCoord.y > threshold), float(FRAMEINDEX),
                                float(TIMEDELTA > 0.0), 1.0);
        }"#,
        ),
    ] {
        let program = FragmentProgram::compile(format, code).unwrap();
        let draw = Rc::new(ShaderDraw::new(&gpu.device, &program).unwrap());
        let primary = ShaderPass::new(draw.clone());
        let secondary = ShaderPass::new(draw);
        let mut time =
            TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::PAUSED).unwrap();
        let first = time.sample().unwrap();
        let initial = [[255, 0, 0, 255], [255, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255]];
        pipeline.draw_shader(&gpu.context, &target, &primary, first);
        assert_eq!(readback(&gpu, &target), initial, "{format:?}");
        let next = time.sample().unwrap();
        let advanced = [
            [255, 255, 255, 255],
            [255, 255, 255, 255],
            [0, 255, 255, 255],
            [0, 255, 255, 255],
        ];
        for _ in 0..2 {
            pipeline.draw_shader(&gpu.context, &target, &primary, next);
            assert_eq!(readback(&gpu, &target), advanced, "{format:?}");
        }
        pipeline.draw_shader(&gpu.context, &target, &secondary, next);
        assert_eq!(readback(&gpu, &target), initial, "{format:?}");
    }
}
