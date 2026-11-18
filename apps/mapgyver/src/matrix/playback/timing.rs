//! Deadline-only continuation of the same captured Matrix timeline, never a polling clock.
use super::State;
use golden_time::TimeSnapshot;
use std::time::{Duration, Instant};

pub(super) struct Observation {
    snapshot: TimeSnapshot,
    observed: Instant,
}
impl Observation {
    fn project(&self, now: Instant) -> Result<TimeSnapshot, String> {
        self.snapshot
            .monotonic()
            .checked_add(now.saturating_duration_since(self.observed))
            .and_then(|time| self.snapshot.at_monotonic(time))
            .map_err(|error| error.to_string())
    }
}
impl State {
    pub(super) fn observe_video_time(&mut self, snapshot: TimeSnapshot) {
        self.video_time = Some(Observation {
            snapshot,
            observed: Instant::now(),
        });
    }
    pub(super) fn cancel_video_deadline(&mut self) {
        if self.video_deadline_at.take().is_some()
            && let Some(deadline) = &self.video_deadline
        {
            deadline.cancel_deadline();
        }
    }
    pub(super) fn advance_videos(&mut self) {
        if !self.video_enabled {
            self.cancel_video_deadline();
            return;
        }
        if self.videos.next_deadline().is_none() && self.layer_stop_deadline().is_none() {
            self.schedule_video_deadline();
            return;
        }
        let now = Instant::now();
        if let Some(observation) = &self.video_time {
            let result = observation.project(now).and_then(|time| {
                self.timeline.frame(
                    &self.current,
                    Some(time),
                    self.version,
                    &self.epochs,
                    &self.launch_orders,
                    &self.cuts,
                )
            });
            match result {
                Ok(Some(frame)) => {
                    self.cuts = Default::default();
                    self.videos.retain_frame(&frame);
                }
                Ok(None) => {}
                Err(error) => self.video_error = Some(error),
            }
        } else {
            // No frame has ever consumed these uses. Keep the configured headless interval;
            // a first capture contains only its current incoming clips, not hidden history.
            self.videos.finish_due(now);
        }
        self.schedule_video_deadline();
    }
    pub(super) fn schedule_video_deadline(&mut self) {
        if !self.video_enabled {
            self.cancel_video_deadline();
            return;
        }
        let now = Instant::now();
        let video_delay = if self.videos.next_deadline().is_none() && self.layer_stop_deadline().is_none() {
            None
        } else if let Some(observation) = &self.video_time {
            let Ok(time) = observation.project(now) else { return };
            self.timeline
                .next_video_interval()
                .map(|interval| {
                    let end = interval.anchor.monotonic().as_nanos() as u128 + interval.duration.as_nanos();
                    Duration::from_nanos(end.saturating_sub(time.monotonic().as_nanos() as u128) as u64)
                })
                .or_else(|| self.layer_stop_deadline().map(|at| at.saturating_duration_since(now)))
        } else {
            self.videos
                .next_deadline()
                .into_iter()
                .chain(self.layer_stop_deadline())
                .min()
                .map(|at| at.saturating_duration_since(now))
        };
        let at = video_delay
            .and_then(|delay| now.checked_add(delay))
            .into_iter()
            .chain(self.auto_next.deadline)
            .min();
        if self.video_deadline_at != at {
            self.cancel_video_deadline();
            if let (Some(deadline), Some(at)) = (&self.video_deadline, at) {
                deadline.wake_after(at.saturating_duration_since(Instant::now()));
                self.video_deadline_at = Some(at);
            }
        }
    }
}
