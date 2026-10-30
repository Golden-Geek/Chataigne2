//! Coalesced native-preview metadata and admission fences. Drive the receiver on a host/preparation
//! worker when notified or its lease deadline is due; never poll UI/engine ticks or transport frames.
mod geometry;
pub mod protocol;
use crate::{
    TextureId,
    plan::{BlitRegion, SampleFilter, UnitRect},
};
pub use geometry::HostViewport;
use golden_io::{RevisionGuard, RevisionSource, WorkerNotifier};
use protocol::{PREVIEW_PROTOCOL_VERSION, PreviewRejection, PreviewRequest, PreviewResponse};
use std::collections::HashSet;
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
}

#[derive(Clone, Debug)]
/// Immutable metadata copied for worker-side render-plan preparation.
pub struct PreviewSnapshot {
    /// Coalesced geometry generation, distinct from wire layout revisions and Video generations.
    pub generation: u64,
    /// Visible, nonempty views in submitted layout order.
    pub views: Vec<ResolvedView>,
    /// Admission fence invalidated when geometry, session ownership or liveness changes.
    pub guard: RevisionGuard,
}
impl PreviewSnapshot {
    /// Resolves logical source keys into native-plan inputs outside the UI/engine thread.
    /// Unknown sources are omitted; this does not establish source availability or frame delivery.
    pub fn regions(&self, mut resolve: impl FnMut(&str) -> Option<TextureId>) -> Vec<BlitRegion> {
        self.views
            .iter()
            .filter_map(|view| {
                resolve(&view.source).map(|source| BlitRegion {
                    source,
                    destination: view.destination,
                    source_rect: view.source_rect,
                    filter: SampleFilter::Linear,
                })
            })
            .collect()
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
    session: Option<Session>,
    views: Vec<ResolvedView>,
    viewport: Option<HostViewport>,
    revision: RevisionSource,
    installed: Option<u64>,
}
impl State {
    fn invalidate(&mut self) {
        self.views.clear();
        self.revision.advance();
        self.installed = None;
    }
    fn expire(&mut self, now: Instant) -> bool {
        if self.session.as_ref().is_some_and(|session| session.deadline <= now) {
            self.session = None;
            self.invalidate();
            true
        } else {
            false
        }
    }
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
                session: None,
                views: Vec::new(),
                viewport: None,
                revision: RevisionSource::default(),
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
                                    state.views = views;
                                    state.viewport = Some(viewport);
                                    state.revision.advance();
                                    state.installed = None;
                                    changed = true;
                                }
                                let current = state.session.as_mut().unwrap();
                                current.revision = Some(revision);
                                current.deadline = now + self.0.limits.lease;
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
        }
    }
    /// Next session expiry to service on the preparation worker when no earlier notification fires.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.0
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .session
            .as_ref()
            .map(|session| session.deadline)
    }
}

#[cfg(test)]
mod tests;
