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
}

impl Target {
    pub fn window_bounds(&self) -> Result<Option<NativeOutputBounds>, String> {
        match &self.kind {
            ResolvedTarget::Spout { .. } => Ok(None),
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
            && match (&self.kind, &next.kind) {
                (ResolvedTarget::Spout { sender: left, .. }, ResolvedTarget::Spout { sender: right, .. }) => {
                    left == right
                }
                (ResolvedTarget::Spout { .. }, _) | (_, ResolvedTarget::Spout { .. }) => false,
                _ => true,
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

#[derive(Default)]
pub(super) struct Targets {
    project: Option<uuid::Uuid>,
    ids: HashMap<ScreenId, OutputId>,
    next: u64,
}

impl Targets {
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
                descriptor: TextureDescriptor {
                    extent: output.extent(),
                    format: PixelFormat::Bgra8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: if matches!(output.target, ResolvedTarget::Spout { .. }) {
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
