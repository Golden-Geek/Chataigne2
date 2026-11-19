use super::{CompileError, TEMPORARY_BUFFERS};
use crate::{Extent2D, PixelFormat, TextureDescriptor};

mod dimension;
pub(super) use dimension::Dimension;

/// One ordered draw in a fragment effect resolved for the requested output size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FragmentStage {
    /// Extent and precision required by this draw.
    pub target: TextureDescriptor,
    /// Current or delayed source for each declared buffer register.
    pub buffers: [Option<FragmentBuffer>; TEMPORARY_BUFFERS],
}

/// An effect-owned buffer is read from the latest applicable draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FragmentBuffer {
    /// Index of an earlier stage in the current frame.
    EarlierPass(u8),
    /// Index of the buffer's last writer in the previous frame.
    PreviousFrame(u8),
}

#[derive(Debug, Default)]
pub(super) struct Stage {
    pub(super) size: [Option<Dimension>; 2],
    pub(super) float: bool,
    pub(super) buffers: [Option<FragmentBuffer>; TEMPORARY_BUFFERS],
}

impl Stage {
    pub(super) fn resolve(&self, output: TextureDescriptor) -> Result<FragmentStage, CompileError> {
        let axis = |index: usize, fallback| {
            self.size[index]
                .as_ref()
                .map_or(Ok(fallback), |value| value.evaluate(output.extent))
        };
        let width = axis(0, output.extent.width())?;
        let height = axis(1, output.extent.height())?;
        Ok(FragmentStage {
            target: TextureDescriptor {
                extent: Extent2D::new(width, height).expect("validated pass dimensions"),
                format: if self.float {
                    PixelFormat::Rgba32Float
                } else {
                    output.format
                },
                ..output
            },
            buffers: self.buffers,
        })
    }

    pub(super) fn retained_bytes(&self) -> usize {
        size_of::<Self>() + self.size.iter().flatten().map(Dimension::retained_bytes).sum::<usize>()
    }
}
