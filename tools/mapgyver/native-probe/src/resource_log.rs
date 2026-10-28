//! Process-owned logical texture observations. Imported fixture allocations are counted by
//! their allocating process only; DXGI process usage is separate from descriptor payload bytes.

use crate::allocation::{Operations, Snapshot};
use serde::{Deserialize, Serialize};

pub const MAX_SAMPLES: usize = 3620;
pub const CHUNK_SAMPLES: usize = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryInfo {
    pub budget_bytes: u64,
    pub current_usage_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub elapsed_seconds: f64,
    pub source_phase: Option<u32>,
    pub allocations: Snapshot,
    pub gpu_local_nonlocal: [Option<MemoryInfo>; 2],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub before_resource_drop: Snapshot,
    pub after_resource_drop: Snapshot,
    pub after_fence: Snapshot,
    pub operations: Operations,
    pub completion_markers: u32,
    pub skipped_completion_markers: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<Sample>,
    pub sample_count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "resource_event", rename_all = "snake_case")]
pub enum Message {
    Samples { offset: usize, samples: Vec<Sample> },
    Finished { report: Box<Report> },
}

#[derive(Default)]
pub struct Stream {
    samples: Vec<Sample>,
    completed: Option<Report>,
    ended: bool,
}

impl Stream {
    pub fn accept(&mut self, message: Message) -> Result<(), String> {
        if self.ended {
            return Err("resource stream continued after completion".into());
        }
        match message {
            Message::Samples { offset, samples } => {
                if offset != self.samples.len()
                    || samples.is_empty()
                    || samples.len() > CHUNK_SAMPLES
                    || offset + samples.len() > MAX_SAMPLES
                {
                    return Err("invalid resource chunk sequence/capacity".into());
                }
                let mut previous = self.samples.last().map_or(0.0, |s| s.elapsed_seconds);
                for sample in &samples {
                    if !sample.elapsed_seconds.is_finite()
                        || sample.elapsed_seconds < previous
                        || sample.source_phase.is_some_and(|phase| phase > 3)
                    {
                        return Err("invalid resource sample time/phase".into());
                    }
                    previous = sample.elapsed_seconds;
                }
                self.samples.extend(samples);
            }
            Message::Finished { mut report } => {
                if !report.samples.is_empty()
                    || report.sample_count != self.samples.len()
                    || report.sample_count == 0
                    || report.after_fence.live_textures != 0
                    || report.after_fence.retiring_textures != 0
                    || report.after_fence.live_bytes_by_owner != [0; 4]
                    || report.after_fence.retiring_bytes_by_owner != [0; 4]
                {
                    return Err("resource completion omitted samples or retained allocations".into());
                }
                report.samples = std::mem::take(&mut self.samples);
                self.completed = Some(*report);
                self.ended = true;
            }
        }
        Ok(())
    }

    pub fn finish(&mut self) -> Result<Report, String> {
        self.completed
            .take()
            .ok_or_else(|| "resource completion missing".into())
    }
}

pub fn messages(mut report: Report) -> impl Iterator<Item = Message> {
    let samples = std::mem::take(&mut report.samples);
    let chunks: Vec<_> = samples
        .chunks(CHUNK_SAMPLES)
        .enumerate()
        .map(|(index, chunk)| Message::Samples {
            offset: index * CHUNK_SAMPLES,
            samples: chunk.to_vec(),
        })
        .collect();
    chunks.into_iter().chain(std::iter::once(Message::Finished {
        report: Box::new(report),
    }))
}

#[cfg(test)]
mod tests;
