//! The app resolves opaque preview source keys; Golden retains geometry and visibility fences.
use super::protocol::PreviewSource;
use crate::media::protocol::ChannelReference;
use crate::screens::protocol::ScreenId;
use golden_video::plan::{BlendMode, Layer, PlanOutput, RenderOperation, RenderPass, SampleFilter};
use golden_video::preview::PreviewSnapshot;
use golden_video::runtime::PlanSubmission;
use golden_video::{Extent2D, LinearColor, Opacity, OutputId, PassId, TextureDescriptor, TextureId};
use std::collections::HashMap;

#[derive(Clone)]
pub struct PreviewLayout {
    pub output: OutputId,
    pub descriptor: TextureDescriptor,
    pub layout: PreviewSnapshot,
}

impl PreviewLayout {
    pub(crate) const RESERVED_TEXTURES: u64 = 5;
    pub(crate) const PASS_COUNT: usize = 5;

    pub(crate) fn maximum_dependencies(&self) -> usize {
        self.layout.maximum_dependencies()
    }

    pub fn sources(&self) -> Result<Vec<PreviewSource>, String> {
        self.layout
            .source_keys()
            .map(|source| serde_json::from_str(source).map_err(|error| format!("invalid app preview source: {error}")))
            .collect()
    }
    pub fn references(&self) -> Result<Vec<ChannelReference>, String> {
        Ok(self
            .sources()?
            .into_iter()
            .filter_map(|source| match source {
                PreviewSource::Channel { reference } => Some(reference),
                PreviewSource::Screen { .. } => None,
            })
            .collect())
    }

    pub(crate) fn append(
        &self,
        submission: &mut PlanSubmission,
        channels: &HashMap<ChannelReference, TextureId>,
        screens: &HashMap<ScreenId, TextureId>,
    ) -> Result<(), String> {
        self.references()?;
        let texture = TextureId::new(1).unwrap();
        submission.plan.passes.push(RenderPass {
            id: PassId::new(1).unwrap(),
            output: texture,
            descriptor: self.descriptor,
            operation: RenderOperation::Regions {
                visibility: None,
                regions: self.layout.regions(self.output, |source| {
                    match serde_json::from_str::<PreviewSource>(source).ok()? {
                        PreviewSource::Channel { reference } => channels.get(&reference).copied(),
                        PreviewSource::Screen { screen } => screens.get(&screen).copied(),
                    }
                }),
            },
        });
        let background = TextureId::new(2).unwrap();
        let output = TextureId::new(3).unwrap();
        submission.plan.passes.push(RenderPass {
            id: PassId::new(2).unwrap(),
            output: background,
            descriptor: TextureDescriptor {
                extent: Extent2D::new(1, 1).unwrap(),
                ..self.descriptor
            },
            operation: RenderOperation::Clear(LinearColor::from_straight(0.0075, 0.0075, 0.0075, 1.).unwrap()),
        });
        let black = TextureId::new(4).unwrap();
        let active_areas = TextureId::new(5).unwrap();
        submission.plan.passes.push(RenderPass {
            id: PassId::new(4).unwrap(),
            output: black,
            descriptor: TextureDescriptor {
                extent: Extent2D::new(1, 1).unwrap(),
                ..self.descriptor
            },
            operation: RenderOperation::Clear(LinearColor::from_straight(0., 0., 0., 1.).unwrap()),
        });
        submission.plan.passes.push(RenderPass {
            id: PassId::new(5).unwrap(),
            output: active_areas,
            descriptor: self.descriptor,
            // Every registered aperture is black, even when its source is unavailable. Keep
            // this below the source regions so their alpha composites over black, not the theme.
            operation: RenderOperation::Regions {
                visibility: None,
                regions: self
                    .layout
                    .backdrops(black)
                    .into_iter()
                    .chain(self.layout.regions(self.output, |_| Some(black)))
                    .collect(),
            },
        });
        submission.plan.passes.push(RenderPass {
            id: PassId::new(3).unwrap(),
            output,
            descriptor: self.descriptor,
            operation: RenderOperation::Composite {
                layers: [background, active_areas, texture]
                    .into_iter()
                    .map(|source| Layer {
                        source: source.into(),
                        opacity: Opacity::OPAQUE,
                        opacity_envelope: None,
                        blend: BlendMode::Normal,
                        filter: SampleFilter::Linear,
                        mapping: None,
                    })
                    .collect(),
            },
        });
        submission.plan.outputs.push(PlanOutput {
            id: self.output,
            source: output,
        });
        Ok(())
    }
}
