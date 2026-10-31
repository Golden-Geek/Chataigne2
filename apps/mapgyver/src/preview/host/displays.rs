//! Metadata-only catalog handoff. OS queries and coarse recovery run on the preparation worker.
use crate::screens::protocol::{DisplayCatalogSnapshot, DisplayInfo};
use golden_io::{RevisionGuard, RevisionSource, WorkerNotifier};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    snapshot: DisplayCatalogSnapshot,
    refresh: bool,
    revision: RevisionSource,
}

#[derive(Clone)]
pub(super) struct Catalog {
    state: Arc<Mutex<State>>,
    notifier: WorkerNotifier,
}

impl Catalog {
    pub fn new(notifier: WorkerNotifier) -> Self {
        Self {
            state: Arc::default(),
            notifier,
        }
    }
    pub fn request(&self, refresh: bool) -> Result<DisplayCatalogSnapshot, String> {
        let mut state = self.state.lock().map_err(|_| "display catalog lock poisoned")?;
        state.refresh |= refresh;
        let snapshot = state.snapshot.clone();
        drop(state);
        if refresh {
            self.notifier.notify();
        }
        Ok(snapshot)
    }
    pub fn take_refresh(&self) -> bool {
        self.state
            .lock()
            .map(|mut state| std::mem::take(&mut state.refresh))
            .unwrap_or(false)
    }
    pub fn guard(&self) -> RevisionGuard {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let reader = state.revision.reader();
        reader.guard_at(reader.current())
    }
    pub fn update(&self, result: Result<Vec<DisplayInfo>, String>) -> Option<DisplayCatalogSnapshot> {
        let (mut displays, error) = match result {
            Ok(displays) => (displays, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        displays.sort_by(|left, right| left.id.cmp(&right.id));
        let mut state = self.state.lock().ok()?;
        if state.snapshot.displays == displays && state.snapshot.error == error {
            return None;
        }
        state.snapshot.revision = state.snapshot.revision.checked_add(1)?;
        state.snapshot.displays = displays;
        state.snapshot.error = error;
        state.revision.advance();
        Some(state.snapshot.clone())
    }
}

pub(super) fn scan() -> Result<Vec<DisplayInfo>, String> {
    golden_core::app::native_displays()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|display| {
            Ok(DisplayInfo {
                id: display.id,
                name: display.name,
                bounds: crate::screens::protocol::PhysicalRectangle {
                    left: display.bounds.left(),
                    top: display.bounds.top(),
                    extent: golden_video::Extent2D::new(display.bounds.width(), display.bounds.height())
                        .map_err(|error| error.to_string())?,
                },
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
