use std::rc::Rc;

use golden_time::{PlaybackRate, ShowPosition, SystemClock, TimeAuthority};

use super::*;
use crate::native::tests::pixels::readback;
use crate::shader::{FragmentFormat, FragmentProgram, TEXTURE_BINDINGS};

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
    let shader = ShaderPass::new(
        Rc::new(ShaderDraw::new(&gpu.device, &program).unwrap()),
        Default::default(),
    );
    let mut time = TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::PAUSED).unwrap();
    pipeline.draw_shader(
        &gpu.context,
        &target,
        &shader,
        [None; TEXTURE_BINDINGS],
        0,
        time.sample().unwrap(),
    );
    assert_eq!(
        readback(&gpu, &target),
        [[0, 0, 128, 128], [0, 128, 128, 128], [0; 4], [0; 4]]
    );

    let later = time
        .discontinuity(ShowPosition::from_nanos(2_000_000_000), PlaybackRate::PAUSED)
        .unwrap();
    pipeline.draw_shader(&gpu.context, &target, &shader, [None; TEXTURE_BINDINGS], 0, later);
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
        let primary = ShaderPass::new(draw.clone(), Default::default());
        let secondary = ShaderPass::new(draw, Default::default());
        let mut time =
            TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::PAUSED).unwrap();
        let first = time.sample().unwrap();
        let initial = [[255, 0, 0, 255], [255, 0, 0, 255], [0, 0, 0, 255], [0, 0, 0, 255]];
        pipeline.draw_shader(&gpu.context, &target, &primary, [None; TEXTURE_BINDINGS], 0, first);
        assert_eq!(readback(&gpu, &target), initial, "{format:?}");
        let next = time.sample().unwrap();
        let advanced = [
            [255, 255, 255, 255],
            [255, 255, 255, 255],
            [0, 255, 255, 255],
            [0, 255, 255, 255],
        ];
        for _ in 0..2 {
            pipeline.draw_shader(&gpu.context, &target, &primary, [None; TEXTURE_BINDINGS], 0, next);
            assert_eq!(readback(&gpu, &target), advanced, "{format:?}");
        }
        pipeline.draw_shader(&gpu.context, &target, &secondary, [None; TEXTURE_BINDINGS], 0, next);
        assert_eq!(readback(&gpu, &target), initial, "{format:?}");
    }
}

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_shader_inputs_preserve_orientation_and_alpha_and_clear_when_unbound() {
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    let gpu = Device::new(adapter.id).unwrap();
    let pipeline = Pipeline::new(&gpu.device).unwrap();
    let descriptor = TextureDescriptor {
        extent: Extent2D::new(2, 2).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let source = Texture::new(&gpu.device, descriptor).unwrap();
    let target = Texture::new(&gpu.device, descriptor).unwrap();
    let compile = |format, code: &str| {
        let program = FragmentProgram::compile(format, code).unwrap();
        ShaderPass::new(
            Rc::new(ShaderDraw::new(&gpu.device, &program).unwrap()),
            Default::default(),
        )
    };
    let producer = compile(
        FragmentFormat::Glsl,
        r#"void main() {
        fragColor = gl_FragCoord.y < 1.0 ? vec4(1.0, 0.0, 0.0, 0.5) : vec4(0.0, 1.0, 0.0, 1.0);
    }"#,
    );
    let time = TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::PAUSED)
        .unwrap()
        .sample()
        .unwrap();
    let empty = [None; TEXTURE_BINDINGS];
    pipeline.draw_shader(&gpu.context, &source, &producer, empty, 0, time);
    let expected = [[128, 0, 0, 128], [128, 0, 0, 128], [0, 255, 0, 255], [0, 255, 0, 255]];
    for (format, code, slot) in [
        (
            FragmentFormat::Glsl,
            "void main() { fragColor = texture(iChannel2, gl_FragCoord.xy / iResolution.xy); }",
            2,
        ),
        (
            FragmentFormat::ShaderToy,
            "void mainImage(out vec4 color, in vec2 p) { color = texture(iChannel3, p / iResolution.xy); }",
            3,
        ),
        (
            FragmentFormat::Isf,
            r#"/*{"INPUTS":[{"NAME":"image","TYPE":"image"}]}*/
            void main() { gl_FragColor = IMG_THIS_PIXEL(image); }"#,
            0,
        ),
    ] {
        let consumer = compile(format, code);
        let mut inputs = empty;
        inputs[slot] = Some(&source);
        pipeline.draw_shader(&gpu.context, &target, &consumer, inputs, 0, time);
        assert_eq!(readback(&gpu, &target), expected, "{format:?}");
        pipeline.draw_shader(&gpu.context, &target, &consumer, empty, 0, time);
        assert_eq!(readback(&gpu, &target), [[0; 4]; 4], "{format:?}");
    }
}
