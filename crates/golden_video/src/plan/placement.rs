//! Checked, fixed-size live blit placement. Geometry changes do not replace GPU resources.
use super::{MAX_VIEW_OVERLAYS, OverlayPrimitive, UnitRect};
use crate::runtime::CompletionToken;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq)]
/// Validated source and destination fractions sampled together.
pub struct RegionGeometry {
    /// Normalized output placement.
    pub destination: UnitRect,
    /// Corresponding normalized source crop.
    pub source_rect: UnitRect,
    /// Editor aperture, also admitting handles outside the image bounds.
    pub overlay_clip: UnitRect,
    /// Authenticated screen-unit to physical-pixel scale.
    pub overlay_scale: f32,
    /// Decorations sampled atomically with the image placement.
    pub overlays: Arc<[OverlayPrimitive]>,
}

#[derive(Debug)]
struct State {
    geometry: RegionGeometry,
    sampled: Option<(CompletionToken, RegionGeometry)>,
    first_use: Option<CompletionToken>,
}

/// One bounded placement shared by all blits of a view, sampled coherently once per GPU batch.
#[derive(Clone, Debug)]
pub struct RegionPlacement(Arc<Mutex<State>>, usize);

impl RegionPlacement {
    /// Reject invalid initial normalized rectangles before publication.
    pub fn new(destination: UnitRect, source_rect: UnitRect) -> Option<Self> {
        Self::with_overlay_capacity(destination, source_rect, 0)
    }
    /// Reserve a fixed bounded decoration capacity before plan admission. Plain image/backing
    /// placements retain zero capacity; later updates cannot silently grow admitted memory.
    pub fn with_overlay_capacity(destination: UnitRect, source_rect: UnitRect, capacity: usize) -> Option<Self> {
        (destination.is_valid() && source_rect.is_valid() && capacity <= MAX_VIEW_OVERLAYS).then(|| {
            Self(
                Arc::new(Mutex::new(State {
                    geometry: RegionGeometry {
                        destination,
                        source_rect,
                        overlay_clip: destination,
                        overlay_scale: 1.0,
                        overlays: Arc::from([]),
                    },
                    sampled: None,
                    first_use: None,
                })),
                capacity,
            )
        })
    }
    /// Immutable admitted decoration capacity.
    pub fn overlay_capacity(&self) -> usize {
        self.1
    }
    /// Publish valid latest geometry without allocating or changing resource/admission lifetimes.
    pub fn update(&self, destination: UnitRect, source_rect: UnitRect) -> bool {
        self.update_decorated(destination, source_rect, destination, 1.0, &[])
    }
    /// Atomically publish bounded image placement and its presentation decorations.
    pub fn update_decorated(
        &self,
        destination: UnitRect,
        source_rect: UnitRect,
        overlay_clip: UnitRect,
        overlay_scale: f32,
        overlays: &[OverlayPrimitive],
    ) -> bool {
        if !destination.is_valid()
            || !source_rect.is_valid()
            || !overlay_clip.is_valid()
            || !overlay_scale.is_finite()
            || !(0.05..=64.0).contains(&overlay_scale)
            || overlays.len() > self.1
            || !overlays.iter().all(OverlayPrimitive::is_valid)
        {
            return false;
        }
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let geometry = RegionGeometry {
            destination,
            source_rect,
            overlay_clip,
            overlay_scale,
            overlays: if state.geometry.overlays.as_ref() == overlays {
                state.geometry.overlays.clone()
            } else {
                Arc::from(overlays)
            },
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
        if let Some((previous, geometry)) = &state.sampled
            && *previous == token
        {
            return geometry.clone();
        }
        let geometry = state.geometry.clone();
        state.sampled = Some((token, geometry.clone()));
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
        // Live metadata can grow after plan admission. Reserve both latest and sampled
        // bounded arrays, rather than undercounting a later selection/hover update.
        std::mem::size_of::<Mutex<State>>()
            + 6 * std::mem::size_of::<usize>()
            + 2 * self.1 * std::mem::size_of::<OverlayPrimitive>()
    }
}
