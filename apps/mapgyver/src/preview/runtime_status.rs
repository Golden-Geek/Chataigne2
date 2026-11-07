//! One-second metadata sampling on the presentation worker; never an engine poll or GPU query.
use super::protocol::{VideoResourceUsage, VideoRuntimeState, VideoRuntimeStatus};
use golden_video::runtime::{DeviceGeneration, ProjectEpoch, RuntimeSnapshot, RuntimeState};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

impl Default for VideoRuntimeStatus {
    fn default() -> Self {
        Self {
            sequence: 0,
            state: VideoRuntimeState::Starting,
            submitted_fps: None,
            cpu_submit_ms: None,
            gpu: None,
            gpu_adapter: None,
            gpu_preference: None,
            gpu_restart_required: false,
            resources: None,
            error: None,
            screens: Vec::new(),
        }
    }
}
pub(crate) struct StatusView(Mutex<(Instant, VideoRuntimeStatus)>);
impl Default for StatusView {
    fn default() -> Self {
        Self(Mutex::new((Instant::now(), VideoRuntimeStatus::default())))
    }
}
impl StatusView {
    pub fn read(&self) -> VideoRuntimeStatus {
        self.read_at(Instant::now())
    }
    fn read_at(&self, now: Instant) -> VideoRuntimeStatus {
        let (at, status) = &*self.0.lock().expect("video status lock poisoned");
        if now.saturating_duration_since(*at) > Duration::from_secs(3) && status.state != VideoRuntimeState::Stopped {
            VideoRuntimeStatus {
                sequence: status.sequence,
                state: VideoRuntimeState::Faulted,
                error: Some("Video runtime observations expired".into()),
                ..Default::default()
            }
        } else {
            status.clone()
        }
    }
    pub fn publish(&self, value: VideoRuntimeStatus) {
        *self.0.lock().expect("video status lock poisoned") = (Instant::now(), value);
    }
}

pub(crate) fn measured_fps(now: Instant, count: u64, previous: Option<(Instant, u64)>) -> Option<f64> {
    let (at, frames) = previous?;
    let elapsed = now.checked_duration_since(at)?.as_secs_f64();
    (elapsed > 0.0)
        .then(|| count.checked_sub(frames).map(|delta| delta as f64 / elapsed))
        .flatten()
}
#[derive(Default)]
pub(crate) struct StatusSampler {
    sequence: u64,
    sampled: Option<Instant>,
    counter: Option<(ProjectEpoch, Option<DeviceGeneration>, Instant, u64)>,
}
impl StatusSampler {
    pub fn due(&self, now: Instant) -> bool {
        self.sampled
            .is_none_or(|at| now.saturating_duration_since(at) >= Duration::from_secs(1))
    }
    pub fn sample(
        &mut self,
        now: Instant,
        runtime: Option<(&RuntimeSnapshot, &str)>,
        error: Option<&str>,
    ) -> Option<VideoRuntimeStatus> {
        if !self.due(now) {
            return None;
        }
        self.sampled = Some(now);
        self.sequence = self.sequence.saturating_add(1);
        let mut status = VideoRuntimeStatus {
            sequence: self.sequence,
            state: VideoRuntimeState::Idle,
            error: error.map(str::to_owned),
            ..Default::default()
        };
        if let Some((snapshot, gpu)) = runtime {
            let device = snapshot.resources.map(|value| value.last_submitted.device);
            status.state = match snapshot.state {
                RuntimeState::Running => VideoRuntimeState::Running,
                RuntimeState::Stopped | RuntimeState::Stopping => VideoRuntimeState::Stopped,
                RuntimeState::Faulted => VideoRuntimeState::Faulted,
                _ => VideoRuntimeState::Starting,
            };
            let recovering = snapshot.resources.is_some_and(|resources| resources.device_lost);
            if recovering {
                status.state = VideoRuntimeState::Recovering;
            }
            if status.state == VideoRuntimeState::Running {
                let previous = self
                    .counter
                    .filter(|(project, old_device, _, _)| *project == snapshot.project && *old_device == device)
                    .map(|(_, _, at, count)| (at, count));
                status.submitted_fps = measured_fps(now, snapshot.frames_submitted, previous);
                status.cpu_submit_ms = snapshot
                    .last_frame
                    .filter(|frame| {
                        frame.identity.project == snapshot.project && Some(frame.completion.device) == device
                    })
                    .map(|frame| frame.submit_duration.as_secs_f64() * 1000.0);
                self.counter = Some((snapshot.project, device, now, snapshot.frames_submitted));
            } else {
                self.counter = None;
            }
            status.gpu = Some(gpu.to_owned());
            status.resources = snapshot.resources.map(|value| VideoResourceUsage {
                used_bytes: value.bytes_used,
                limit_bytes: value.byte_limit,
                in_flight: value.in_flight,
                retiring: value.retiring_generations,
            });
            if status.state != VideoRuntimeState::Running {
                let runtime_error = snapshot.failure.as_ref().map(ToString::to_string);
                // A lost device invalidates captured source observations. Report its owning
                // failure before the resulting stale-plan error while recovery is pending.
                status.error = if recovering {
                    runtime_error.or(status.error)
                } else {
                    status.error.or(runtime_error)
                };
            }
        } else {
            self.counter = None;
            if error.is_some() {
                status.state = VideoRuntimeState::Faulted;
            }
        }
        Some(status)
    }
}

#[cfg(test)]
mod tests;
