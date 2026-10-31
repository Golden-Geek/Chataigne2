//! App-owned column weights; evaluated only on launch/capture, never once per video frame.
use super::protocol::{ColumnId, MatrixPlaybackSnapshot, TransitionCurve};
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
#[derive(Clone, Default)]
pub(crate) struct Frame {
    pub columns: Vec<(ColumnId, f32, f32)>,
    pub interval: Option<Interval>,
}
#[derive(Default)]
pub(super) struct Timeline {
    launch: Option<uuid::Uuid>,
    frame: Frame,
}
impl Timeline {
    pub fn settle(&mut self, time: TimeSnapshot) {
        if self
            .frame
            .interval
            .is_some_and(|interval| interval.envelope(0.0, 1.0).finished(time))
        {
            self.frame.columns.retain(|(_, _, to)| *to > 0.0);
            for (_, from, to) in &mut self.frame.columns {
                *from = *to;
            }
            self.frame.interval = None;
        }
    }
    pub fn accepts(&self, column: ColumnId) -> bool {
        self.frame.columns.len() < MAX_MIX_COLUMNS || self.frame.columns.iter().any(|(id, _, _)| *id == column)
    }
    pub fn frame(
        &mut self,
        desired: &MatrixPlaybackSnapshot,
        time: Option<TimeSnapshot>,
    ) -> Result<Option<Frame>, String> {
        if desired.transition.duration_ms != 0 && desired.launch_id.is_some() && time.is_none() {
            return Ok(None); // A topology-only capture cannot invent a time authority.
        }
        if self.launch != desired.launch_id {
            let mut frame = Frame::default();
            if desired.transition.duration_ms != 0 {
                let now = time.ok_or("timed Matrix capture requires shared time")?;
                for (column, from, to) in &self.frame.columns {
                    let weight = self
                        .frame
                        .interval
                        .map_or(*to, |interval| interval.envelope(*from, *to).sample(now).get());
                    if weight > 0.0 {
                        frame.columns.push((*column, weight, 0.0));
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
            if let Some(column) = desired.desired_column {
                if let Some((_, _, to)) = frame.columns.iter_mut().find(|(id, _, _)| *id == column) {
                    *to = 1.0;
                } else {
                    frame
                        .columns
                        .push((column, if frame.interval.is_none() { 1.0 } else { 0.0 }, 1.0));
                }
            }
            if frame.columns.len() > MAX_MIX_COLUMNS {
                return Err("Matrix interrupted mix exceeds 32 retained columns".into());
            }
            self.frame = frame;
            self.launch = desired.launch_id;
        }
        if let Some(time) = time {
            self.settle(time);
        }
        Ok(Some(self.frame.clone()))
    }
}
