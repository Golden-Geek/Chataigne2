use std::mem::size_of;

use super::{ExternalTexture, Layer, PlanOutput, RenderOperation, RenderPass, RenderPlan};

impl RenderPlan {
    /// Counts actual retained vector capacity, not just active graph values.
    pub(crate) fn retained_bytes(&self) -> Option<usize> {
        let mut bytes = size_of::<Self>()
            .checked_add(
                self.external_textures
                    .capacity()
                    .checked_mul(size_of::<ExternalTexture>())?,
            )?
            .checked_add(self.passes.capacity().checked_mul(size_of::<RenderPass>())?)?
            .checked_add(self.outputs.capacity().checked_mul(size_of::<PlanOutput>())?)?;
        for pass in &self.passes {
            if let RenderOperation::Composite { layers } = &pass.operation {
                bytes = bytes.checked_add(layers.capacity().checked_mul(size_of::<Layer>())?)?;
            }
        }
        for texture in &self.external_textures {
            if let Some(source) = &texture.source {
                bytes = bytes.checked_add(source.sender.capacity())?;
            }
        }
        Some(bytes)
    }
}
