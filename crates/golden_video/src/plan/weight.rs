use std::mem::size_of;

use super::{BlitRegion, ExternalTexture, Layer, PlanOutput, RenderOperation, RenderPass, RenderPlan};

impl RenderPlan {
    /// Counts actual retained vector capacity, not just active graph values.
    pub(crate) fn retained_bytes(&self) -> Option<usize> {
        let mut canvases = std::collections::HashSet::new();
        let mut images = std::collections::HashSet::new();
        let mut bytes = size_of::<Self>()
            .checked_add(
                self.external_textures
                    .capacity()
                    .checked_mul(size_of::<ExternalTexture>())?,
            )?
            .checked_add(self.passes.capacity().checked_mul(size_of::<RenderPass>())?)?
            .checked_add(self.outputs.capacity().checked_mul(size_of::<PlanOutput>())?)?;
        for pass in &self.passes {
            for image in pass.operation.canvas_images() {
                if images.insert(std::sync::Arc::as_ptr(image) as usize) {
                    bytes = bytes
                        .checked_add(image.retained_bytes())?
                        .checked_add(2 * size_of::<usize>())?;
                }
            }
            match &pass.operation {
                RenderOperation::Composite { layers } => {
                    bytes = bytes.checked_add(layers.capacity().checked_mul(size_of::<Layer>())?)?;
                    for layer in layers {
                        if let super::LayerSource::Pattern(super::Pattern::Canvas(canvas)) = &layer.source
                            && canvases.insert(canvas.storage_identity())
                        {
                            bytes = bytes.checked_add(canvas.retained_bytes())?;
                        }
                    }
                }
                RenderOperation::Regions { regions, visibility } => {
                    bytes = bytes.checked_add(regions.capacity().checked_mul(size_of::<BlitRegion>())?)?;
                    for region in regions {
                        if let Some(placement) = &region.placement {
                            bytes = bytes.checked_add(placement.retained_heap_bytes())?;
                        }
                        if let Some(visibility) = &region.visibility {
                            bytes = bytes.checked_add(visibility.retained_heap_bytes())?;
                        }
                    }
                    if let Some(visibility) = visibility {
                        bytes = bytes.checked_add(visibility.retained_heap_bytes())?;
                    }
                }
                _ => {}
            }
        }
        for texture in &self.external_textures {
            if let Some(source) = &texture.source {
                bytes = bytes.checked_add(source.retained_bytes())?;
            }
        }
        Some(bytes)
    }
}
