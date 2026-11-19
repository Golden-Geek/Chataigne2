//! Source dialect adapters for the shared fragment compiler.

use super::{CompileError, FragmentFormat};

mod isf;

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
    let source = source.trim_start();
    let source = if source.starts_with("#version ") {
        source.split_once('\n').map_or("", |(_, body)| body)
    } else {
        source
    };
    Ok(format!(
        "#version 450 core\n{FRAME}\n{coordinates}\n{prelude}\n{declarations}\n{source}\n\
         #undef main\nvoid main() {{\n    goldenOutput = vec4(0.0);\n    {call}\n\
         goldenOutput.a = clamp(goldenOutput.a, 0.0, 1.0);\n    goldenOutput.rgb *= goldenOutput.a;\n}}\n"
    ))
}
