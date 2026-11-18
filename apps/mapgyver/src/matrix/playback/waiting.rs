//! Hold explicit navigation against the active finite clocks in its authored scope.
use super::*;
use crate::{
    matrix::{
        controls::WaitMode,
        view::navigation::{self, Step, StepScope},
    },
    media::video::{TransportSample, VideoControl},
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Columns(Option<uuid::Uuid>),
    Layer {
        base: Option<uuid::Uuid>,
        cell: Option<(Option<ColumnId>, Option<uuid::Uuid>)>,
    },
}

impl Anchor {
    fn read(snapshot: &MatrixPlaybackSnapshot, scope: StepScope) -> Self {
        match scope {
            StepScope::Layer(layer) => Self::Layer {
                base: snapshot.launch_id,
                cell: snapshot
                    .layers
                    .iter()
                    .find(|request| request.layer == layer)
                    .map(|request| (request.column, request.launch_id)),
            },
            _ => Self::Columns(snapshot.request_id),
        }
    }
}

struct Clock {
    layer: LayerId,
    column: ColumnId,
    launch: uuid::Uuid,
    control: Arc<VideoControl>,
}

struct Request {
    step: Step,
    mode: WaitMode,
    anchor: Anchor,
    clocks: Vec<Clock>,
}

// Column launches share one slot. Independent Layers each have their own latest request.
type Slot = Option<uuid::Uuid>;
fn slot(scope: StepScope) -> Slot {
    match scope {
        StepScope::Layer(layer) => Some(layer.0.0),
        _ => None,
    }
}

#[derive(Default)]
pub(super) struct Waiting(BTreeMap<Slot, Request>);

impl Waiting {
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(super) fn clear(&mut self) {
        self.0.clear();
    }

    pub(super) fn retain_current(&mut self, snapshot: &MatrixPlaybackSnapshot) {
        self.0
            .retain(|_, wait| wait.anchor == Anchor::read(snapshot, wait.step.scope));
    }

    pub(super) fn poll(
        &mut self,
        videos: &super::super::video::Playback,
        now: Instant,
        mut observe: impl FnMut(&Arc<VideoControl>) -> Result<TransportSample, String>,
    ) -> Result<(Option<Step>, Option<Instant>), String> {
        // Route replacement under the same Layer/Column must invalidate its queued intent.
        self.0.retain(|_, wait| {
            wait.clocks.iter().all(|clock| {
                videos
                    .active_source(clock.layer, clock.launch, clock.column)
                    .is_some_and(|active| Arc::ptr_eq(active, &clock.control))
            })
        });
        let mut future = None::<Instant>;
        let mut ready = None;
        for (&key, wait) in &self.0 {
            let delay = end_delay(wait.mode, wait.clocks.iter().map(|clock| observe(&clock.control)))?;
            if let Some(delay) = delay {
                if delay.is_zero() {
                    ready = Some(key);
                    break;
                }
                if let Some(at) = now.checked_add(delay) {
                    future = Some(future.map_or(at, |previous| previous.min(at)));
                }
            }
        }
        Ok((
            ready.and_then(|slot| self.0.remove(&slot)).map(|wait| wait.step),
            future,
        ))
    }
}

// No eligible finite clock means no wait. An admitted paused finite clock stays in
// the cohort, and a clock explicitly switched to Loop stops participating.
fn end_delay(
    mode: WaitMode,
    samples: impl Iterator<Item = Result<TransportSample, String>>,
) -> Result<Option<Duration>, String> {
    let mut combined = None::<Duration>;
    let mut has_finite = false;
    let mut paused = false;
    for sample in samples {
        let sample = sample?;
        if !sample.has_finite_end() {
            continue;
        }
        has_finite = true;
        if let Some(delay) = sample.finite_end_delay() {
            combined = Some(combined.map_or(delay, |previous| match mode {
                WaitMode::First => previous.min(delay),
                _ => previous.max(delay),
            }));
        } else {
            paused = true;
        }
    }
    Ok(if !has_finite {
        Some(Duration::ZERO)
    } else if mode == WaitMode::All && paused {
        None
    } else {
        combined
    })
}

impl PlaybackControl {
    pub(in crate::matrix) fn cancel_navigation(&self, scope: StepScope) {
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        if state.navigation_waits.0.remove(&slot(scope)).is_some() {
            self.0.wake_auto_next();
        }
    }

    pub(in crate::matrix) fn wait_for_navigation(
        &self,
        engine: &AppEngine,
        media: MediaId,
        snapshot: &MatrixPlaybackSnapshot,
        step: Step,
        mode: WaitMode,
    ) -> Result<bool, String> {
        let group_view = match step.scope {
            StepScope::Children(group) => Some((group, super::super::view::index(engine, media)?)),
            _ => None,
        };
        let mut state = self.0.state.lock().expect("Matrix playback lock poisoned");
        let mut clocks = Vec::new();
        let mut samples = Vec::new();
        for (layer, column, launch, control) in state.videos.active_clocks() {
            let in_scope = match step.scope {
                StepScope::Layer(target) => layer == target,
                StepScope::Children(_) => group_view
                    .as_ref()
                    .is_some_and(|(group, view)| navigation::column_in_group(view, column, *group)),
                _ => true,
            };
            if !in_scope {
                continue;
            }
            let sample = control.sample()?;
            if sample.has_finite_end() {
                clocks.push(Clock {
                    layer,
                    column,
                    launch,
                    control: control.clone(),
                });
                samples.push(Ok(sample));
            }
        }
        if end_delay(mode, samples.into_iter())?.is_some_and(|delay| delay.is_zero()) {
            return Ok(false);
        }
        state.navigation_waits.0.insert(
            slot(step.scope),
            Request {
                step,
                mode,
                anchor: Anchor::read(snapshot, step.scope),
                clocks,
            },
        );
        self.0.auto_next_interested.store(true, Ordering::Release);
        self.0.wake_auto_next();
        Ok(true)
    }
}
