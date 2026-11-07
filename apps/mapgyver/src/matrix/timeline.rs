//! Shared-time column and independent Layer mixes; one bounded source cache serves every mix.
use super::{bindings::Bindings, protocol::*};
use golden_time::TimeSnapshot;
use golden_video::{
    Opacity,
    plan::{InterpolationCurve, OpacityEnvelope},
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

pub(super) const MAX_MIX_COLUMNS: usize = 32;
#[derive(Clone, Copy)]
pub(crate) struct Interval {
    pub anchor: TimeSnapshot,
    pub duration: Duration,
    pub curve: InterpolationCurve,
}
impl Interval {
    pub fn envelope(self, from: f32, to: f32) -> OpacityEnvelope {
        OpacityEnvelope::new(
            self.anchor,
            self.duration,
            Opacity::new(from.clamp(0.0, 1.0)).unwrap(),
            Opacity::new(to.clamp(0.0, 1.0)).unwrap(),
            self.curve,
        )
        .expect("validated Matrix interval")
    }
    fn end(self) -> u64 {
        self.anchor
            .monotonic()
            .checked_add(self.duration)
            .expect("validated interval")
            .as_nanos()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Clip {
    pub id: uuid::Uuid,
    pub column: ColumnId,
    pub version: u64,
    pub individual: bool,
    pub order: u64,
}
struct Launch {
    column: Option<ColumnId>,
    id: Option<uuid::Uuid>,
    transition: TransitionDefinition,
    version: u64,
    individual: bool,
    order: u64,
    suppressed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Contribution {
    pub clip: Clip,
    pub from: f32,
    pub to: f32,
}
#[derive(Clone, Default)]
pub(crate) struct Frame {
    pub clips: Vec<Contribution>,
    pub interval: Option<Interval>,
    pub layers: Arc<HashMap<LayerId, Arc<Frame>>>,
    pub stamps: Arc<[MatrixLayerPlanStamp]>,
    pub next: Option<Interval>,
    pub generation: u64,
    target: Option<uuid::Uuid>,
}
impl Frame {
    pub fn layer(&self, layer: LayerId) -> &Frame {
        self.layers.get(&layer).map_or(self, Arc::as_ref)
    }
    pub fn columns(&self) -> Vec<ColumnId> {
        let mut columns = Vec::new();
        for entry in &self.clips {
            if !columns.contains(&entry.clip.column) {
                columns.push(entry.clip.column);
            }
        }
        columns
    }
    fn flat(&self) -> Self {
        Self {
            clips: self.clips.clone(),
            interval: self.interval,
            target: self.target,
            ..Default::default()
        }
    }
    fn accepts(&self, column: ColumnId, version: u64, individual: bool) -> bool {
        self.clips.len() < MAX_MIX_COLUMNS
            || self.clips.iter().any(|entry| {
                entry.clip.column == column && entry.clip.version == version && entry.clip.individual == individual
            })
    }
    fn changed(&self, request: Launch, time: Option<TimeSnapshot>) -> Result<Self, String> {
        let Launch {
            column,
            id: launch,
            transition,
            version,
            individual,
            order,
            suppressed,
        } = request;
        if self.target == launch {
            return Ok(self.flat());
        }
        if suppressed {
            let mut frame = self.flat();
            frame.target = launch;
            return Ok(frame);
        }
        let mut frame = Self {
            target: launch,
            ..Default::default()
        };
        if transition.duration_ms != 0 {
            let now = time.ok_or("timed Matrix capture requires shared time")?;
            for entry in &self.clips {
                let weight = self.interval.map_or(entry.to, |interval| {
                    interval.envelope(entry.from, entry.to).sample(now).get()
                });
                if weight > 0.0 {
                    frame.clips.push(Contribution {
                        clip: entry.clip,
                        from: weight,
                        to: 0.0,
                    });
                }
            }
            let duration = Duration::from_millis(u64::from(transition.duration_ms));
            now.monotonic().checked_add(duration).map_err(|e| e.to_string())?;
            frame.interval = Some(Interval {
                anchor: now,
                duration,
                curve: match transition.curve {
                    TransitionCurve::Linear => InterpolationCurve::Linear,
                    TransitionCurve::SmoothStep => InterpolationCurve::SmoothStep,
                },
            });
        }
        if let (Some(column), Some(launch)) = (column, launch) {
            if let Some(entry) = frame.clips.iter_mut().find(|entry| {
                entry.clip.column == column && entry.clip.version == version && entry.clip.individual == individual
            }) {
                entry.to = 1.0;
                entry.clip.order = order;
            } else {
                frame.clips.push(Contribution {
                    clip: Clip {
                        id: launch,
                        column,
                        version,
                        individual,
                        order,
                    },
                    from: if frame.interval.is_none() { 1.0 } else { 0.0 },
                    to: 1.0,
                });
            }
        }
        if frame.clips.len() > MAX_MIX_COLUMNS {
            return Err("Matrix interrupted mix exceeds 32 retained clips per Layer".into());
        }
        Ok(frame)
    }
    fn settle_mix(&mut self, time: TimeSnapshot) {
        if self
            .interval
            .is_some_and(|interval| interval.envelope(0.0, 1.0).finished(time))
        {
            self.clips.retain(|entry| entry.to > 0.0);
            for entry in &mut self.clips {
                entry.from = entry.to;
            }
            self.interval = None;
        }
    }
    fn cut_mix(&mut self, cuts: &super::stops::Cuts, layer: Option<LayerId>) {
        self.clips.retain(|entry| !cuts.removes(layer, entry.clip));
        if self.clips.is_empty() {
            self.interval = None;
        }
    }
    fn cut(&mut self, cuts: &super::stops::Cuts) {
        if cuts.is_empty() {
            return;
        }
        self.cut_mix(cuts, None);
        let base = self.flat();
        let layers = Arc::make_mut(&mut self.layers);
        for layer in cuts.layers() {
            layers.entry(layer).or_insert_with(|| Arc::new(base.clone()));
        }
        for (layer, frame) in layers {
            Arc::make_mut(frame).cut_mix(cuts, Some(*layer));
        }
        self.refresh();
    }
    fn refresh(&mut self) {
        self.generation = self.generation.checked_add(1).expect("Matrix frame revision exhausted");
        self.next = self
            .interval
            .into_iter()
            .chain(self.layers.values().filter_map(|frame| frame.interval))
            .min_by_key(|interval| interval.end());
        let mut stamps = self
            .layers
            .iter()
            .map(|(layer, frame)| MatrixLayerPlanStamp {
                layer: *layer,
                columns: frame.columns(),
                transitioning: frame.interval.is_some(),
            })
            .collect::<Vec<_>>();
        stamps.sort_by_key(|stamp| stamp.layer.0.0);
        self.stamps = stamps.into();
    }
}
#[derive(Default)]
pub(crate) struct Timeline {
    request: Option<uuid::Uuid>,
    frame: Frame,
    explicit: HashSet<LayerId>,
    pub bindings: Bindings,
    pub applications: super::applications::Applications,
}
impl Timeline {
    pub(super) fn has_playable(&self, layer: LayerId, columns: &HashSet<(LayerId, ColumnId)>) -> bool {
        self.frame
            .layer(layer)
            .clips
            .iter()
            .any(|entry| columns.contains(&(layer, entry.clip.column)))
    }
    pub(super) fn columns(&self) -> HashSet<ColumnId> {
        self.frame
            .clips
            .iter()
            .chain(self.frame.layers.values().flat_map(|frame| frame.clips.iter()))
            .map(|entry| entry.clip.column)
            .collect()
    }
    pub fn settle(&mut self, time: TimeSnapshot) {
        if self
            .frame
            .next
            .is_none_or(|interval| !interval.envelope(0.0, 1.0).finished(time))
        {
            return;
        }
        self.frame.settle_mix(time);
        let base = self.frame.clips.clone();
        let base_interval = self.frame.interval.is_some();
        let layers = Arc::make_mut(&mut self.frame.layers);
        layers.retain(|layer, frame| {
            Arc::make_mut(frame).settle_mix(time);
            self.explicit.contains(layer) || base_interval || frame.interval.is_some() || frame.clips != base
        });
        self.frame.refresh();
        self.bindings.retain_frame(&self.frame);
        self.applications.retain(&self.frame);
    }
    fn filtered(&self, cuts: &super::stops::Cuts) -> std::borrow::Cow<'_, Frame> {
        if cuts.is_empty() {
            return std::borrow::Cow::Borrowed(&self.frame);
        }
        let mut frame = self.frame.clone();
        frame.cut(cuts);
        std::borrow::Cow::Owned(frame)
    }
    pub(super) fn accepts_request(
        &self,
        request: &MatrixPlaybackSnapshot,
        version: u64,
        cuts: &super::stops::Cuts,
    ) -> bool {
        let frame = self.filtered(cuts);
        let Some(column) = request.desired_column else {
            return true;
        };
        if request.transition.duration_ms != 0 && !frame.accepts(column, version, false) {
            return false;
        }
        let layers = request
            .layers
            .iter()
            .map(|request| (request.layer, request))
            .collect::<HashMap<_, _>>();
        for (layer, frame) in frame.layers.iter() {
            if !layers.contains_key(layer)
                && request.transition.duration_ms != 0
                && !frame.accepts(column, version, false)
            {
                return false;
            }
        }
        request.layers.iter().all(|request| {
            let frame = frame.layer(request.layer);
            request.transition.duration_ms == 0
                || frame.target == request.launch_id
                || request
                    .column
                    .is_none_or(|column| frame.accepts(column, version, request.individual))
        })
    }
    pub(super) fn accepts_cell(
        &self,
        layer: LayerId,
        column: ColumnId,
        version: u64,
        cuts: &super::stops::Cuts,
    ) -> bool {
        self.filtered(cuts).layer(layer).accepts(column, version, true)
    }
    pub(super) fn frame(
        &mut self,
        desired: &MatrixPlaybackSnapshot,
        time: Option<TimeSnapshot>,
        version: u64,
        epochs: &HashMap<uuid::Uuid, u64>,
        orders: &HashMap<uuid::Uuid, u64>,
        cuts: &super::stops::Cuts,
    ) -> Result<Option<Frame>, String> {
        if time.is_none()
            && ((desired.transition.duration_ms != 0 && desired.launch_id.is_some())
                || desired
                    .layers
                    .iter()
                    .any(|request| request.transition.duration_ms != 0 && request.launch_id.is_some()))
        {
            return Ok(None);
        }
        // Remove stopped historical clips before sampling a newer launch's starting weights.
        self.frame.cut(cuts);
        if let Some(time) = time {
            self.settle(time);
        }
        if self.request != desired.request_id {
            let old = self.frame.flat();
            let column_changed = self.frame.target != desired.launch_id;
            let mut frame = if column_changed {
                old.changed(
                    Launch {
                        column: desired.desired_column,
                        id: desired.launch_id,
                        transition: desired.transition,
                        version,
                        individual: false,
                        order: desired.launch_id.and_then(|id| orders.get(&id)).copied().unwrap_or(0),
                        suppressed: cuts.suppresses(None, desired.launch_id),
                    },
                    time,
                )?
            } else {
                old.clone()
            };
            frame.generation = self.frame.generation;
            let mut layers = (*self.frame.layers).clone();
            let requests = desired
                .layers
                .iter()
                .map(|request| (request.layer, request))
                .collect::<HashMap<_, _>>();
            if column_changed {
                for (layer, current) in &mut layers {
                    if !requests.contains_key(layer) {
                        *current = Arc::new(current.changed(
                            Launch {
                                column: desired.desired_column,
                                id: desired.launch_id,
                                transition: desired.transition,
                                version,
                                individual: false,
                                order: desired.launch_id.and_then(|id| orders.get(&id)).copied().unwrap_or(0),
                                suppressed: cuts.suppresses(None, desired.launch_id),
                            },
                            time,
                        )?);
                    }
                }
            }
            for request in desired.layers.iter() {
                let current = layers.get(&request.layer).map_or(&old, Arc::as_ref);
                let epoch = request
                    .launch_id
                    .and_then(|id| epochs.get(&id))
                    .copied()
                    .unwrap_or(version);
                layers.insert(
                    request.layer,
                    Arc::new(current.changed(
                        Launch {
                            column: request.column,
                            id: request.launch_id,
                            transition: request.transition,
                            version: epoch,
                            individual: request.individual,
                            order: request.launch_id.and_then(|id| orders.get(&id)).copied().unwrap_or(0),
                            suppressed: cuts.suppresses(Some(request.layer), request.launch_id),
                        },
                        time,
                    )?),
                );
            }
            frame.layers = Arc::new(layers);
            // A stopped request may not have been captured before this stop was committed.
            frame.cut(cuts);
            frame.refresh();
            self.frame = frame;
            self.explicit = requests.keys().copied().collect();
            self.request = desired.request_id;
            self.bindings.retain_frame(&self.frame);
            self.applications.retain(&self.frame);
        }
        Ok(Some(self.frame.clone()))
    }
}
