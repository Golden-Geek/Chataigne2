//! Runtime identities and immutable native topology, owned by the preparation worker.

use crate::{
    preview::capture::CapturedPreview,
    screens::{
        protocol::{ScreenId, ScreenTarget},
        rendering::ResolvedTarget,
    },
};
use golden_core::app::NativeOutputBounds;
use golden_video::{AlphaMode, ColorEncoding, OutputId, PixelFormat, TextureDescriptor};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub project: uuid::Uuid,
    pub screen: ScreenId,
    pub authored: ScreenTarget,
    pub id: OutputId,
    pub kind: ResolvedTarget,
    pub descriptor: TextureDescriptor,
    pub frame_rate: golden_time::RationalRate,
    #[cfg(all(target_arch = "x86_64", feature = "video-files"))]
    pub ndi_audio: Option<golden_video::ndi::NdiOutputAudio>,
}

impl Target {
    pub fn window_bounds(&self) -> Result<Option<NativeOutputBounds>, String> {
        match &self.kind {
            ResolvedTarget::Spout { .. } | ResolvedTarget::Ndi { .. } => Ok(None),
            ResolvedTarget::Rectangle(bounds) | ResolvedTarget::Fullscreen { bounds, .. } => {
                NativeOutputBounds::new(bounds.left, bounds.top, bounds.extent.width(), bounds.extent.height())
                    .map(Some)
                    .map_err(|error| error.to_string())
            }
        }
    }

    /// Physical position/display policy is independent of immutable renderer bindings.
    pub fn same_binding(&self, next: &Self) -> bool {
        self.project == next.project
            && self.screen == next.screen
            && self.id == next.id
            && self.descriptor == next.descriptor
            && self.frame_rate == next.frame_rate
            && self.same_audio(next)
            && match (&self.kind, &next.kind) {
                (ResolvedTarget::Spout { sender: left, .. }, ResolvedTarget::Spout { sender: right, .. }) => {
                    left == right
                }
                (ResolvedTarget::Ndi { sender: left, .. }, ResolvedTarget::Ndi { sender: right, .. }) => left == right,
                (ResolvedTarget::Spout { .. } | ResolvedTarget::Ndi { .. }, _)
                | (_, ResolvedTarget::Spout { .. } | ResolvedTarget::Ndi { .. }) => false,
                _ => true,
            }
    }

    fn same_audio(&self, next: &Self) -> bool {
        #[cfg(all(target_arch = "x86_64", feature = "video-files"))]
        return self.ndi_audio == next.ndi_audio;
        #[cfg(not(all(target_arch = "x86_64", feature = "video-files")))]
        {
            let _ = next;
            true
        }
    }
}

pub(super) fn same_bindings(current: &[Target], next: &[Target]) -> bool {
    current.len() == next.len()
        && current
            .iter()
            .zip(next)
            .all(|(current, next)| current.same_binding(next))
}

/// Debug fixtures may limit windows without changing authored or recovered display allocation.
#[derive(Default)]
pub(super) enum DisplayOutputs {
    #[default]
    All,
    Disabled,
    FullscreenOnly(String),
}

impl DisplayOutputs {
    pub fn from_environment() -> Self {
        if !cfg!(debug_assertions) {
            return Self::All;
        }
        if std::env::var("MAPGYVER_AUTOMATION_DISABLE_DISPLAY_OUTPUTS").is_ok_and(|value| value == "1") {
            return Self::Disabled;
        }
        match std::env::var("MAPGYVER_AUTOMATION_FULLSCREEN_DISPLAY") {
            Ok(display) if !display.is_empty() => Self::FullscreenOnly(display),
            Err(std::env::VarError::NotPresent) => Self::All,
            _ => Self::Disabled,
        }
    }

    fn admits(&self, target: &ResolvedTarget) -> bool {
        match (self, target) {
            (Self::All, _) | (_, ResolvedTarget::Spout { .. } | ResolvedTarget::Ndi { .. }) => true,
            (Self::FullscreenOnly(allowed), ResolvedTarget::Fullscreen { display, .. }) => allowed == display,
            _ => false,
        }
    }
}

#[derive(Default)]
pub(super) struct Targets {
    project: Option<uuid::Uuid>,
    ids: HashMap<ScreenId, OutputId>,
    next: u64,
    display_outputs: DisplayOutputs,
}

impl Targets {
    pub fn new(display_outputs: DisplayOutputs) -> Self {
        Self {
            display_outputs,
            ..Default::default()
        }
    }

    pub fn assign(&mut self, capture: &mut CapturedPreview) -> Result<Vec<Target>, String> {
        if self.project != Some(capture.channels.project) {
            self.ids.clear();
            self.next = 1; // Output 1 is reserved for the host window.
            self.project = Some(capture.channels.project);
        }
        self.ids.retain(|id, _| capture.catalog.contains(id));
        for &screen in &capture.catalog {
            if let std::collections::hash_map::Entry::Vacant(slot) = self.ids.entry(screen) {
                self.next = self.next.checked_add(1).ok_or("native output identity exhausted")?;
                slot.insert(OutputId::new(self.next).unwrap());
            }
        }
        let mut targets = Vec::new();
        for output in &capture.outputs {
            if !self.display_outputs.admits(&output.target) {
                continue;
            }
            let id = *self.ids.get(&output.screen).ok_or("output Screen was not captured")?;
            let route = capture
                .screens
                .iter_mut()
                .find(|route| route.screen.id() == output.screen)
                .ok_or("output Screen route was not captured")?;
            route.output = Some(id);
            targets.push(Target {
                project: capture.channels.project,
                screen: output.screen,
                authored: output.authored.clone(),
                id,
                kind: output.target.clone(),
                frame_rate: output.frame_rate,
                #[cfg(all(target_arch = "x86_64", feature = "video-files"))]
                ndi_audio: output.ndi_audio.clone(),
                descriptor: TextureDescriptor {
                    extent: output.extent(),
                    format: PixelFormat::Bgra8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: if matches!(output.target, ResolvedTarget::Spout { .. } | ResolvedTarget::Ndi { .. }) {
                        AlphaMode::Straight
                    } else {
                        AlphaMode::Opaque
                    },
                },
            });
        }
        // Authored Screen order changes painter order only within a Screen, not native topology.
        targets.sort_by_key(|target| target.id);
        Ok(targets)
    }
}

#[cfg(test)]
mod tests;
