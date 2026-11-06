//! Coalesced native-preview metadata and admission fences. Drive the receiver on a host/preparation
//! worker when notified or its lease deadline is due; never poll UI/engine ticks or transport frames.
mod geometry;
mod pointer;
pub mod protocol;
mod tiles;
use crate::{
    TextureId,
    plan::{BlitRegion, SampleFilter, UnitRect},
};
pub use geometry::HostViewport;
use golden_io::{RevisionGuard, RevisionSource, WorkerNotifier};
use protocol::{PREVIEW_PROTOCOL_VERSION, PreviewRejection, PreviewRequest, PreviewResponse};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
/// Fixed metadata admission and session-liveness limits for one native preview owner.
pub struct PreviewLimits {
    /// Maximum submitted view definitions per layout, including hidden or clipped views.
    pub max_views: u32,
    /// Time from an accepted layout to expiry unless another layout refreshes the session.
    pub lease: Duration,
}
impl Default for PreviewLimits {
    fn default() -> Self {
        Self {
            max_views: 64,
            lease: Duration::from_millis(2500),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// One visible view after authenticated host scaling and client/WebView clipping.
pub struct ResolvedView {
    /// Consumer-supplied identity within this preview session.
    pub id: Uuid,
    /// Opaque source key resolved by the native consumer, never a native resource handle.
    pub source: String,
    /// Clipped destination normalized to the host's physical client extent.
    pub destination: UnitRect,
    /// Portion of the full source image remaining after destination clipping.
    pub source_rect: UnitRect,
    /// Stationary clipped editor backing, independent of the moving image rectangle.
    pub backdrop: UnitRect,
    /// Validated, tightly retained image-relative editor presentation.
    pub overlays: Arc<[crate::plan::OverlayPrimitive]>,
    /// Host-authenticated CSS-to-physical scale for decoration widths and radii.
    pub overlay_scale: f32,
    /// Fixed retained decoration capacity for this view's plan lifetime.
    pub overlay_capacity: usize,
    /// Passive source bindings within this logical view.
    pub tile_groups: Option<Arc<[ResolvedTileGroup]>>,
}

#[derive(Clone, Debug, PartialEq)]
/// Checked repetitions sharing one source binding, independent of registration count.
pub struct ResolvedTileGroup {
    /// Opaque consumer-owned source key.
    pub source: String,
    /// Host-clipped image repetitions.
    pub tiles: Arc<[crate::plan::RegionTile]>,
    /// Immutable admitted repetition capacity.
    pub capacity: usize,
    /// Source-independent image presentation, applied by the native renderer per frame.
    pub fit: crate::plan::RegionFit,
}

#[derive(Clone, Debug)]
/// Immutable metadata copied for worker-side render-plan preparation.
pub struct PreviewSnapshot {
    /// Coalesced layout generation, distinct from wire revisions and Video generations.
    pub generation: u64,
    /// Visible, nonempty views in submitted layout order.
    pub views: Vec<ResolvedView>,
    /// Full metadata fence, invalidated by geometry as well as routing/lifetime changes.
    pub guard: RevisionGuard,
    /// Resource routing changes independently of live pan/zoom placement.
    pub routing_generation: u64,
    /// Admission lifetime for the source/view topology, unaffected by geometry-only updates.
    pub routing_guard: RevisionGuard,
    view_guards: HashMap<Uuid, RevisionGuard>,
    placements: HashMap<Uuid, crate::plan::RegionPlacement>,
    backdrops: HashMap<Uuid, crate::plan::RegionPlacement>,
    tile_placements: tiles::Placements,
}
impl PreviewSnapshot {
    /// Builds a snapshot whose destinations share one visibility lifetime. Controller snapshots
    /// retain independent destination lifetimes so unrelated layout edits do not clear live views.
    pub fn new(generation: u64, views: Vec<ResolvedView>, guard: RevisionGuard) -> Self {
        let placements = views
            .iter()
            .filter(|view| !view.overlays.is_empty() || view.tile_groups.is_some())
            .map(|view| {
                let placement = create_placement(view);
                (view.id, placement)
            })
            .collect();
        let mut tile_placements = HashMap::new();
        tiles::update(&mut tile_placements, &views);
        Self {
            generation,
            views,
            routing_generation: generation,
            routing_guard: guard.clone(),
            guard,
            view_guards: HashMap::new(),
            placements,
            backdrops: HashMap::new(),
            tile_placements,
        }
    }

    /// Resolves logical source keys into native-plan inputs outside the UI/engine thread.
    /// Unknown sources are omitted; this does not establish source availability or frame delivery.
    pub fn regions(
        &self,
        output: crate::OutputId,
        mut resolve: impl FnMut(&str) -> Option<TextureId>,
    ) -> Vec<BlitRegion> {
        let mut regions = Vec::new();
        for view in &self.views {
            if self
                .placements
                .get(&view.id)
                .is_some_and(|placement| !placement.bind_presentation_output(output))
            {
                continue;
            }
            let visibility = self.view_guards.get(&view.id).unwrap_or(&self.guard);
            if let Some(groups) = &view.tile_groups {
                regions.extend(tiles::regions(
                    view.id,
                    groups,
                    &self.tile_placements,
                    output,
                    visibility,
                    &mut resolve,
                ));
                continue;
            }
            if let Some(source) = resolve(&view.source) {
                regions.push(BlitRegion {
                    source,
                    destination: view.destination,
                    source_rect: view.source_rect,
                    filter: SampleFilter::Linear,
                    visibility: Some(self.view_guards.get(&view.id).unwrap_or(&self.guard).clone()),
                    placement: self.placements.get(&view.id).cloned(),
                });
            }
        }
        regions
    }

    /// Black editor backing stays under the complete canvas while its active image moves.
    pub fn backdrops(&self, source: TextureId) -> Vec<BlitRegion> {
        self.views
            .iter()
            .map(|view| BlitRegion {
                source,
                destination: view.backdrop,
                source_rect: UnitRect::FULL,
                filter: SampleFilter::Nearest,
                visibility: Some(self.view_guards.get(&view.id).unwrap_or(&self.guard).clone()),
                placement: if view.tile_groups.is_some() {
                    self.placements.get(&view.id).cloned()
                } else {
                    self.backdrops.get(&view.id).cloned()
                },
            })
            .collect()
    }

    /// Source keys actually used by image bindings; a passive parent source is an owner key.
    pub fn source_keys(&self) -> impl Iterator<Item = &str> {
        self.views.iter().flat_map(|view| {
            view.tile_groups
                .iter()
                .flat_map(|groups| groups.iter().map(|group| group.source.as_str()))
                .chain(view.tile_groups.is_none().then_some(view.source.as_str()))
        })
    }

    /// Conservative source/backing dependency reservation, including each passive parent marker.
    pub fn maximum_dependencies(&self) -> usize {
        self.views
            .iter()
            .map(|view| view.tile_groups.as_ref().map_or(3, |groups| groups.len() * 3 + 1))
            .sum::<usize>()
            .saturating_add(3)
    }
}

struct Session {
    client: Uuid,
    id: Uuid,
    revision: Option<u32>,
    deadline: Instant,
}
struct State {
    closed: bool,
    suspended_at: Option<Instant>,
    session: Option<Session>,
    views: Vec<ResolvedView>,
    viewport: Option<HostViewport>,
    revision: RevisionSource,
    routing: RevisionSource,
    placements: HashMap<Uuid, crate::plan::RegionPlacement>,
    backdrops: HashMap<Uuid, crate::plan::RegionPlacement>,
    tile_placements: tiles::Placements,
    view_revisions: HashMap<Uuid, RevisionSource>,
    installed: Option<u64>,
}
impl State {
    fn set_host_suspended(&mut self, suspended: bool, now: Instant) -> bool {
        let expired = self.expire(now);
        match (self.suspended_at, suspended) {
            (None, true) => self.suspended_at = Some(now),
            (Some(start), false) => {
                if let Some(session) = &mut self.session {
                    session.deadline += now.saturating_duration_since(start);
                }
                self.suspended_at = None;
            }
            _ => {}
        }
        expired
    }
    fn invalidate(&mut self) {
        self.views.clear();
        self.view_revisions.clear();
        self.placements.clear();
        self.backdrops.clear();
        self.tile_placements.clear();
        self.routing.advance();
        self.revision.advance();
        self.installed = None;
    }
    fn replace_views(&mut self, views: Vec<ResolvedView>, viewport: HostViewport) {
        let routing_changed = self.viewport != Some(viewport)
            || self.views.len() != views.len()
            || self.views.iter().zip(&views).any(|(old, new)| {
                old.id != new.id
                    || old.source != new.source
                    || old.overlay_capacity != new.overlay_capacity
                    || !tiles::same_routing(old, new)
            });
        if routing_changed {
            self.routing.advance();
        }
        if self.viewport != Some(viewport) {
            self.view_revisions.clear();
            self.placements.clear();
            self.backdrops.clear();
            self.tile_placements.clear();
        } else {
            let previous = self
                .views
                .iter()
                .enumerate()
                .map(|(index, view)| (view.id, (index, view)))
                .collect::<HashMap<_, _>>();
            let current = views
                .iter()
                .enumerate()
                .map(|(index, view)| (view.id, (index, view)))
                .collect::<HashMap<_, _>>();
            self.view_revisions.retain(|id, _| {
                previous
                    .get(id)
                    .zip(current.get(id))
                    .is_some_and(|((old_index, old), (new_index, new))| {
                        // Keep the installed image while its checked placement updates live.
                        // A reordered overlap changes which live image owns those pixels.
                        // Independent views may reorder without withdrawing their destinations.
                        !previous.iter().any(|(other, (old_other_index, old_other))| {
                            current.get(other).is_some_and(|(new_other_index, new_other)| {
                                old_index.cmp(old_other_index) != new_index.cmp(new_other_index)
                                    && (overlaps(old.destination, old_other.destination)
                                        || overlaps(new.destination, new_other.destination))
                            })
                        })
                    })
            });
        }
        for view in &views {
            if self.placements.get(&view.id).is_some_and(|placement| {
                placement.overlay_capacity() != view.overlay_capacity
                    || placement.tile_capacity() != usize::from(view.tile_groups.is_some())
                    || self
                        .views
                        .iter()
                        .any(|old| old.id == view.id && old.source != view.source)
            }) {
                self.placements.remove(&view.id);
            }
            self.backdrops
                .entry(view.id)
                .and_modify(|placement| {
                    assert!(placement.update(view.backdrop, UnitRect::FULL));
                })
                .or_insert_with(|| {
                    crate::plan::RegionPlacement::new(view.backdrop, UnitRect::FULL).expect("resolved preview backing")
                });
            self.view_revisions.entry(view.id).or_default();
            self.placements
                .entry(view.id)
                .and_modify(|placement| {
                    update_placement(placement, view);
                })
                .or_insert_with(|| create_placement(view));
        }
        self.placements.retain(|id, _| views.iter().any(|view| view.id == *id));
        self.backdrops.retain(|id, _| views.iter().any(|view| view.id == *id));
        tiles::update(&mut self.tile_placements, &views);
        self.views = views;
        self.viewport = Some(viewport);
        self.revision.advance();
        self.installed = None;
    }
    fn expire(&mut self, now: Instant) -> bool {
        if self.suspended_at.is_some() {
            return false;
        }
        if self.session.as_ref().is_some_and(|session| session.deadline <= now) {
            self.session = None;
            self.invalidate();
            true
        } else {
            false
        }
    }
}

fn create_placement(view: &ResolvedView) -> crate::plan::RegionPlacement {
    let placement = if view.tile_groups.is_some() {
        // An empty tiled binding samples parent geometry without painting an opaque aperture.
        crate::plan::RegionPlacement::with_tile_capacity(
            view.destination,
            view.source_rect,
            1,
            crate::plan::RegionFit::Stretch,
        )
    } else {
        crate::plan::RegionPlacement::with_overlay_capacity(view.destination, view.source_rect, view.overlay_capacity)
    }
    .expect("resolved preview geometry");
    update_placement(&placement, view);
    placement
}

fn update_placement(placement: &crate::plan::RegionPlacement, view: &ResolvedView) {
    assert!(placement.update_decorated(
        view.destination,
        view.source_rect,
        view.backdrop,
        view.overlay_scale,
        &view.overlays
    ));
}

fn overlaps(left: UnitRect, right: UnitRect) -> bool {
    left.x < right.x + right.width
        && right.x < left.x + left.width
        && left.y < right.y + right.height
        && right.y < left.y + left.height
}
struct Shared {
    state: Mutex<State>,
    limits: PreviewLimits,
    notify: Option<WorkerNotifier>,
}
impl Shared {
    fn notify(&self) {
        if let Some(notifier) = &self.notify {
            notifier.notify();
        }
    }
}

#[derive(Clone)]
/// Bounded metadata control for one host-owned native preview window.
/// Explicitly close it when that presentation lifetime ends.
pub struct PreviewController(Arc<Shared>);
#[derive(Clone)]
/// Worker-side geometry snapshots and installation acknowledgement for the same preview owner.
pub struct PreviewReceiver(Arc<Shared>);

impl PreviewController {
    /// Creates paired control/worker handles after validating the fixed admission limits.
    /// Notifications carry readiness only; rendering and preparation remain consumer-owned.
    pub fn new(limits: PreviewLimits, notifier: Option<WorkerNotifier>) -> Result<(Self, PreviewReceiver), String> {
        if limits.max_views == 0
            || limits.max_views > 256
            || limits.lease < Duration::from_millis(100)
            || limits.lease > Duration::from_secs(60)
        {
            return Err("invalid native preview limits".into());
        }
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                closed: false,
                suspended_at: None,
                session: None,
                views: Vec::new(),
                viewport: None,
                revision: RevisionSource::default(),
                routing: RevisionSource::default(),
                placements: HashMap::new(),
                backdrops: HashMap::new(),
                tile_placements: HashMap::new(),
                view_revisions: HashMap::new(),
                installed: None,
            }),
            limits,
            notify: notifier,
        });
        Ok((Self(shared.clone()), PreviewReceiver(shared)))
    }

    /// Parses and handles the bounded local control envelope using host-authenticated metrics.
    /// The host supplies `viewport`; frontend scale claims are never accepted here.
    pub fn control_json(&self, request: &str, viewport: HostViewport) -> Result<String, String> {
        if request.len() > 64 * 1024 {
            return Err("native preview request exceeds64KiB".into());
        }
        let request = serde_json::from_str(request).map_err(|error| error.to_string())?;
        serde_json::to_string(&self.handle(request, viewport)).map_err(|error| error.to_string())
    }

    /// Applies negotiation, layout, status or disconnect metadata without doing native work.
    /// New layouts coalesce desired geometry; acceptance does not acknowledge installed frames.
    pub fn handle(&self, request: PreviewRequest, viewport: HostViewport) -> PreviewResponse {
        let now = Instant::now();
        let mut changed = false;
        let mut state = self.0.state.lock().unwrap_or_else(|error| error.into_inner());
        changed |= state.expire(now);
        let response = if state.closed {
            PreviewResponse::Rejected {
                code: PreviewRejection::Closed,
            }
        } else {
            match request {
                PreviewRequest::Pointer {
                    session,
                    view,
                    source,
                    point,
                } => {
                    if state.session.as_ref().is_none_or(|current| current.id != session) {
                        PreviewResponse::Rejected {
                            code: PreviewRejection::Session,
                        }
                    } else if !viewport.valid()
                        || state.viewport != Some(viewport)
                        || !point
                            .iter()
                            .all(|value| value.is_finite() && value.abs() <= 1_000_000.0)
                    {
                        PreviewResponse::Rejected {
                            code: PreviewRejection::Geometry,
                        }
                    } else {
                        let sample = state
                            .views
                            .iter()
                            .any(|registered| {
                                registered.id == view && registered.source == source && registered.tile_groups.is_none()
                            })
                            .then(|| {
                                state
                                    .placements
                                    .get(&view)
                                    .and_then(crate::plan::RegionPlacement::presented)
                            })
                            .flatten()
                            .and_then(|geometry| pointer::project(geometry, viewport, point));
                        PreviewResponse::Pointer { sample }
                    }
                }
                PreviewRequest::Connect { client, version } => {
                    if version != PREVIEW_PROTOCOL_VERSION {
                        PreviewResponse::Rejected {
                            code: PreviewRejection::Version,
                        }
                    } else {
                        if state.session.as_ref().is_none_or(|session| session.client != client) {
                            state.invalidate();
                            state.session = Some(Session {
                                client,
                                id: Uuid::new_v4(),
                                revision: None,
                                deadline: now + self.0.limits.lease,
                            });
                            if state.suspended_at.is_some() {
                                state.suspended_at = Some(now);
                            }
                            changed = true;
                        }
                        PreviewResponse::Connected {
                            session: state.session.as_ref().unwrap().id,
                            version: PREVIEW_PROTOCOL_VERSION,
                            max_views: self.0.limits.max_views,
                        }
                    }
                }
                PreviewRequest::Disconnect { session } => {
                    if state.session.as_ref().is_none_or(|current| current.id != session) {
                        PreviewResponse::Rejected {
                            code: PreviewRejection::Session,
                        }
                    } else {
                        state.session = None;
                        state.invalidate();
                        changed = true;
                        PreviewResponse::Disconnected
                    }
                }
                PreviewRequest::Status { session, revision } => {
                    if state.session.as_ref().is_none_or(|current| current.id != session) {
                        PreviewResponse::Rejected {
                            code: PreviewRejection::Session,
                        }
                    } else if state.session.as_ref().unwrap().revision != Some(revision) {
                        PreviewResponse::Rejected {
                            code: PreviewRejection::Revision,
                        }
                    } else if !viewport.valid() || state.viewport != Some(viewport) {
                        state.invalidate();
                        changed = true;
                        PreviewResponse::Rejected {
                            code: PreviewRejection::Geometry,
                        }
                    } else {
                        PreviewResponse::Applied {
                            revision,
                            visible_views: state.views.len() as u32,
                            installed: state.installed == Some(state.revision.reader().current()),
                        }
                    }
                }
                PreviewRequest::Layout {
                    session,
                    revision,
                    views,
                } => {
                    if state.session.as_ref().is_none_or(|current| current.id != session) {
                        PreviewResponse::Rejected {
                            code: PreviewRejection::Session,
                        }
                    } else if state
                        .session
                        .as_ref()
                        .unwrap()
                        .revision
                        .is_some_and(|current| revision <= current)
                    {
                        PreviewResponse::Rejected {
                            code: PreviewRejection::Revision,
                        }
                    } else {
                        let resolved = (|| {
                            if views.len() > self.0.limits.max_views as usize {
                                return Err(PreviewRejection::Capacity);
                            }
                            let (mut groups, mut tiles, mut capacity) = (0usize, 0usize, 0u64);
                            for group in views.iter().filter_map(|view| view.tile_groups.as_ref()).flatten() {
                                groups += 1;
                                tiles = tiles.saturating_add(group.tiles.len());
                                capacity += u64::from(group.capacity);
                                if groups > crate::plan::MAX_REGION_TILES
                                    || tiles > crate::plan::MAX_REGION_TILES
                                    || capacity > (2 * crate::plan::MAX_REGION_TILES) as u64
                                {
                                    return Err(PreviewRejection::Capacity);
                                }
                            }
                            if !viewport.valid() {
                                return Err(PreviewRejection::Geometry);
                            }
                            let mut seen = HashSet::new();
                            let mut resolved = Vec::new();
                            for view in views {
                                if !seen.insert(view.id) {
                                    return Err(PreviewRejection::Duplicate);
                                }
                                if let Some(view) = geometry::resolve(view, viewport)? {
                                    resolved.push(view);
                                }
                            }
                            Ok(resolved)
                        })();
                        match resolved {
                            Ok(views) => {
                                if state.views != views || state.viewport != Some(viewport) {
                                    state.replace_views(views, viewport);
                                    changed = true;
                                }
                                let current = state.session.as_mut().unwrap();
                                current.revision = Some(revision);
                                current.deadline = now + self.0.limits.lease;
                                if state.suspended_at.is_some() {
                                    state.suspended_at = Some(now);
                                }
                                PreviewResponse::Applied {
                                    revision,
                                    visible_views: state.views.len() as u32,
                                    installed: state.installed == Some(state.revision.reader().current()),
                                }
                            }
                            Err(code) => {
                                state.invalidate();
                                changed = true;
                                PreviewResponse::Rejected { code }
                            }
                        }
                    }
                }
            }
        };
        drop(state);
        if changed {
            self.0.notify();
        }
        response
    }

    /// Permanently rejects new control, clears registered views and invalidates captured geometry.
    /// This metadata transition does not wait for or certify native resource retirement.
    pub fn close(&self) {
        let mut state = self.0.state.lock().unwrap_or_else(|error| error.into_inner());
        state.closed = true;
        state.session = None;
        state.invalidate();
        drop(state);
        self.0.notify();
    }
}

impl PreviewReceiver {
    /// Pauses only the liveness clock while the trusted native host reports its owned UI is
    /// suspended. Geometry/owner fences and explicit close remain active. On resume the exact
    /// remaining lease is restored; expired sessions cannot be revived by entering suspension.
    pub fn set_host_suspended(&self, suspended: bool) {
        let mut state = self.0.state.lock().unwrap_or_else(|error| error.into_inner());
        let changed = state.set_host_suspended(suspended, Instant::now());
        drop(state);
        if changed {
            self.0.notify();
        }
    }

    /// Confirm live geometry only after both its resource plan and sampled frame were accepted.
    /// The returned full layout generation may advance without replacing resource routing.
    pub fn mark_frame_installed(&self, routing: u64, accepted: crate::runtime::CompletionToken) -> Option<u64> {
        let mut state = self.0.state.lock().unwrap_or_else(|error| error.into_inner());
        state.expire(Instant::now());
        if state.closed
            || state.routing.reader().current() != routing
            || !state
                .placements
                .values()
                .chain(state.tile_placements.values())
                .all(|placement| placement.presented_through(accepted))
        {
            return None;
        }
        let generation = state.revision.reader().current();
        state.installed = Some(generation);
        Some(generation)
    }
    /// Withdraws a prior installation when its renderer is rebuilding or unavailable.
    /// Geometry stays registered; only a fresh installation/frame proof may expose it again.
    pub fn mark_uninstalled(&self, generation: u64) -> bool {
        let mut state = self.0.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.closed || state.revision.reader().current() != generation {
            return false;
        }
        state.installed = None;
        true
    }
    /// Acknowledges installation only for the current, unexpired geometry generation.
    /// The caller must first observe a reliable Video Installed outcome and a submitted frame
    /// with that exact installed identity. This does not prove scanout, delivery or source readiness.
    /// Stale generations cannot acknowledge a replacement layout.
    pub fn mark_installed(&self, generation: u64) -> bool {
        let mut state = self.0.state.lock().unwrap_or_else(|error| error.into_inner());
        state.expire(Instant::now());
        if state.closed || state.revision.reader().current() != generation {
            return false;
        }
        state.installed = Some(generation);
        true
    }
    /// Bounded metadata copy on a geometry notification/lease deadline, outside per-frame work.
    pub fn snapshot(&self) -> PreviewSnapshot {
        let mut state = self.0.state.lock().unwrap_or_else(|error| error.into_inner());
        state.expire(Instant::now());
        let reader = state.revision.reader();
        PreviewSnapshot {
            generation: reader.current(),
            views: state.views.clone(),
            guard: reader.guard_at(reader.current()),
            routing_generation: state.routing.reader().current(),
            routing_guard: state.routing.reader().guard_at(state.routing.reader().current()),
            placements: state.placements.clone(),
            backdrops: state.backdrops.clone(),
            tile_placements: state.tile_placements.clone(),
            view_guards: state
                .view_revisions
                .iter()
                .map(|(id, source)| {
                    let reader = source.reader();
                    (*id, reader.guard_at(reader.current()))
                })
                .collect(),
        }
    }
    /// Next session expiry to service on the preparation worker when no earlier notification fires.
    pub fn next_deadline(&self) -> Option<Instant> {
        let state = self.0.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.suspended_at.is_some() {
            None
        } else {
            state.session.as_ref().map(|session| session.deadline)
        }
    }
}

#[cfg(test)]
mod tests;
