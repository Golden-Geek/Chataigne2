use super::{CompileError, FragmentProgram, TEXTURE_BINDINGS, TEXTURE_INPUTS};
use crate::{AlphaMode, Extent2D, PixelFormat, TextureDescriptor};

mod dimension;
pub(super) use dimension::{Dimension, ScalarInput};

/// One ordered draw in a fragment effect resolved for the requested output size.
#[derive(Clone, Debug)]
pub struct FragmentStage {
    /// Validated program for this draw.
    pub program: FragmentProgram,
    /// Extent and precision required by this draw.
    pub target: TextureDescriptor,
    /// Media, current or delayed source for each texture register.
    pub inputs: [Option<FragmentInput>; TEXTURE_BINDINGS],
}

/// A texture source within a compiled effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragmentInput {
    /// Index of an authored Media input on this effect.
    Media(u8),
    /// Index of an earlier stage in the current frame.
    EarlierPass(u8),
    /// Index of the buffer's last writer in the previous frame.
    PreviousFrame(u8),
}

#[derive(Debug)]
pub(super) struct Stage {
    pub(super) size: [Option<Dimension>; 2],
    pub(super) float: bool,
    pub(super) inputs: [Option<FragmentInput>; TEXTURE_BINDINGS],
}

impl Default for Stage {
    fn default() -> Self {
        Self {
            size: [None, None],
            float: false,
            inputs: std::array::from_fn(|index| (index < TEXTURE_INPUTS).then_some(FragmentInput::Media(index as u8))),
        }
    }
}

impl Stage {
    pub(super) fn resolve(
        &self,
        output: TextureDescriptor,
        program: FragmentProgram,
    ) -> Result<FragmentStage, CompileError> {
        let axis = |index: usize, fallback| {
            self.size[index]
                .as_ref()
                .map_or(Ok(fallback), |value| value.evaluate(output.extent, &program))
        };
        let width = axis(0, output.extent.width())?;
        let height = axis(1, output.extent.height())?;
        let used = program.sampled_bindings();
        let alpha_mode = if program.raw_output() {
            AlphaMode::Straight
        } else {
            output.alpha_mode
        };
        Ok(FragmentStage {
            program,
            target: TextureDescriptor {
                extent: Extent2D::new(width, height).expect("validated pass dimensions"),
                format: if self.float {
                    PixelFormat::Rgba32Float
                } else {
                    output.format
                },
                alpha_mode,
                ..output
            },
            inputs: std::array::from_fn(|slot| self.inputs[slot].filter(|_| used[slot])),
        })
    }
}
