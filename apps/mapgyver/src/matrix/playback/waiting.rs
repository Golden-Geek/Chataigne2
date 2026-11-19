//! Hold explicit navigation until the active clocks in scope reach their next boundary.
use super::*;
use crate::{
    matrix::{
        controls::WaitMode,
        view::navigation::{self, Step, StepScope},
    },
    media::video::{TransportSample, VideoControl},
};
use golden_time::TimeEpoch;
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
    pending: Option<(TimeEpoch, i64)>,
}

enum BoundaryDelay {
    Unavailable,
    Paused,
    Moving(Duration),
}

impl Clock {
    fn boundary_delay(&mut self, sample: TransportSample) -> BoundaryDelay {
        let Some((epoch, cycle)) = self.pending else {
            return BoundaryDelay::Moving(Duration::ZERO);
        };
        if sample.duration.is_zero() {
            return BoundaryDelay::Unavailable;
        }
        // A timer can wake after the loop has wrapped. Latch that crossing so All Media End
        // can wait for slower peers. An explicit seek or rate change starts a new epoch.
        let crossed = sample.looping && sample.epoch == epoch && sample.cycle != cycle;
        let delay = if crossed {
            Some(Duration::ZERO)
        } else {
            sample.next_boundary_delay()
        };
        self.pending = (!delay.is_some_and(|delay| delay.is_zero())).then_some((sample.epoch, sample.cycle));
        delay.map_or(BoundaryDelay::Paused, BoundaryDelay::Moving)
    }
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
        for (&key, wait) in &mut self.0 {
            let delay = end_delay(
                wait.mode,
                wait.clocks.iter_mut().map(|clock| {
                    let sample = observe(&clock.control)?;
                    Ok(clock.boundary_delay(sample))
                }),
            )?;
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

// A paused clock keeps its place in All Media End. First Media End can still be
// satisfied by a different clock reaching its boundary.
fn end_delay(
    mode: WaitMode,
    delays: impl Iterator<Item = Result<BoundaryDelay, String>>,
) -> Result<Option<Duration>, String> {
    let mut combined = None::<Duration>;
    let mut paused = false;
    for delay in delays {
        match delay? {
            BoundaryDelay::Unavailable => {}
            BoundaryDelay::Paused => paused = true,
            BoundaryDelay::Moving(delay) => {
                combined = Some(combined.map_or(delay, |previous| match mode {
                    WaitMode::First => previous.min(delay),
                    _ => previous.max(delay),
                }));
            }
        }
    }
    Ok(if paused && mode == WaitMode::All {
        None
    } else if paused {
        combined
    } else {
        Some(combined.unwrap_or_default())
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
        let mut delays = Vec::new();
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
            if !sample.duration.is_zero() {
                let mut clock = Clock {
                    layer,
                    column,
                    launch,
                    control: control.clone(),
                    pending: Some((sample.epoch, sample.cycle)),
                };
                delays.push(Ok(clock.boundary_delay(sample)));
                clocks.push(clock);
            }
        }
        if end_delay(mode, delays.into_iter())?.is_some_and(|delay| delay.is_zero()) {
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
