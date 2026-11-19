//! Source dialect adapters for the shared fragment compiler.

use std::fmt::Write;

use super::{CompileError, FragmentFormat, TEXTURE_INPUTS};

mod isf;

pub(super) const TEXTURE_NAMES: [&str; TEXTURE_INPUTS] =
    ["goldenMedia0", "goldenMedia1", "goldenMedia2", "goldenMedia3"];

// Texture loads keep filtering in Shader Model 5.0 without a sampler descriptor heap.
const SAMPLE_TEXTURE: &str = r#"
vec4 goldenSample(texture2D image, vec2 uv) {
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
    return rgba.a > 0.0 ? vec4(rgba.rgb / rgba.a, rgba.a) : vec4(0.0);
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

fn textures(format: FragmentFormat) -> String {
    let mut source = String::new();
    for (slot, name) in TEXTURE_NAMES.iter().enumerate() {
        writeln!(source, "layout(binding = {}) uniform texture2D {name};", slot + 1).unwrap();
        if format != FragmentFormat::Isf {
            writeln!(source, "#define iChannel{slot} {name}").unwrap();
        }
    }
    let coords = match format {
        FragmentFormat::Glsl => "uv",
        FragmentFormat::ShaderToy | FragmentFormat::Isf => "vec2(uv.x, 1.0 - uv.y)",
    };
    writeln!(source, "vec2 goldenTextureCoordinates(vec2 uv) {{ return {coords}; }}").unwrap();
    source.push_str(SAMPLE_TEXTURE);
    if format == FragmentFormat::Isf {
        source.push_str(ISF_IMAGES);
    }
    source
}

pub(super) fn fragment_source(format: FragmentFormat, source: &str) -> Result<String, CompileError> {
    let source = source.trim_start_matches('\u{feff}').trim_start();
    let (source, prelude, declarations, call) = match format {
        FragmentFormat::Glsl => (source, GLSL, String::new(), "goldenUserMain();"),
        FragmentFormat::ShaderToy => (
            source,
            SHADERTOY,
            String::new(),
            "mainImage(goldenOutput, gl_FragCoord.xy);",
        ),
        FragmentFormat::Isf => {
            let (body, declarations) = isf::single_pass(source)?;
            (body, ISF, declarations, "goldenUserMain();")
        }
    };
    let coordinates = match format {
        FragmentFormat::Glsl => "",
        FragmentFormat::ShaderToy | FragmentFormat::Isf => BOTTOM_LEFT,
    };
    let textures = textures(format);
    let source = source.trim_start();
    let source = if source.starts_with("#version ") {
        source.split_once('\n').map_or("", |(_, body)| body)
    } else {
        source
    };
    Ok(format!(
        "#version 450 core\n{FRAME}\n{coordinates}\n{prelude}\n{textures}\n{declarations}\n{source}\n\
         #undef main\nvoid main() {{\n    goldenOutput = vec4(0.0);\n    {call}\n\
         goldenOutput.a = clamp(goldenOutput.a, 0.0, 1.0);\n    goldenOutput.rgb *= goldenOutput.a;\n}}\n"
    ))
}
