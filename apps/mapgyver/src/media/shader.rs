pub(super) mod uniforms;

use golden_video::shader::FragmentFormat;

pub(super) const FILE_FORMATS: &[(&str, FragmentFormat)] = &[
    (".glsl", FragmentFormat::Glsl),
    (".frag", FragmentFormat::Glsl),
    (".fs", FragmentFormat::Isf),
    (".isf", FragmentFormat::Isf),
    (".shadertoy.json", FragmentFormat::ShaderToy),
];

pub(super) fn file_format(name: &str) -> Option<(&str, FragmentFormat)> {
    let lower = name.to_ascii_lowercase();
    FILE_FORMATS.iter().find_map(|&(suffix, format)| {
        let stem = lower.strip_suffix(suffix)?;
        Some((&name[..stem.len()], format))
    })
}

pub(super) const DEFAULT_CODE: &str = "void main() {\n    vec2 uv = gl_FragCoord.xy / iResolution.xy;\n    fragColor = vec4(uv, 0.5 + 0.5 * sin(iTime), 1.0);\n}\n";

pub(super) fn validate_code(code: &str) -> Result<(), String> {
    if code.len() > golden_video::shader::MAX_SOURCE_BYTES || code.contains('\0') {
        return Err("Shader code must be at most 128 KiB of UTF-8 without NUL".into());
    }
    Ok(())
}
