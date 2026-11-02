//! Checked, fixed-size live blit placement. Geometry changes do not replace GPU resources.
use super::UnitRect;
use crate::runtime::CompletionToken;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq)]
/// Validated source and destination fractions sampled together.
pub struct RegionGeometry {
    /// Normalized output placement.
    pub destination: UnitRect,
    /// Corresponding normalized source crop.
    pub source_rect: UnitRect,
}

#[derive(Debug)]
struct State {
    geometry: RegionGeometry,
    sampled: Option<(CompletionToken, RegionGeometry)>,
    first_use: Option<CompletionToken>,
}

/// One bounded placement shared by all blits of a view, sampled coherently once per GPU batch.
#[derive(Clone, Debug)]
pub struct RegionPlacement(Arc<Mutex<State>>);

impl RegionPlacement {
    /// Reject invalid initial normalized rectangles before publication.
    pub fn new(destination: UnitRect, source_rect: UnitRect) -> Option<Self> {
        (destination.is_valid() && source_rect.is_valid()).then(|| {
            Self(Arc::new(Mutex::new(State {
                geometry: RegionGeometry {
                    destination,
                    source_rect,
                },
                sampled: None,
                first_use: None,
            })))
        })
    }
    /// Publish valid latest geometry without allocating or changing resource/admission lifetimes.
    pub fn update(&self, destination: UnitRect, source_rect: UnitRect) -> bool {
        if !destination.is_valid() || !source_rect.is_valid() {
            return false;
        }
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let geometry = RegionGeometry {
            destination,
            source_rect,
        };
        if state.geometry == geometry {
            return true;
        }
        state.geometry = geometry;
        state.first_use = None;
        true
    }
    /// Sample from the render worker. Shared background/image passes use one coherent placement.
    pub fn sample(&self, token: CompletionToken) -> RegionGeometry {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if let Some((previous, geometry)) = state.sampled
            && previous == token
        {
            return geometry;
        }
        let geometry = state.geometry;
        state.sampled = Some((token, geometry));
        if state.first_use.is_none_or(|first| first.device != token.device) {
            state.first_use = Some(token);
        }
        geometry
    }
    /// Whether the latest geometry was sampled by a batch no later than this accepted output use.
    pub fn presented_through(&self, accepted: CompletionToken) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .first_use
            .is_some_and(|first| first.device == accepted.device && first.serial <= accepted.serial)
    }
    pub(crate) fn retained_heap_bytes(&self) -> usize {
        std::mem::size_of::<Mutex<State>>() + 2 * std::mem::size_of::<usize>()
    }
}
