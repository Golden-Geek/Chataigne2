//! Source dialect adapters for the shared fragment compiler.

use super::{CompileError, FragmentFormat, FragmentUniform, TEXTURE_BINDINGS};

mod declarations;
#[cfg(all(windows, feature = "d3d11"))]
mod glsl;
mod isf;
pub(super) mod shadertoy;
#[cfg(all(windows, feature = "d3d11"))]
pub(super) use glsl::{ShaderToyBuffers, fragment_source, fragment_source_with_buffers};

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

#[cfg(all(windows, feature = "d3d11"))]
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
