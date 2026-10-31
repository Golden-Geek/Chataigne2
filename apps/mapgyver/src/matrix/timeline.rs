//! App-owned clip weights; evaluated only on launch/capture, never once per video frame.
use super::{
    bindings::Bindings,
    protocol::{ColumnId, MatrixPlaybackSnapshot, TransitionCurve},
};
use golden_time::TimeSnapshot;
use golden_video::{
    Opacity,
    plan::{InterpolationCurve, OpacityEnvelope},
};
use std::time::Duration;

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
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Clip {
    pub id: uuid::Uuid,
    pub column: ColumnId,
    pub version: u64,
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
}
#[derive(Default)]
pub(crate) struct Timeline {
    launch: Option<uuid::Uuid>,
    frame: Frame,
    pub bindings: Bindings,
}
impl Timeline {
    pub fn settle(&mut self, time: TimeSnapshot) {
        if self
            .frame
            .interval
            .is_some_and(|interval| interval.envelope(0.0, 1.0).finished(time))
        {
            self.frame.clips.retain(|entry| entry.to > 0.0);
            for entry in &mut self.frame.clips {
                entry.from = entry.to;
            }
            self.frame.interval = None;
            self.bindings.retain(self.frame.clips.iter().map(|entry| entry.clip.id));
        }
    }
    pub fn accepts(&self, column: ColumnId, version: u64) -> bool {
        self.frame.clips.len() < MAX_MIX_COLUMNS
            || self
                .frame
                .clips
                .iter()
                .any(|entry| entry.clip.column == column && entry.clip.version == version)
    }
    pub fn frame(
        &mut self,
        desired: &MatrixPlaybackSnapshot,
        time: Option<TimeSnapshot>,
        version: u64,
    ) -> Result<Option<Frame>, String> {
        if desired.transition.duration_ms != 0 && desired.launch_id.is_some() && time.is_none() {
            return Ok(None);
        }
        if self.launch != desired.launch_id {
            let mut frame = Frame::default();
            if desired.transition.duration_ms != 0 {
                let now = time.ok_or("timed Matrix capture requires shared time")?;
                for entry in &self.frame.clips {
                    let weight = self.frame.interval.map_or(entry.to, |interval| {
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
                let duration = Duration::from_millis(u64::from(desired.transition.duration_ms));
                now.monotonic().checked_add(duration).map_err(|e| e.to_string())?;
                frame.interval = Some(Interval {
                    anchor: now,
                    duration,
                    curve: match desired.transition.curve {
                        TransitionCurve::Linear => InterpolationCurve::Linear,
                        TransitionCurve::SmoothStep => InterpolationCurve::SmoothStep,
                    },
                });
            }
            if let (Some(column), Some(launch)) = (desired.desired_column, desired.launch_id) {
                if let Some(entry) = frame
                    .clips
                    .iter_mut()
                    .find(|entry| entry.clip.column == column && entry.clip.version == version)
                {
                    entry.to = 1.0;
                } else {
                    frame.clips.push(Contribution {
                        clip: Clip {
                            id: launch,
                            column,
                            version,
                        },
                        from: if frame.interval.is_none() { 1.0 } else { 0.0 },
                        to: 1.0,
                    });
                }
            }
            if frame.clips.len() > MAX_MIX_COLUMNS {
                return Err("Matrix interrupted mix exceeds 32 retained clips".into());
            }
            self.frame = frame;
            self.launch = desired.launch_id;
            self.bindings.retain(self.frame.clips.iter().map(|entry| entry.clip.id));
        }
        if let Some(time) = time {
            self.settle(time);
        }
        Ok(Some(self.frame.clone()))
    }
}
