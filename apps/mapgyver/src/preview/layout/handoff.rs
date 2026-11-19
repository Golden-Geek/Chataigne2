use super::PreviewLayout;
use crate::{
    media::{protocol::ChannelReference, resolution::ChannelAvailability},
    preview::protocol::PreviewSource,
};
use golden_video::{
    PassId, TextureDescriptor, TextureId,
    plan::{RenderOperation, RenderPass, RenderPlan},
    preview::PreviewSnapshot,
};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(crate) struct PreviewImage {
    source: PreviewSource,
    texture: TextureId,
    descriptor: TextureDescriptor,
}

pub(crate) type PreviewImages = HashMap<Uuid, PreviewImage>;

pub(crate) struct Handoff<'a> {
    pub(crate) previous: &'a PreviewImages,
    pub(crate) availability: &'a HashMap<ChannelReference, ChannelAvailability>,
    pub(crate) max_passes: usize,
}

impl Handoff<'_> {
    /// Select the image per viewport. An outgoing pass can survive without its producer:
    /// the native backend keeps its written allocation when the replacement is still warming.
    pub(super) fn images(
        self,
        layout: &PreviewSnapshot,
        plan: &mut RenderPlan,
        channels: &HashMap<ChannelReference, TextureId>,
        patterns: &HashMap<PreviewSource, TextureId>,
    ) -> PreviewImages {
        let mut images = PreviewImages::new();
        let mut pass_descriptors: HashMap<_, _> =
            plan.passes.iter().map(|pass| (pass.output, pass.descriptor)).collect();
        let mut current_images: HashSet<_> = patterns.values().copied().collect();
        current_images.extend(channels.iter().filter_map(|(reference, texture)| {
            (self.availability.get(reference) == Some(&ChannelAvailability::Ready)).then_some(*texture)
        }));
        for view in layout.views.iter().filter(|view| view.tile_groups.is_none()) {
            let Ok(source) = serde_json::from_str::<PreviewSource>(&view.source) else {
                continue;
            };
            let (replacement, waiting) = match source {
                PreviewSource::Channel { reference } => match self.availability.get(&reference) {
                    Some(ChannelAvailability::Ready) => (channels.get(&reference), false),
                    Some(ChannelAvailability::Unavailable | ChannelAvailability::Incompatible) => (None, true),
                    _ => (None, false),
                },
                PreviewSource::Calibration { .. } => (patterns.get(&source), false),
                _ => (None, false),
            };
            if let Some(&texture) = replacement {
                images.insert(
                    view.id,
                    PreviewImage {
                        source,
                        texture,
                        descriptor: pass_descriptors[&texture],
                    },
                );
                continue;
            }
            let Some(&previous) = self
                .previous
                .get(&view.id)
                .filter(|old| waiting && old.source != source)
            else {
                continue;
            };
            let texture = previous.texture;
            if let Some(descriptor) = pass_descriptors.get(&texture) {
                if *descriptor != previous.descriptor || !current_images.contains(&texture) {
                    continue;
                }
            } else {
                if plan.passes.len() + PreviewLayout::PASS_COUNT >= self.max_passes {
                    continue;
                }
                plan.passes.push(RenderPass {
                    id: PassId::new(texture.get()).unwrap(),
                    output: texture,
                    descriptor: previous.descriptor,
                    operation: RenderOperation::Retain,
                });
                pass_descriptors.insert(texture, previous.descriptor);
                current_images.insert(texture);
            }
            images.insert(view.id, previous);
        }
        images
    }
}

impl PreviewImage {
    pub(super) fn texture(self) -> TextureId {
        self.texture
    }
}
