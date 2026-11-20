//! The app resolves opaque preview source keys; Golden retains geometry and visibility fences.
use super::protocol::PreviewSource;
use crate::media::protocol::ChannelReference;
use crate::screens::protocol::{Calibration, ScreenId};
use golden_video::plan::{BlendMode, Layer, LayerSource, PlanOutput, RenderOperation, RenderPass, SampleFilter};
use golden_video::preview::PreviewSnapshot;
use golden_video::runtime::PlanSubmission;
use golden_video::{Extent2D, LinearColor, Opacity, OutputId, PassId, TextureDescriptor, TextureId};
use std::collections::HashMap;

mod handoff;
pub(crate) use handoff::{Handoff, PreviewImages};

pub(super) fn validate_calibration(mode: Calibration, extent: Extent2D) -> Result<(), String> {
    if mode == Calibration::Off {
        return Err("calibration preview requires an active pattern".into());
    }
    if extent.width() > 8192 || extent.height() > 8192 {
        return Err("calibration preview exceeds the 8192-pixel render limit".into());
    }
    Ok(())
}

#[derive(Clone)]
pub struct PreviewLayout {
    pub output: OutputId,
    pub descriptor: TextureDescriptor,
    pub layout: PreviewSnapshot,
}

#[cfg(all(windows, feature = "native-video"))]
#[derive(Clone, Copy)]
pub(crate) struct PreviewFrameRoute {
    pub output: OutputId,
    pub source: PreviewSource,
}

/// A window composites its measured regions together. Independent preview frames expose the
/// same source textures to consumers that position and clip images in their own viewports.
#[derive(Clone)]
pub(crate) enum PreviewOutputs {
    Window(Box<PreviewLayout>),
    #[cfg(all(windows, feature = "native-video"))]
    Frames {
        routes: Vec<PreviewFrameRoute>,
        descriptor: TextureDescriptor,
    },
}

impl From<PreviewLayout> for PreviewOutputs {
    fn from(layout: PreviewLayout) -> Self {
        Self::Window(Box::new(layout))
    }
}

impl PreviewOutputs {
    pub(crate) fn routing_guard(&self) -> Option<golden_io::RevisionGuard> {
        match self {
            Self::Window(layout) => Some(layout.layout.routing_guard.clone()),
            #[cfg(all(windows, feature = "native-video"))]
            Self::Frames { .. } => None,
        }
    }

    pub(crate) fn register_outputs(&self, outputs: &mut std::collections::HashSet<OutputId>) -> Result<(), String> {
        let unique = match self {
            Self::Window(layout) => outputs.insert(layout.output),
            #[cfg(all(windows, feature = "native-video"))]
            Self::Frames { routes, .. } => routes.iter().all(|route| outputs.insert(route.output)),
        };
        unique
            .then_some(())
            .ok_or_else(|| "preview output duplicates an output route".into())
    }

    pub(crate) fn references(&self) -> Result<Vec<ChannelReference>, String> {
        match self {
            Self::Window(layout) => layout.references(),
            #[cfg(all(windows, feature = "native-video"))]
            Self::Frames { routes, .. } => Ok(routes.iter().filter_map(|route| route.source.reference()).collect()),
        }
    }

    pub(crate) fn count(&self) -> usize {
        match self {
            Self::Window(_) => 1,
            #[cfg(all(windows, feature = "native-video"))]
            Self::Frames { routes, .. } => routes.len(),
        }
    }

    pub(crate) fn pass_count(&self) -> Result<usize, String> {
        match self {
            Self::Window(layout) => layout.pass_count(),
            #[cfg(all(windows, feature = "native-video"))]
            Self::Frames { routes, .. } => Ok(calibrations(routes.iter().map(|route| route.source))?.len()),
        }
    }

    pub(crate) fn maximum_dependencies(&self) -> Result<usize, String> {
        match self {
            Self::Window(layout) => layout.maximum_dependencies(),
            #[cfg(all(windows, feature = "native-video"))]
            Self::Frames { routes, .. } => Ok(calibration_work(calibrations(routes.iter().map(|route| route.source))?)),
        }
    }

    pub(crate) fn append(
        &self,
        submission: &mut PlanSubmission,
        channels: &HashMap<ChannelReference, TextureId>,
        screens: &HashMap<ScreenId, TextureId>,
        assets: &mut crate::screens::CalibrationAssets,
        handoff: Handoff<'_>,
    ) -> Result<PreviewImages, String> {
        match self {
            Self::Window(layout) => layout.append(submission, channels, screens, assets, Some(handoff)),
            #[cfg(all(windows, feature = "native-video"))]
            Self::Frames { routes, descriptor } => {
                let patterns = append_patterns(
                    submission,
                    calibrations(routes.iter().map(|route| route.source))?,
                    *descriptor,
                    assets,
                )?;
                for route in routes {
                    if let Some(source) = resolve_texture(route.source, channels, screens, &patterns) {
                        submission.plan.outputs.push(PlanOutput {
                            id: route.output,
                            source,
                        });
                    }
                }
                Ok(PreviewImages::new())
            }
        }
    }
}

fn calibrations(sources: impl IntoIterator<Item = PreviewSource>) -> Result<Vec<(Calibration, Extent2D)>, String> {
    let mut patterns = std::collections::HashSet::new();
    for source in sources {
        if let PreviewSource::Calibration { mode, extent } = source {
            validate_calibration(mode, extent)?;
            patterns.insert((mode, extent));
        }
    }
    if patterns.len() > super::config::MAX_NATIVE_PRODUCERS {
        return Err("calibration previews exceed admitted source count".into());
    }
    let mut patterns = patterns.into_iter().collect::<Vec<_>>();
    patterns.sort_by_key(|(mode, extent)| (*mode as u8, extent.width(), extent.height()));
    Ok(patterns)
}

fn calibration_work(patterns: Vec<(Calibration, Extent2D)>) -> usize {
    patterns
        .into_iter()
        .map(|(mode, extent)| crate::screens::calibration::work_count(mode, extent))
        .sum()
}

fn append_patterns(
    submission: &mut PlanSubmission,
    calibrations: Vec<(Calibration, Extent2D)>,
    descriptor: TextureDescriptor,
    assets: &mut crate::screens::CalibrationAssets,
) -> Result<HashMap<PreviewSource, TextureId>, String> {
    let mut patterns = HashMap::new();
    for (mode, extent) in calibrations {
        let texture = TextureId::new(
            PreviewLayout::CALIBRATION_TEXTURE_BASE
                | ((mode as u64) << 32)
                | (u64::from(extent.width()) << 16)
                | u64::from(extent.height()),
        )
        .unwrap();
        let pattern = crate::screens::calibration::pattern(mode, extent, assets)?
            .ok_or("calibration preview requires an active pattern")?;
        submission.plan.passes.push(RenderPass {
            id: PassId::new(texture.get()).unwrap(),
            output: texture,
            descriptor: TextureDescriptor { extent, ..descriptor },
            operation: RenderOperation::Composite {
                layers: vec![Layer {
                    source: LayerSource::Pattern(pattern),
                    opacity: Opacity::OPAQUE,
                    opacity_envelope: None,
                    blend: BlendMode::Normal,
                    filter: SampleFilter::Linear,
                    mapping: None,
                }],
            },
        });
        patterns.insert(PreviewSource::Calibration { mode, extent }, texture);
    }
    Ok(patterns)
}

fn resolve_texture(
    source: PreviewSource,
    channels: &HashMap<ChannelReference, TextureId>,
    screens: &HashMap<ScreenId, TextureId>,
    patterns: &HashMap<PreviewSource, TextureId>,
) -> Option<TextureId> {
    match source {
        PreviewSource::Channel { .. } | PreviewSource::MatrixCell { .. } => channels.get(&source.reference()?).copied(),
        PreviewSource::Screen { screen } => screens.get(&screen).copied(),
        PreviewSource::Calibration { .. } => patterns.get(&source).copied(),
        PreviewSource::MatrixPlayback { .. } => None,
    }
}

impl PreviewLayout {
    /// The fixed window-compositor passes precede ordinary channel allocations.
    pub(crate) const RESERVED_TEXTURES: u64 = 5;
    pub(crate) const PASS_COUNT: usize = 5;
    /// Calibration route identities encode mode and extent in a separate namespace.
    pub(crate) const CALIBRATION_TEXTURE_BASE: u64 = 1 << 63;

    pub(crate) fn maximum_dependencies(&self) -> Result<usize, String> {
        Ok(self.layout.maximum_dependencies() + calibration_work(calibrations(self.sources()?)?))
    }

    pub(crate) fn pass_count(&self) -> Result<usize, String> {
        Ok(Self::PASS_COUNT + calibrations(self.sources()?)?.len())
    }

    pub fn sources(&self) -> Result<Vec<PreviewSource>, String> {
        let mut sources = self
            .layout
            .source_keys()
            .map(|source| serde_json::from_str(source).map_err(|error| format!("invalid app preview source: {error}")))
            .collect::<Result<Vec<_>, _>>()?;
        for view in &self.layout.views {
            if view.tile_groups.is_some()
                && let Ok(source @ PreviewSource::MatrixPlayback { .. }) = serde_json::from_str(&view.source)
            {
                sources.push(source);
            }
        }
        Ok(sources)
    }
    pub fn references(&self) -> Result<Vec<ChannelReference>, String> {
        Ok(self
            .sources()?
            .into_iter()
            .filter_map(PreviewSource::reference)
            .collect())
    }

    pub(crate) fn append(
        &self,
        submission: &mut PlanSubmission,
        channels: &HashMap<ChannelReference, TextureId>,
        screens: &HashMap<ScreenId, TextureId>,
        assets: &mut crate::screens::CalibrationAssets,
        handoff: Option<Handoff<'_>>,
    ) -> Result<PreviewImages, String> {
        let patterns = append_patterns(submission, calibrations(self.sources()?)?, self.descriptor, assets)?;
        let images = handoff
            .map(|handoff| handoff.images(&self.layout, &mut submission.plan, channels, &patterns))
            .unwrap_or_default();
        let texture = TextureId::new(1).unwrap();
        submission.plan.passes.push(RenderPass {
            id: PassId::new(1).unwrap(),
            output: texture,
            descriptor: self.descriptor,
            operation: RenderOperation::Regions {
                visibility: None,
                regions: self.layout.regions_by_view(self.output, |view, source| {
                    if let Some(image) = images.get(&view.id) {
                        return Some(image.texture());
                    }
                    resolve_texture(serde_json::from_str(source).ok()?, channels, screens, &patterns)
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
            operation: RenderOperation::Clear(self.layout.background),
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
            // Content stays black even without a source; the larger overlay clip retains the
            // themed canvas. Source alpha composites over content black, never the surround.
            operation: RenderOperation::Regions {
                visibility: None,
                regions: self
                    .layout
                    .backdrops(black)
                    .into_iter()
                    // Ordinary backdrops already cover their complete canvas. Reusing their
                    // decorated image regions here paints selection fills twice through alpha.
                    // Only passive tiled views need additional per-image black apertures;
                    // their protocol forbids decorations.
                    .chain(
                        self.layout
                            .regions(self.output, |_| Some(black))
                            .into_iter()
                            .filter(|region| {
                                region
                                    .placement
                                    .as_ref()
                                    .is_some_and(|placement| placement.tile_capacity() > 0)
                            }),
                    )
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
        Ok(images)
    }
}
