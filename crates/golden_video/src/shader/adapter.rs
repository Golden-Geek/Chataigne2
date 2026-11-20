//! Source dialect adapters for the shared fragment compiler.

use std::{borrow::Cow, fmt::Write};

use super::stages::Stage;
use super::{CompileError, FragmentFormat, FragmentUniform, TEXTURE_BINDINGS, TEXTURE_INPUTS, uniforms};

mod declarations;
mod isf;
pub(super) mod shadertoy;

pub(super) const TEXTURE_NAMES: [&str; TEXTURE_BINDINGS] = [
    "goldenMedia0",
    "goldenMedia1",
    "goldenMedia2",
    "goldenMedia3",
    "goldenBuffer0",
    "goldenBuffer1",
    "goldenBuffer2",
    "goldenBuffer3",
];

// Texture loads keep filtering in Shader Model 5.0 without a sampler descriptor heap.
const SAMPLE_TEXTURE: &str = r#"
vec4 goldenStraightColor(vec4 rgba) {
    return rgba.a > 0.0 ? vec4(rgba.rgb / rgba.a, rgba.a) : vec4(0.0);
}
vec4 goldenSample(texture2D image, bool raw, vec2 uv) {
    ivec2 extent = textureSize(image, 0);
    if (any(lessThanEqual(extent, ivec2(0)))) return vec4(0.0);
    vec2 p = clamp(goldenTextureCoordinates(uv), vec2(0.0), vec2(1.0)) * vec2(extent) - 0.5;
    ivec2 base = ivec2(floor(p));
    ivec2 lo = clamp(base, ivec2(0), extent - 1);
    ivec2 hi = clamp(base + 1, ivec2(0), extent - 1);
    vec2 weight = fract(p);
    vec4 upper = mix(texelFetch(image, lo, 0), texelFetch(image, ivec2(hi.x, lo.y), 0), weight.x);
    vec4 lower = mix(texelFetch(image, ivec2(lo.x, hi.y), 0), texelFetch(image, hi, 0), weight.x);
    vec4 rgba = mix(upper, lower, weight.y);
    return raw ? rgba : goldenStraightColor(rgba);
}
vec4 goldenSample(texture2D image, vec2 uv) {
    return goldenSample(image, false, uv);
}
#define texture(image, uv) goldenSample(image, uv)
#define texture2D(image, uv) goldenSample(image, uv)
"#;

const FRAME: &str = r#"
layout(std140, binding = 0) uniform GoldenFrame {
    vec4 sizeAndTime;
    vec4 timing;
    ivec4 indices;
} goldenFrame;
layout(location = 0) out vec4 goldenOutput;
"#;

const BOTTOM_LEFT: &str = r#"
vec4 goldenFragmentCoordinates() {
    return vec4(gl_FragCoord.x, goldenFrame.sizeAndTime.y - gl_FragCoord.y, gl_FragCoord.zw);
}
#define gl_FragCoord goldenFragmentCoordinates()
"#;

const GLSL: &str = r#"
#define iResolution (goldenFrame.sizeAndTime.xyz)
#define iTime (goldenFrame.sizeAndTime.w)
#define fragColor goldenOutput
#define gl_FragColor goldenOutput
#define main goldenUserMain
"#;

const SHADERTOY: &str = r#"
#define iResolution (goldenFrame.sizeAndTime.xyz)
#define iTime (goldenFrame.sizeAndTime.w)
#define iTimeDelta (goldenFrame.timing.x)
#define iFrameRate (goldenFrame.timing.y)
#define iFrame (goldenFrame.indices.x)
"#;

const FRAGCOORD: &str = r#"
#define iResolution (goldenFrame.sizeAndTime.xyz)
#define iTime (goldenFrame.sizeAndTime.w)
#define u_resolution (goldenFrame.sizeAndTime.xy)
#define u_time (goldenFrame.sizeAndTime.w)
#define u_time_delta (goldenFrame.timing.x)
#define u_refresh_rate (goldenFrame.timing.y)
#define u_frame (goldenFrame.indices.x)
#define u_mouse vec4(0.0, 0.0, -1.0, -1.0)
#define u_drag vec2(0.0)
#define u_scroll 0.0
#define u_color_gamut 0
#define fragColor goldenOutput
#define main goldenUserMain
"#;

const CHANNEL_RESOLUTION: &str = r#"
vec3 iChannelResolution[4];
vec3 goldenChannelResolution(texture2D source) {
    ivec2 extent = textureSize(source, 0);
    return vec3(vec2(extent), all(greaterThan(extent, ivec2(0))) ? 1.0 : 0.0);
}
vec3 goldenChannelResolution(texture2D source, bool raw) {
    return goldenChannelResolution(source);
}
"#;

const SET_CHANNEL_RESOLUTION: &str = r#"
    iChannelResolution[0] = goldenChannelResolution(iChannel0);
    iChannelResolution[1] = goldenChannelResolution(iChannel1);
    iChannelResolution[2] = goldenChannelResolution(iChannel2);
    iChannelResolution[3] = goldenChannelResolution(iChannel3);
"#;
const ISF: &str = r#"
#define RENDERSIZE (goldenFrame.sizeAndTime.xy)
#define TIME (goldenFrame.sizeAndTime.w)
#define TIMEDELTA (goldenFrame.timing.x)
#define FRAMEINDEX (goldenFrame.indices.x)
#define PASSINDEX (goldenFrame.indices.y)
#define isf_FragNormCoord (gl_FragCoord.xy / RENDERSIZE)
#define vv_FragNormCoord isf_FragNormCoord
#define gl_FragColor goldenOutput
#define main goldenUserMain
"#;

const ISF_IMAGES: &str = r#"
#define IMG_SIZE(image) vec2(textureSize(image, 0))
#define IMG_NORM_PIXEL(image, uv) goldenSample(image, uv)
#define IMG_PIXEL(image, pixel) goldenSample(image, (pixel) / IMG_SIZE(image))
#define IMG_THIS_NORM_PIXEL(image) goldenSample(image, isf_FragNormCoord)
#define IMG_THIS_PIXEL(image) IMG_PIXEL(image, gl_FragCoord.xy)
"#;

const SHADERTOY_TEXELS: &str = r#"
ivec2 goldenTextureSize(texture2D image, bool raw, int level) {
    return textureSize(image, level);
}
ivec2 goldenTextureSize(texture2D image, int level) {
    return textureSize(image, level);
}
vec4 goldenTexelFetch(texture2D image, bool raw, ivec2 pixel, int level) {
    ivec2 size = textureSize(image, level);
    vec4 rgba = texelFetch(image, ivec2(pixel.x, size.y - 1 - pixel.y), level);
    return raw ? rgba : goldenStraightColor(rgba);
}
vec4 goldenTexelFetch(texture2D image, ivec2 pixel, int level) {
    return goldenTexelFetch(image, false, pixel, level);
}
#define textureSize(image, level) goldenTextureSize(image, level)
#define texelFetch(image, pixel, level) goldenTexelFetch(image, pixel, level)
"#;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ShaderToyBuffers {
    pub raw_output: bool,
    pub raw_inputs: [bool; TEXTURE_INPUTS],
}

fn textures(format: FragmentFormat, shadertoy: &ShaderToyBuffers) -> String {
    let mut source = String::new();
    let count = if format == FragmentFormat::Isf {
        TEXTURE_BINDINGS
    } else {
        TEXTURE_INPUTS
    };
    for (slot, name) in TEXTURE_NAMES.iter().take(count).enumerate() {
        writeln!(source, "layout(binding = {}) uniform texture2D {name};", slot + 1).unwrap();
        if format != FragmentFormat::Isf {
            let raw = if shadertoy.raw_inputs[slot] { ", true" } else { "" };
            writeln!(source, "#define iChannel{slot} {name}{raw}").unwrap();
        }
    }
    let coords = match format {
        FragmentFormat::Glsl => "uv",
        FragmentFormat::ShaderToy | FragmentFormat::Isf | FragmentFormat::FragCoord => "vec2(uv.x, 1.0 - uv.y)",
    };
    writeln!(source, "vec2 goldenTextureCoordinates(vec2 uv) {{ return {coords}; }}").unwrap();
    source.push_str(SAMPLE_TEXTURE);
    if format == FragmentFormat::Isf {
        source.push_str(ISF_IMAGES);
    } else if matches!(format, FragmentFormat::ShaderToy | FragmentFormat::FragCoord) {
        source.push_str(SHADERTOY_TEXELS);
    }
    source
}

pub(super) struct Adapted {
    pub glsl: String,
    pub stages: Vec<Stage>,
    pub uniforms: Vec<FragmentUniform>,
}

pub(super) fn fragment_source(format: FragmentFormat, source: &str) -> Result<Adapted, CompileError> {
    fragment_source_with_buffers(format, source, ShaderToyBuffers::default())
}

pub(super) fn fragment_source_with_buffers(
    format: FragmentFormat,
    source: &str,
    shadertoy: ShaderToyBuffers,
) -> Result<Adapted, CompileError> {
    let source = source.trim_start_matches('\u{feff}').trim_start();
    let (source, prelude, mut declarations, stages, uniforms, call) = match format {
        FragmentFormat::Glsl => (
            source,
            GLSL,
            String::new(),
            vec![Stage::default()],
            vec![],
            "goldenUserMain();",
        ),
        FragmentFormat::ShaderToy => (
            source,
            SHADERTOY,
            String::new(),
            vec![Stage::default()],
            vec![],
            "mainImage(goldenOutput, gl_FragCoord.xy);",
        ),
        FragmentFormat::FragCoord => (
            source,
            FRAGCOORD,
            String::new(),
            vec![Stage::default()],
            vec![],
            if declarations::uses_main_image(source) {
                "mainImage(goldenOutput, gl_FragCoord.xy);"
            } else {
                "goldenUserMain();"
            },
        ),
        FragmentFormat::Isf => {
            let isf::IsfSource {
                body,
                aliases: declarations,
                stages,
                uniforms,
            } = isf::source(source)?;
            (body, ISF, declarations, stages, uniforms, "goldenUserMain();")
        }
    };
    let coordinates = match format {
        FragmentFormat::Glsl => "",
        FragmentFormat::ShaderToy | FragmentFormat::Isf | FragmentFormat::FragCoord => BOTTOM_LEFT,
    };
    let textures = textures(format, &shadertoy);
    let source = without_version(source);
    let uses_channel_resolution = format == FragmentFormat::ShaderToy
        && source
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .any(|token| token == "iChannelResolution");
    let (channel_resolution, channel_setup) = if uses_channel_resolution {
        (CHANNEL_RESOLUTION, SET_CHANNEL_RESOLUTION)
    } else {
        ("", "")
    };
    let (source, uniforms) = match format {
        FragmentFormat::Isf => (Cow::Borrowed(source), uniforms),
        FragmentFormat::Glsl | FragmentFormat::ShaderToy | FragmentFormat::FragCoord => {
            let declared = declarations::parse(format, source)?;
            (declared.body(source), declared.uniforms)
        }
    };
    let color_output = if shadertoy.raw_output {
        ""
    } else {
        "    goldenOutput.a = clamp(goldenOutput.a, 0.0, 1.0);\n    goldenOutput.rgb *= goldenOutput.a;\n"
    };
    let mut uniform_setup = String::new();
    uniforms::glsl(&uniforms, &mut declarations, &mut uniform_setup);
    let glsl = format!(
        "#version 450 core\n{FRAME}\n{coordinates}\n{prelude}\n{textures}\n{declarations}\n{channel_resolution}\n{source}\n\
         #undef main\nvoid main() {{\n    goldenOutput = vec4(0.0);\n{uniform_setup}{channel_setup}    {call}\n{color_output}}}\n"
    );
    Ok(Adapted { glsl, stages, uniforms })
}

pub(super) fn inspect_uniforms(format: FragmentFormat, source: &str) -> Result<Vec<FragmentUniform>, CompileError> {
    let source = source.trim_start_matches('\u{feff}').trim_start();
    match format {
        FragmentFormat::Isf => Ok(isf::source(source)?.uniforms),
        FragmentFormat::ShaderToy if source.starts_with(['{', '[']) => {
            let fields = shadertoy::parse(source)?
                .into_iter()
                .map(|(code, _)| declarations::parse(format, without_version(&code)).map(|declared| declared.uniforms))
                .collect::<Result<Vec<_>, _>>()?;
            declarations::merge_uniforms(fields.iter().flatten())
        }
        FragmentFormat::Glsl | FragmentFormat::ShaderToy | FragmentFormat::FragCoord => {
            Ok(declarations::parse(format, without_version(source))?.uniforms)
        }
    }
}

pub(super) fn merge_uniforms<'a>(
    fields: impl IntoIterator<Item = &'a FragmentUniform>,
) -> Result<Vec<FragmentUniform>, CompileError> {
    declarations::merge_uniforms(fields)
}

fn without_version(source: &str) -> &str {
    let source = source.trim_start();
    if source.starts_with("#version ") || source.starts_with("#version\t") {
        source.split_once('\n').map_or("", |(_, body)| body)
    } else {
        source
    }
}
