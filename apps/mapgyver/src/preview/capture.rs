//! Bounded actor capture for every app preview kind; no GPU or IO work on the actor.

use super::protocol::{PreviewSource, PreviewStatus};
use crate::{
    app::AppEngine,
    media::{CapturedChannels, capture_channels},
    screens::rendering::{ScreenRoute, capture_screen, membership_guard},
};
use golden_io::RevisionGuard;
use std::collections::HashSet;

#[derive(Clone)]
pub struct CapturedPreview {
    pub channels: CapturedChannels,
    pub screens: Vec<ScreenRoute>,
    statuses: Vec<PreviewStatus>,
    guards: Vec<RevisionGuard>,
}

impl CapturedPreview {
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
        statuses,
        guards,
    })
}
