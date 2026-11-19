use super::stages::Stage;
use super::{CompileError, FragmentFormat, FragmentInput, FragmentProgram, FragmentStage, MAX_SOURCE_BYTES, adapter};
use crate::TextureDescriptor;

/// A compiled sequence of fragment draws with same-frame and previous-frame texture bindings.
#[derive(Debug)]
pub struct FragmentEffect {
    stages: Box<[(FragmentProgram, Stage)]>,
}

impl FragmentEffect {
    /// Compile an effect off the render thread. ShaderToy JSON can provide separate programs per pass.
    pub fn compile(format: FragmentFormat, source: &str) -> Result<Self, CompileError> {
        let source = source.trim_start_matches('\u{feff}').trim();
        if source.is_empty() || source.len() > MAX_SOURCE_BYTES {
            return Err(CompileError::Size);
        }
        let effect = uuid::Uuid::new_v4();
        let stages = if format == FragmentFormat::ShaderToy && source.starts_with(['{', '[']) {
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
                    let (glsl, _) = adapter::fragment_source_with_buffers(format, &code, buffers)?;
                    Ok((FragmentProgram::compile_source(&glsl, effect, buffers)?, layout))
                })
                .collect::<Result<_, CompileError>>()?
        } else {
            let (glsl, stages) = adapter::fragment_source(format, source)?;
            let program = FragmentProgram::compile_source(&glsl, effect, Default::default())?;
            stages.into_iter().map(|layout| (program.clone(), layout)).collect()
        };
        Ok(Self { stages })
    }

    /// Resolve ordered draws using the requested output dimensions.
    pub fn stage_layout(&self, output: TextureDescriptor) -> Result<Vec<FragmentStage>, CompileError> {
        self.stages
            .iter()
            .map(|(program, layout)| layout.resolve(output, program.clone()))
            .collect()
    }
}
