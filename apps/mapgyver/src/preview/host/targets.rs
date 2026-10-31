//! Runtime identities and immutable native topology, owned by the preparation worker.

use crate::{
    preview::capture::CapturedPreview,
    screens::protocol::{PhysicalRectangle, ScreenId, ScreenTarget},
};
use golden_core::app::{NativeOutputBounds, NativeOutputWindow};
use golden_video::native::OutputBinding;
use golden_video::{AlphaMode, ColorEncoding, OutputId, PixelFormat, TextureDescriptor};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub id: OutputId,
    pub kind: TargetKind,
    pub descriptor: TextureDescriptor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum TargetKind {
    Spout(String),
    Rectangle(PhysicalRectangle),
}

impl Target {
    pub fn binding(&self, windows: &mut Vec<NativeOutputWindow>) -> Result<OutputBinding, String> {
        match &self.kind {
            TargetKind::Spout(name) => Ok(OutputBinding::Spout {
                id: self.id,
                name: name.clone(),
                descriptor: self.descriptor,
            }),
            TargetKind::Rectangle(bounds) => {
                let physical =
                    NativeOutputBounds::new(bounds.left, bounds.top, bounds.extent.width(), bounds.extent.height())
                        .map_err(|error| error.to_string())?;
                let window = NativeOutputWindow::new(&format!("MapGyver output {}", self.id.get()), physical)
                    .map_err(|error| error.to_string())?;
                let actual = window.physical_bounds().map_err(|error| error.to_string())?;
                if actual != physical {
                    return Err(format!(
                        "output physical client bounds differ: expected {physical:?}, actual {actual:?}"
                    ));
                }
                eprintln!("[mapgyver-output] window id={} physical={actual:?}", self.id.get());
                // SAFETY: the session owner retains this HWND through actual renderer retirement.
                let target = unsafe { golden_video::native::WindowTarget::from_raw(window.raw_handle()) }
                    .map_err(|error| error.to_string())?;
                windows.push(window);
                Ok(OutputBinding::Window {
                    id: self.id,
                    window: target,
                    extent: bounds.extent,
                })
            }
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
                kind: match &output.target {
                    ScreenTarget::Spout { sender, .. } => TargetKind::Spout(sender.clone()),
                    ScreenTarget::Rectangle { bounds } => TargetKind::Rectangle(*bounds),
                    ScreenTarget::Fullscreen { .. } => return Err("fullscreen display is unresolved".into()),
                },
                descriptor: TextureDescriptor {
                    extent: output.extent,
                    format: PixelFormat::Bgra8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: if matches!(output.target, ScreenTarget::Spout { .. }) {
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
