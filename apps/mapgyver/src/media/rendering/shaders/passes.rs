use std::collections::HashMap;

use golden_video::plan::{RenderOperation, RenderPass, SampleFilter, ShaderTexture};
use golden_video::shader::{FragmentEffect, FragmentInput, FragmentStage, TEXTURE_INPUTS};
use golden_video::{AlphaMode, ColorEncoding, PassId, PixelFormat, TextureDescriptor, TextureId};

use crate::media::capture::CapturedChannels;
use crate::media::protocol::{ChannelReference, ChannelSource};
use crate::media::resolution::{self, ChannelAvailability, SourceObservation};

use super::super::{Entry, plan};
use super::Shaders;

#[cfg(test)]
mod tests;

pub(in crate::media::rendering) struct Passes {
    stages: Vec<FragmentStage>,
    resolve_output: bool,
}

pub(in crate::media::rendering) fn prepare(
    shaders: Option<&Shaders>,
    capture: &CapturedChannels,
    dimension_limit: u32,
) -> HashMap<ChannelReference, Result<Passes, ChannelAvailability>> {
    let Some(shaders) = shaders else {
        return HashMap::new();
    };
    capture
        .channels
        .iter()
        .filter(|channel| channel.enabled)
        .filter_map(|channel| {
            let definition = channel.definition.as_ref()?;
            let ChannelSource::Shader { format, input, .. } = &definition.source else {
                return None;
            };
            let prepared = match shaders.program(*format, input) {
                Ok(None) => return None,
                Err(problem) => Err(problem),
                Ok(Some(program)) => resolution::resolve(definition, SourceObservation::Unavailable)
                    .ok()
                    .and_then(|value| value.extent)
                    .ok_or(ChannelAvailability::Incompatible)
                    .and_then(|extent| {
                        Passes::new(
                            program,
                            TextureDescriptor {
                                extent,
                                format: PixelFormat::Rgba8Unorm,
                                color_encoding: ColorEncoding::Linear,
                                alpha_mode: AlphaMode::Premultiplied,
                            },
                            dimension_limit,
                            &channel.shader_uniforms,
                        )
                    }),
            };
            Some((channel.reference, prepared))
        })
        .collect()
}

impl Passes {
    fn new(
        effect: &FragmentEffect,
        output: TextureDescriptor,
        dimension_limit: u32,
        values: &crate::media::shader::uniforms::Values,
    ) -> Result<Self, ChannelAvailability> {
        let stages = effect
            .stage_layout_with_uniforms(output, |uniform| {
                values
                    .get(&uniform.name)
                    .and_then(crate::media::shader::uniforms::value)
            })
            .map_err(|_| ChannelAvailability::Incompatible)?;
        if stages.iter().any(|stage| {
            stage.target.extent.width() > dimension_limit || stage.target.extent.height() > dimension_limit
        }) {
            return Err(ChannelAvailability::Incompatible);
        }
        let resolve_output = stages.last().expect("effect has an output stage").target != output;
        Ok(Self { stages, resolve_output })
    }

    fn buffer_count(&self) -> usize {
        self.stages.len() - usize::from(!self.resolve_output)
    }

    pub(in crate::media::rendering) fn work(
        &self,
        references: &[Option<ChannelReference>; TEXTURE_INPUTS],
    ) -> (usize, usize) {
        let dependencies = self
            .stages
            .iter()
            .map(|stage| {
                1 + stage
                    .inputs
                    .iter()
                    .flatten()
                    .filter(|binding| match binding {
                        FragmentInput::Media(slot) => references[usize::from(*slot)].is_some(),
                        _ => true,
                    })
                    .count()
            })
            .sum::<usize>();
        // Derived admission already charges the authored Media edges once.
        let extra_dependencies =
            (dependencies + usize::from(self.resolve_output)).saturating_sub(references.iter().flatten().count());
        (self.buffer_count(), extra_dependencies)
    }

    pub(in crate::media::rendering) fn append(
        &self,
        passes: &mut Vec<RenderPass>,
        entry: &mut Entry,
        media: [Option<TextureId>; TEXTURE_INPUTS],
        next: &mut u64,
    ) -> Result<RenderOperation, String> {
        let temporary = &mut entry.shader_buffers;
        temporary.truncate(self.buffer_count());
        while temporary.len() < self.buffer_count() {
            temporary.push(plan::allocate(next)?);
        }
        let clock = PassId::new(entry.target.get()).expect("nonzero channel target");
        let channel_target = entry.target;
        let target = |stage: u8| temporary.get(usize::from(stage)).copied().unwrap_or(channel_target);
        let operation = |index: usize, stage: &FragmentStage| RenderOperation::Shader {
            program: stage.program.clone(),
            inputs: stage.inputs.map(|input| match input? {
                FragmentInput::Media(slot) => media[usize::from(slot)].map(ShaderTexture::Current),
                FragmentInput::EarlierPass(writer) => Some(ShaderTexture::Current(target(writer))),
                FragmentInput::PreviousFrame(writer) => Some(ShaderTexture::PreviousFrame(target(writer))),
            }),
            clock,
            index: index as u8,
        };
        let (last, earlier) = self.stages.split_last().expect("effect has an output stage");
        for (index, stage) in earlier.iter().enumerate() {
            let output = temporary[index];
            passes.push(RenderPass {
                id: PassId::new(output.get()).expect("nonzero temporary target"),
                output,
                descriptor: stage.target,
                operation: operation(index, stage),
            });
        }
        let final_draw = operation(earlier.len(), last);
        if !self.resolve_output {
            return Ok(final_draw);
        }
        let output = temporary[earlier.len()];
        passes.push(RenderPass {
            id: PassId::new(output.get()).expect("nonzero temporary target"),
            output,
            descriptor: last.target,
            operation: final_draw,
        });
        Ok(RenderOperation::Sample {
            source: output,
            filter: SampleFilter::Linear,
        })
    }
}
