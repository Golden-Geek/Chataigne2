use std::rc::Rc;

use golden_time::{MonotonicTime, PlaybackRate, ShowPosition, SystemClock, TimeAuthority};

use super::super::history::History;
use super::super::presentation::Presentation;
use crate::native::device::{Device, adapters};
use crate::native::outputs::OutputDemand;
use crate::native::pipeline::{Pipeline, ShaderClock, ShaderDraw, ShaderPass};
use crate::native::tests::pixels::readback;
use crate::native::texture::Texture;
use crate::plan::{PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan, ShaderTexture};
use crate::shader::{FragmentEffect, FragmentFormat, FragmentProgram, TEXTURE_BINDINGS};
use crate::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId,
};

#[test]
fn previous_only_writer_is_scheduled_even_when_it_follows_the_reader() {
    let first = TextureId::new(1).unwrap();
    let second = TextureId::new(2).unwrap();
    let program = FragmentProgram::compile_glsl("void main() { fragColor = texture(iChannel0, vec2(0.5)); }").unwrap();
    let pass = |output: TextureId, binding, index| {
        let mut inputs = [None; TEXTURE_BINDINGS];
        inputs[0] = Some(binding);
        RenderPass {
            id: PassId::new(output.get()).unwrap(),
            output,
            descriptor: TextureDescriptor {
                extent: Extent2D::new(2, 2).unwrap(),
                format: PixelFormat::Rgba8Unorm,
                color_encoding: ColorEncoding::Linear,
                alpha_mode: AlphaMode::Premultiplied,
            },
            operation: RenderOperation::Shader {
                program: program.clone(),
                inputs,
                index,
                clock: PassId::new(1).unwrap(),
            },
        }
    };
    let plan = RenderPlan {
        passes: vec![
            pass(first, ShaderTexture::PreviousFrame(second), 0),
            pass(second, ShaderTexture::Current(first), 1),
        ],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: first,
        }],
        ..Default::default()
    }
    .compile(&PlanLimits {
        max_passes: 2,
        max_dependencies: 4,
        max_external_textures: 0,
        max_outputs: 1,
        max_texture_dimension: 2,
        max_texture_bytes: 48,
    })
    .unwrap();
    let mut presentation = Presentation::new(&plan, None);
    presentation.begin(false, |_| OutputDemand::Draw);
    assert!(presentation.pass(0) && presentation.pass(1));
    presentation.begin(false, |_| OutputDemand::None);
    assert!(!presentation.pass(0) && !presentation.pass(1));
}

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_shader_history_uses_a_transparent_first_frame_and_one_snapshot_per_time() {
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
    let program = FragmentProgram::compile_glsl("void main() { fragColor = vec4(1.0); }").unwrap();
    let create = || Rc::new(Texture::new(&gpu.device, descriptor).unwrap());
    let history = History::new(create(), create(), program);
    let sample = |frame| history.begin(&gpu.context, MonotonicTime::from_nanos(frame));
    let paint = |r, g, b| {
        pipeline.clear(
            &gpu.context,
            &history.current,
            LinearColor::from_straight(r, g, b, 1.0).unwrap(),
        );
        history.written();
    };
    sample(1);
    assert_eq!(readback(&gpu, &history.previous), [[0; 4]; 4]);
    paint(1.0, 0.0, 0.0);
    sample(1);
    assert_eq!(readback(&gpu, &history.previous), [[0; 4]; 4]);
    sample(2);
    assert_eq!(readback(&gpu, &history.previous), [[255, 0, 0, 255]; 4]);
    paint(0.0, 1.0, 0.0);
    sample(2);
    assert_eq!(readback(&gpu, &history.previous), [[255, 0, 0, 255]; 4]);
    sample(3);
    assert_eq!(readback(&gpu, &history.previous), [[0, 255, 0, 255]; 4]);
}

#[test]
#[ignore = "requires an explicitly available Windows hardware D3D11 adapter"]
fn hardware_shader_history_clears_raw_float_targets_and_preserves_data_with_zero_alpha() {
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
    let effect = FragmentEffect::compile(FragmentFormat::ShaderToy, r#"{"renderpass":[
        {"type":"buffer","name":"Buffer A","outputs":[{"id":257}],
            "inputs":[{"ctype":"buffer","channel":0,"id":257}],
            "code":"void mainImage(out vec4 c,in vec2 p) { c = vec4(2.0, texelFetch(iChannel0, ivec2(0), 0).g + 0.25, 0.0, 0.0); }"},
        {"type":"image","inputs":[{"ctype":"buffer","channel":0,"id":257}],
            "code":"void mainImage(out vec4 c,in vec2 p) { vec4 v=texture(iChannel0,vec2(0.5)); c=vec4(float(v.r==2.0 && v.a==0.0),v.g,0.0,1.0); }"}
    ]}"#).unwrap();
    let passes = effect.stage_layout(descriptor).unwrap();
    let allocate = |descriptor| Rc::new(Texture::new(&gpu.device, descriptor).unwrap());
    let buffer = History::new(
        allocate(passes[0].target),
        allocate(passes[0].target),
        passes[0].program.clone(),
    );
    let output = allocate(descriptor);
    let clock = Rc::new(ShaderClock::default());
    let draw = |index: usize| {
        let program = &passes[index].program;
        ShaderPass::new(
            Rc::new(ShaderDraw::new(&gpu.device, program).unwrap()),
            clock.clone(),
            program,
        )
    };
    let first = draw(0);
    let last = draw(1);
    let mut time = TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::PAUSED).unwrap();
    for green in [64, 128] {
        let frame = time.sample().unwrap();
        buffer.begin(&gpu.context, frame.monotonic());
        let mut inputs = [None; TEXTURE_BINDINGS];
        inputs[0] = Some(buffer.previous.as_ref());
        pipeline.draw_shader(&gpu.context, &buffer.current, &first, inputs, 0, frame);
        buffer.written();
        inputs[0] = Some(buffer.current.as_ref());
        pipeline.draw_shader(&gpu.context, &output, &last, inputs, 1, frame);
        assert_eq!(readback(&gpu, &output), [[255, green, 0, 255]; 4]);
    }
}
