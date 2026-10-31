//! Bounded actor capture for every app preview kind; no GPU or IO work on the actor.

use super::protocol::{PreviewSource, PreviewStatus};
use crate::{
    app::AppEngine,
    media::{CapturedChannels, capture_channels},
    screens::{
        protocol::{DisplayInfo, ScreenId},
        rendering::{CapturedOutput, ScreenRoute, capture_outputs, capture_screen, membership_guard},
    },
};
use golden_io::RevisionGuard;
use std::collections::HashSet;

#[derive(Clone)]
pub struct CapturedPreview {
    pub channels: CapturedChannels,
    pub screens: Vec<ScreenRoute>,
    pub catalog: Vec<ScreenId>,
    pub outputs: Vec<CapturedOutput>,
    statuses: Vec<PreviewStatus>,
    guards: Vec<RevisionGuard>,
}

impl CapturedPreview {
    #[cfg(all(windows, feature = "native-video"))]
    pub(crate) fn add_revision_guard(&mut self, guard: RevisionGuard) {
        let mut guards = self.channels.guards.to_vec();
        guards.push(guard);
        self.channels.guards = guards.into();
    }
    pub fn statuses(&self, channels: &[crate::media::rendering::PreparedChannel]) -> Vec<PreviewStatus> {
        use crate::media::resolution::ChannelAvailability;
        let mut statuses = self.statuses.clone();
        for channel in channels {
            statuses.push(PreviewStatus {
                source: PreviewSource::Channel {
                    reference: channel.reference,
                },
                extent: channel.resolution.extent,
                error: None,
                message: (channel.resolution.availability != ChannelAvailability::Ready)
                    .then(|| format!("Source {:?}", channel.resolution.availability)),
            });
        }
        for route in &self.screens {
            let references = route.screen.references();
            let missing = references
                .iter()
                .filter(|reference| {
                    !channels.iter().any(|channel| {
                        channel.reference == **reference
                            && channel.resolution.availability == ChannelAvailability::Ready
                    })
                })
                .count();
            if missing > 0
                && let Some(status) = statuses.iter_mut().find(|status| {
                    status.source
                        == PreviewSource::Screen {
                            screen: route.screen.id(),
                        }
                })
            {
                status.message = Some(format!("{missing} Screen source(s) unavailable"));
            }
        }
        statuses
    }
    pub fn is_current(&self) -> bool {
        self.channels.is_current()
            && self.guards.iter().all(RevisionGuard::is_current)
            && self.screens.iter().all(|route| route.screen.is_current())
    }
}

pub fn capture_preview(
    engine: &AppEngine,
    sources: &[PreviewSource],
    maximum_sources: usize,
    maximum_surfaces: usize,
) -> Result<CapturedPreview, String> {
    if sources.len() > maximum_sources {
        return Err("preview exceeds admitted view count".into());
    }
    let mut seen = HashSet::new();
    let mut channels = Vec::new();
    let mut screens = Vec::new();
    let mut statuses = Vec::new();
    let mut guards = vec![membership_guard(engine)?];
    for &source in sources {
        if !seen.insert(source) {
            continue;
        }
        match source {
            PreviewSource::Channel { reference } => channels.push(reference),
            PreviewSource::Screen { screen } => {
                if let Some(guard) = crate::screens::rendering::screen_guard(engine, screen) {
                    guards.push(guard);
                }
                let captured = capture_screen(engine, screen, maximum_surfaces);
                let (extent, error, message) = match captured {
                    Ok(captured) => {
                        guards.extend(captured.guards.iter().cloned());
                        let extent = captured.preview_extent();
                        let references = captured.references();
                        let message = (!captured.has_content()).then(|| "No enabled sources assigned".into());
                        channels.extend(references);
                        screens.push(ScreenRoute {
                            output: None,
                            extent,
                            screen: captured,
                        });
                        (Some(extent), None, message)
                    }
                    Err(error) => (None, Some(error), None),
                };
                statuses.push(PreviewStatus {
                    source,
                    extent,
                    error,
                    message,
                });
            }
        }
    }
    let mut unique = HashSet::new();
    channels.retain(|source| unique.insert(*source));
    Ok(CapturedPreview {
        channels: capture_channels(engine, &channels, maximum_sources)?,
        screens,
        catalog: Vec::new(),
        outputs: Vec::new(),
        statuses,
        guards,
    })
}

/// Union of editor demand and enabled outputs. Channel and Screen passes remain shared, while
/// closing/minimizing all previews cannot withdraw an authored virtual output.
pub fn capture_presentation(
    engine: &AppEngine,
    sources: &[PreviewSource],
    maximum_sources: usize,
    maximum_surfaces: usize,
    displays: &[DisplayInfo],
) -> Result<CapturedPreview, String> {
    let mut result = capture_preview(engine, sources, maximum_sources, maximum_surfaces)?;
    let captured = capture_outputs(engine, 128, crate::screens::rendering::MAX_SCREEN_OUTPUTS, displays)?;
    result.catalog = captured.catalog;
    result.guards.extend(captured.guards);
    for route in &mut result.screens {
        if let Some(&extent) = captured.preview_extents.get(&route.screen.id()) {
            route.extent = extent;
            if let Some(status) = result.statuses.iter_mut().find(|status| {
                status.source
                    == PreviewSource::Screen {
                        screen: route.screen.id(),
                    }
            }) {
                status.extent = Some(extent);
            }
        }
    }
    let mut references = sources
        .iter()
        .filter_map(|source| match source {
            PreviewSource::Channel { reference } => Some(*reference),
            PreviewSource::Screen { .. } => None,
        })
        .collect::<Vec<_>>();
    for route in &result.screens {
        references.extend(route.screen.references());
    }
    for output in &captured.outputs {
        if let Some(route) = result
            .screens
            .iter_mut()
            .find(|route| route.screen.id() == output.screen)
        {
            route.extent = output.extent();
            if let Some(status) = result
                .statuses
                .iter_mut()
                .find(|status| status.source == PreviewSource::Screen { screen: output.screen })
            {
                status.extent = Some(output.extent());
            }
            continue;
        }
        let screen = capture_screen(engine, output.screen, maximum_surfaces)?;
        result.statuses.push(PreviewStatus {
            source: PreviewSource::Screen { screen: output.screen },
            extent: Some(output.extent()),
            error: None,
            message: (!screen.has_content()).then(|| "No enabled sources assigned".into()),
        });
        references.extend(screen.references());
        result.screens.push(ScreenRoute {
            output: None,
            extent: output.extent(),
            screen,
        });
    }
    let mut seen = HashSet::new();
    references.retain(|reference| seen.insert(*reference));
    result.channels = capture_channels(engine, &references, maximum_sources)?;
    result.outputs = captured.outputs;
    for (screen, reason) in captured.unavailable {
        let source = PreviewSource::Screen { screen };
        let message = Some(reason);
        if let Some(status) = result.statuses.iter_mut().find(|status| status.source == source) {
            status.message = message;
        } else {
            result.statuses.push(PreviewStatus {
                source,
                extent: None,
                error: None,
                message,
            });
        }
    }
    Ok(result)
}
