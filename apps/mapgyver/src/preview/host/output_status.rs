//! One-second output metadata sampling; no GPU queries or engine captures.
use crate::{
    preview::{protocol::ScreenRuntimeStatus, runtime_status::measured_fps},
    screens::protocol::ScreenId,
};
use golden_video::{Extent2D, OutputId, native::OutputSnapshot, runtime::DeviceGeneration};
use std::{collections::HashMap, time::Instant};

#[derive(Default)]
pub(super) struct OutputSampler {
    previous: HashMap<ScreenId, Baseline>,
}
struct Baseline {
    id: OutputId,
    generation: u64,
    device: Option<DeviceGeneration>,
    at: Instant,
    frames: u64,
}
impl OutputSampler {
    pub fn sample(
        &mut self,
        now: Instant,
        routes: &[(ScreenId, OutputId, Extent2D)],
        outputs: &[OutputSnapshot],
    ) -> Vec<ScreenRuntimeStatus> {
        let mut next = HashMap::new();
        let rows = routes
            .iter()
            .filter_map(|(screen, id, extent)| {
                let output = outputs.iter().find(|output| output.id == *id)?;
                let device = output.last_accepted.map(|token| token.device);
                let previous = self
                    .previous
                    .get(screen)
                    .filter(|old| old.id == *id && old.generation == output.generation && old.device == device)
                    .map(|old| (old.at, old.frames));
                let accepted_fps = measured_fps(now, output.accepted_frames, previous);
                next.insert(
                    *screen,
                    Baseline {
                        id: *id,
                        generation: output.generation,
                        device,
                        at: now,
                        frames: output.accepted_frames,
                    },
                );
                Some(ScreenRuntimeStatus {
                    screen: *screen,
                    extent: *extent,
                    accepted_fps,
                    state: format!("{:?}", output.state),
                })
            })
            .collect();
        self.previous = next;
        rows
    }
}

#[cfg(test)]
mod tests;
