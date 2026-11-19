use super::{CompileError, FragmentProgram, MAX_SOURCE_BYTES};

#[test]
fn translates_glsl_with_pixel_coordinates_and_frame_input_to_d3d11() {
    let program = FragmentProgram::compile_glsl(
        "#version 450 core\nvoid main() { fragColor = vec4(gl_FragCoord.xy / iResolution.xy, sin(iTime), 0.5); }",
    )
    .unwrap();
    assert!(program.bytecode().starts_with(b"DXBC"));
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
