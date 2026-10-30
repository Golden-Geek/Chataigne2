//! Runtime identities and immutable native topology, owned by the preparation worker.

use crate::{preview::capture::CapturedPreview, screens::protocol::ScreenId};
use golden_video::native::OutputBinding;
use golden_video::{AlphaMode, ColorEncoding, OutputId, PixelFormat, TextureDescriptor};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub id: OutputId,
    pub name: String,
    pub descriptor: TextureDescriptor,
}

impl Target {
    pub fn binding(&self) -> OutputBinding {
        OutputBinding::Spout {
            id: self.id,
            name: self.name.clone(),
            descriptor: self.descriptor,
        }
    }
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
                id,
                name: output.sender.clone(),
                descriptor: TextureDescriptor {
                    extent: output.extent,
                    format: PixelFormat::Bgra8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: AlphaMode::Straight,
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
