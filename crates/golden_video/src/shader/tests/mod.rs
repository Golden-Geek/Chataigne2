use super::{
    CompileError, FragmentEffect, FragmentFormat, FragmentInput, FragmentProgram, MAX_SOURCE_BYTES, TEXTURE_INPUTS,
};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

#[test]
fn translates_glsl_with_pixel_coordinates_and_frame_input_to_d3d11() {
    let program = FragmentProgram::compile_glsl(
        "#version 450 core\nvoid main() { fragColor = vec4(gl_FragCoord.xy / iResolution.xy, sin(iTime), 0.5); }",
    )
    .unwrap();
    assert!(program.bytecode().starts_with(b"DXBC"));
}

#[test]
fn rejects_unsupported_isf_passes_and_resources_explicitly() {
    for (header, diagnostic) in [
        (r#"{"PASSES":[{"PERSISTENT":true}]}"#, "requires a TARGET"),
        (r#"{"PASSES":[{},{}]}"#, "requires a TARGET"),
        (
            r#"{"PASSES":[{"TARGET":"buffer","WIDTH":"$WIDTH/foo"},{}]}"#,
            "dimension",
        ),
        (r#"{"IMPORTED":{"mask":{"PATH":"mask.png"}}}"#, "imported images"),
        (r#"{"INPUTS":[{"NAME":"levels","TYPE":"audioFFT"}]}"#, "texture binding"),
        (
            r#"{"INPUTS":[{"NAME":"tint","TYPE":"color","DEFAULT":[1,0]}]}"#,
            "Invalid default",
        ),
        (
            r#"{"INPUTS":[{"NAME":"bad-name","TYPE":"float"}]}"#,
            "Invalid or duplicate",
        ),
    ] {
        let source = format!("/*{header}*/\nvoid main() {{ gl_FragColor = vec4(1.0); }}");
        let error = FragmentEffect::compile(FragmentFormat::Isf, &source).unwrap_err();
        assert!(error.to_string().contains(diagnostic), "{error}");
    }
}

#[test]
fn isf_layout_resolves_dimensions_precision_and_latest_earlier_writer() {
    let program = FragmentEffect::compile(
        FragmentFormat::Isf,
        r#"/*{
        "INPUTS":[{"NAME":"divisor","TYPE":"float","DEFAULT":2.0}],
        "PASSES":[
            {"TARGET":"buffer","WIDTH":"($WIDTH + 8) / $divisor","HEIGHT":"$HEIGHT * 0.25","FLOAT":1},
            {"TARGET":"buffer","WIDTH":12,"HEIGHT":"1e+1"},
            {}
        ]
    }*/
    void main() {
        gl_FragColor = PASSINDEX == 0 ? vec4(2.0, 0.0, 1.0, 1.0) : IMG_THIS_NORM_PIXEL(buffer);
    }"#,
    )
    .unwrap();
    let target = TextureDescriptor {
        extent: Extent2D::new(88, 80).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let passes = program.stage_layout(target).unwrap();
    assert_eq!(passes.len(), 3);
    assert_eq!(passes[0].target.extent, Extent2D::new(48, 20).unwrap());
    assert_eq!(passes[0].target.format, PixelFormat::Rgba32Float);
    assert_eq!(passes[0].inputs[TEXTURE_INPUTS..], [None; 4]);
    assert_eq!(passes[1].target.extent, Extent2D::new(12, 10).unwrap());
    assert_eq!(
        passes[1].inputs[TEXTURE_INPUTS..],
        [Some(FragmentInput::EarlierPass(0)), None, None, None]
    );
    assert_eq!(passes[2].target, target);
    assert_eq!(
        passes[2].inputs[TEXTURE_INPUTS..],
        [Some(FragmentInput::EarlierPass(1)), None, None, None]
    );
    assert!(passes[2].program.sampled_bindings()[TEXTURE_INPUTS]);
}

#[test]
fn isf_persistent_reads_the_last_writer_before_the_first_write_of_each_frame() {
    let program = FragmentEffect::compile(
        FragmentFormat::Isf,
        r#"/*{
            "PASSES":[{"TARGET":"state","PERSISTENT":true}, {"TARGET":"state"}, {}]
        }*/
        void main() { gl_FragColor = IMG_THIS_NORM_PIXEL(state); }"#,
    )
    .unwrap();
    let stages = program
        .stage_layout(TextureDescriptor {
            extent: Extent2D::new(16, 16).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        })
        .unwrap();
    assert_eq!(stages[0].inputs[TEXTURE_INPUTS], Some(FragmentInput::PreviousFrame(1)));
    assert_eq!(stages[1].inputs[TEXTURE_INPUTS], Some(FragmentInput::EarlierPass(0)));
    assert_eq!(stages[2].inputs[TEXTURE_INPUTS], Some(FragmentInput::EarlierPass(1)));
}

#[test]
fn shadertoy_json_compiles_common_and_binds_each_pass_in_buffer_order() {
    let effect = FragmentEffect::compile(
        FragmentFormat::ShaderToy,
        r#"{"Shader":{"renderpass":[
        {"type":"image","code":"void mainImage(out vec4 c, in vec2 p) { c = texture(iChannel1,p/iResolution.xy); }",
            "inputs":[{"ctype":"buffer","channel":1,"id":"258"}]},
        {"type":"buffer","name":"Buffer B","outputs":[{"id":258,"channel":0}],
            "code":"void mainImage(out vec4 c,in vec2 p) { c = texture(iChannel2,p/iChannelResolution[2].xy); }",
            "inputs":[{"ctype":"buffer","channel":2,"id":257}]},
        {"type":"common","code":"vec4 previous(vec2 uv) { return texture(iChannel0,uv)+texture(iChannel1,uv); }"},
        {"type":"buffer","name":"Buffer A","outputs":[{"id":257,"channel":0}],
            "code":"void mainImage(out vec4 c, in vec2 p) { c = previous(p/iResolution.xy); }",
            "inputs":[{"ctype":"buffer","channel":0,"id":257},{"ctype":"buffer","channel":1,"id":258}]}
    ]}}"#,
    )
    .unwrap();
    let output = TextureDescriptor {
        extent: Extent2D::new(8, 4).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    };
    let stages = effect.stage_layout(output).unwrap();
    assert_eq!(stages.len(), 3);
    assert_eq!(stages[0].target.format, PixelFormat::Rgba32Float);
    assert_eq!(stages[1].target.format, PixelFormat::Rgba32Float);
    assert_eq!(stages[2].target, output);
    assert_eq!(stages[0].inputs[0], Some(FragmentInput::PreviousFrame(0)));
    assert_eq!(stages[0].inputs[1], Some(FragmentInput::PreviousFrame(1)));
    assert_eq!(stages[1].inputs[2], Some(FragmentInput::EarlierPass(0)));
    assert_eq!(stages[2].inputs[1], Some(FragmentInput::EarlierPass(1)));
    assert_ne!(stages[0].program.bytecode(), stages[1].program.bytecode());
    assert!(
        stages
            .iter()
            .all(|stage| stage.program.effect() == stages[0].program.effect())
    );
}

#[test]
fn shadertoy_json_rejects_unknown_buffer_ids_and_duplicate_channels() {
    let shader = |inputs| {
        format!(
            r#"{{"renderpass":[
        {{"type":"image","code":"void mainImage(out vec4 c,in vec2 p) {{ c=vec4(1.0); }}","inputs":{inputs}}}
    ]}}"#
        )
    };
    for (inputs, expected) in [
        (r#"[{"ctype":"buffer","channel":0,"id":257}]"#, "missing Buffer"),
        (
            r#"[{"ctype":"texture","channel":0},{"ctype":"texture","channel":0}]"#,
            "duplicate",
        ),
        (r#"[{"ctype":"cubemap","channel":3}]"#, "Unsupported ShaderToy input"),
    ] {
        assert!(
            FragmentEffect::compile(FragmentFormat::ShaderToy, &shader(inputs))
                .unwrap_err()
                .to_string()
                .contains(expected)
        );
    }
}
#[test]
fn compiles_independent_texture_slots_in_each_dialect() {
    for (format, source, expected) in [
        (
            FragmentFormat::Glsl,
            "void main() { fragColor = texture(iChannel1, vec2(0.5)); }",
            [false, true, false, false],
        ),
        (
            FragmentFormat::ShaderToy,
            "void mainImage(out vec4 color, in vec2 p) { color = texture(iChannel3, p / iResolution.xy); }",
            [false, false, false, true],
        ),
        (
            FragmentFormat::Isf,
            r#"/*{"INPUTS":[{"NAME":"source","TYPE":"image"},{"NAME":"overlay","TYPE":"image"}]}*/
            void main() { gl_FragColor = IMG_THIS_NORM_PIXEL(overlay); }"#,
            [false, true, false, false],
        ),
    ] {
        let program = FragmentProgram::compile(format, source).unwrap();
        assert!(program.bytecode().starts_with(b"DXBC"));
        assert_eq!(program.sampled_inputs(), expected, "{format:?}");
    }
}

#[test]
fn rejects_unbound_resources_invalid_source_and_oversize_source() {
    for source in [
        "void main() { fragColor = missing; }",
        "layout(binding = 1) uniform User { vec4 color; } user; void main() { fragColor = user.color; }",
        "layout(location = 0) in vec4 varyingColor; void main() { fragColor = varyingColor; }",
        "layout(location = 1) out vec4 extra; void main() { fragColor = extra = vec4(1.0); }",
    ] {
        assert!(matches!(
            FragmentProgram::compile_glsl(source),
            Err(CompileError::Invalid(_))
        ));
    }
    assert!(matches!(
        FragmentProgram::compile_glsl(&"x".repeat(MAX_SOURCE_BYTES + 1)),
        Err(CompileError::Size)
    ));
}
