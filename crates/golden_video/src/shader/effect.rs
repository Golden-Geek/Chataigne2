use std::sync::Arc;

use super::stages::Stage;
use super::{
    CompileError, FragmentFormat, FragmentInput, FragmentProgram, FragmentStage, FragmentUniform, MAX_SOURCE_BYTES,
    UniformValue, adapter,
};
use crate::TextureDescriptor;

/// A compiled sequence of fragment draws with same-frame and previous-frame texture bindings.
#[derive(Debug)]
pub struct FragmentEffect {
    stages: Box<[(FragmentProgram, Stage)]>,
    uniforms: Arc<[FragmentUniform]>,
}

impl FragmentEffect {
    /// Compile an effect off the render thread. ShaderToy JSON can provide separate programs per pass.
    pub fn compile(format: FragmentFormat, source: &str) -> Result<Self, CompileError> {
        let source = source.trim_start_matches('\u{feff}').trim();
        if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
            return Err(CompileError::Size);
        }
        let effect = uuid::Uuid::new_v4();
        let stages: Box<[_]> = if format == FragmentFormat::ShaderToy && source.starts_with(['{', '[']) {
            adapter::shadertoy::parse(source)?
                .into_iter()
                .map(|(code, layout)| {
                    let buffers = adapter::ShaderToyBuffers {
                        raw_output: layout.float,
                        raw_inputs: std::array::from_fn(|index| {
                            matches!(
                                layout.inputs[index],
                                Some(FragmentInput::EarlierPass(_) | FragmentInput::PreviousFrame(_))
                            )
                        }),
                    };
                    let adapted = adapter::fragment_source_with_buffers(format, &code, buffers)?;
                    Ok((
                        FragmentProgram::compile_source(&adapted.glsl, effect, buffers, adapted.uniforms)?,
                        layout,
                    ))
                })
                .collect::<Result<_, CompileError>>()?
        } else {
            let adapted = adapter::fragment_source(format, source)?;
            let program = FragmentProgram::compile_source(&adapted.glsl, effect, Default::default(), adapted.uniforms)?;
            adapted
                .stages
                .into_iter()
                .map(|layout| (program.clone(), layout))
                .collect()
        };
        let uniforms = if format == FragmentFormat::ShaderToy && stages.len() > 1 {
            adapter::merge_uniforms(stages.iter().flat_map(|(program, _)| program.uniforms()))?.into()
        } else {
            stages[0].0.uniform_layout()
        };
        Ok(Self { stages, uniforms })
    }

    /// Resolve ordered draws using the requested output dimensions.
    pub fn stage_layout(&self, output: TextureDescriptor) -> Result<Vec<FragmentStage>, CompileError> {
        self.stages
            .iter()
            .map(|(program, layout)| layout.resolve(output, program.clone()))
            .collect()
    }

    /// Validated metadata from the same source revision as the compiled draws.
    pub fn uniform_layout(&self) -> Arc<[FragmentUniform]> {
        self.uniforms.clone()
    }

    /// Resolve draws and pass dimensions using live authored values.
    pub fn stage_layout_with_uniforms(
        &self,
        output: TextureDescriptor,
        mut read: impl FnMut(&FragmentUniform) -> Option<UniformValue>,
    ) -> Result<Vec<FragmentStage>, CompileError> {
        self.stages
            .iter()
            .map(|(program, layout)| layout.resolve(output, program.with_uniform_values(&mut read)))
            .collect()
    }

    /// Inspect source input metadata without a device or native shader compilation.
    pub fn inspect_uniforms(format: FragmentFormat, source: &str) -> Result<Vec<FragmentUniform>, CompileError> {
        if source.trim().is_empty() || source.len() > MAX_SOURCE_BYTES {
            return Err(CompileError::Size);
        }
        adapter::inspect_uniforms(format, source)
    }
}
