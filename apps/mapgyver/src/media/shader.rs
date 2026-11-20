pub(super) mod uniforms;

pub(super) const DEFAULT_CODE: &str = "void main() {\n    vec2 uv = gl_FragCoord.xy / iResolution.xy;\n    fragColor = vec4(uv, 0.5 + 0.5 * sin(iTime), 1.0);\n}\n";

pub(super) fn validate_code(code: &str) -> Result<(), String> {
    if code.len() > golden_video::shader::MAX_SOURCE_BYTES || code.contains('\0') {
        return Err("Shader code must be at most 128 KiB of UTF-8 without NUL".into());
    }
    Ok(())
}
