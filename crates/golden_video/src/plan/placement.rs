//! Checked, fixed-size live blit placement. Geometry changes do not replace GPU resources.
use super::{MAX_VIEW_OVERLAYS, OverlayPrimitive, RegionFit, UnitRect};
use crate::runtime::CompletionToken;
use std::sync::{Arc, Mutex};

/// Fixed metadata bound for repeated image rectangles sharing one texture binding.
pub const MAX_REGION_TILES: usize = 2048;

#[derive(Clone, Copy, Debug, PartialEq)]
/// One checked image repetition; it owns no texture, source or decoder.
pub struct RegionTile {
    /// Normalized output placement.
    pub destination: UnitRect,
    /// Corresponding normalized source crop.
    pub source_rect: UnitRect,
}

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
    /// Repeated image rectangles, sampled with the same completion token as this placement.
    pub tiles: Arc<[RegionTile]>,
}

#[derive(Debug)]
struct State {
    geometry: RegionGeometry,
    sampled: Option<(CompletionToken, RegionGeometry)>,
    first_use: Option<CompletionToken>,
    presented: Option<(CompletionToken, RegionGeometry)>,
    presentation_output: Option<crate::OutputId>,
}

/// One bounded placement shared by all blits of a view, sampled coherently once per GPU batch.
#[derive(Clone, Debug)]
pub struct RegionPlacement(Arc<Mutex<State>>, usize, usize, RegionFit);

impl RegionPlacement {
    /// Reject invalid initial normalized rectangles before publication.
    pub fn new(destination: UnitRect, source_rect: UnitRect) -> Option<Self> {
        Self::with_overlay_capacity(destination, source_rect, 0)
    }
    /// Reserve a fixed bounded decoration capacity before plan admission. Plain image/backing
    /// placements retain zero capacity; later updates cannot silently grow admitted memory.
    pub fn with_overlay_capacity(destination: UnitRect, source_rect: UnitRect, capacity: usize) -> Option<Self> {
        Self::with_capacities(destination, source_rect, capacity, 0, RegionFit::Stretch)
    }
    /// Reserve repeated-image metadata without increasing source/dependency admission.
    /// A positive capacity selects tiled drawing, including when the current tile set is empty.
    pub fn with_tile_capacity(
        destination: UnitRect,
        source_rect: UnitRect,
        capacity: usize,
        fit: RegionFit,
    ) -> Option<Self> {
        if capacity == 0 || capacity > MAX_REGION_TILES {
            return None;
        }
        Self::with_capacities(destination, source_rect, 0, capacity, fit)
    }
    fn with_capacities(
        destination: UnitRect,
        source_rect: UnitRect,
        capacity: usize,
        tile_capacity: usize,
        fit: RegionFit,
    ) -> Option<Self> {
        (destination.is_valid() && source_rect.is_valid() && capacity <= MAX_VIEW_OVERLAYS).then(|| {
            Self(
                Arc::new(Mutex::new(State {
                    geometry: RegionGeometry {
                        destination,
                        source_rect,
                        overlay_clip: destination,
                        overlay_scale: 1.0,
                        overlays: Arc::from([]),
                        tiles: Arc::from([]),
                    },
                    sampled: None,
                    first_use: None,
                    presented: None,
                    presentation_output: None,
                })),
                capacity,
                tile_capacity,
                fit,
            )
        })
    }
    /// Immutable admitted decoration capacity.
    pub fn overlay_capacity(&self) -> usize {
        self.1
    }
    /// Immutable admitted repetition capacity. Zero retains ordinary single-image drawing.
    pub fn tile_capacity(&self) -> usize {
        self.2
    }
    /// Immutable per-tile presentation, resolved against current native texture dimensions.
    pub fn tile_fit(&self) -> RegionFit {
        self.3
    }
    /// Atomically publish a bounded repetition set. Failed updates retain the old geometry.
    pub fn update_tiles(&self, tiles: &[RegionTile]) -> bool {
        if tiles.len() > self.2
            || !tiles
                .iter()
                .all(|tile| tile.destination.is_valid() && tile.source_rect.is_valid())
        {
            return false;
        }
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.geometry.tiles.as_ref() != tiles {
            state.geometry.tiles = Arc::from(tiles);
            state.first_use = None;
        }
        true
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
            tiles: state.geometry.tiles.clone(),
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
            if state
                .presented
                .as_ref()
                .is_some_and(|(previous, _)| previous.device != token.device)
            {
                state.presented = None;
            }
            state.first_use = Some(token);
        }
        geometry
    }
    /// Promote only the geometry sampled for this exact accepted output frame.
    pub fn mark_presented(&self, output: crate::OutputId, token: CompletionToken) -> bool {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.presentation_output != Some(output) {
            return false;
        }
        let Some((sampled, geometry)) = &state.sampled else {
            return false;
        };
        if *sampled != token {
            return false;
        }
        let geometry = geometry.clone();
        state.presented = Some((token, geometry));
        true
    }
    pub(crate) fn bind_presentation_output(&self, output: crate::OutputId) -> bool {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.presentation_output.is_some_and(|bound| bound != output) {
            return false;
        }
        state.presentation_output = Some(output);
        true
    }
    /// Last output-accepted placement, independently of newer desired/rendered geometry.
    pub fn presented(&self) -> Option<RegionGeometry> {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .presented
            .as_ref()
            .map(|(_, geometry)| geometry.clone())
    }
    /// Withdraw input when the host/device no longer has an installed presentation.
    pub fn clear_presented(&self) {
        self.0.lock().unwrap_or_else(|error| error.into_inner()).presented = None;
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
        // Reserve latest, sampled and accepted arrays plus each bounded opaque hit identity.
        std::mem::size_of::<Mutex<State>>()
            + 12 * std::mem::size_of::<usize>()
            + 3 * self.1 * (std::mem::size_of::<OverlayPrimitive>() + 128)
            + 3 * self.2 * std::mem::size_of::<RegionTile>()
    }
}
