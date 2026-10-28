//! Bounded dependency and per-context timing evidence; GPU clocks are never joined directly.

use crate::statistics::MAX_SAMPLES;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct GpuSpan {
    pub frame: u64,
    pub submitted_qpc: u64,
    pub observed_completed_qpc: u64,
    pub start_ticks: u64,
    pub end_ticks: u64,
    pub frequency: u64,
    pub collected: bool,
}

#[derive(Default, Serialize)]
pub struct GpuTrace {
    pub spans: Vec<GpuSpan>,
    pub skipped: u32,
    pub invalid: u32,
    pub pending: usize,
    pub truncated: u32,
}

impl GpuTrace {
    pub fn record(&mut self, span: GpuSpan) {
        if self.spans.len() < MAX_SAMPLES {
            self.spans.push(span);
        } else {
            self.truncated += 1;
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Dependencies {
    pub renderer_frame: u64,
    /// Exact receive-batch IDs of the two retained bridge readers, or no source at draw time.
    pub sources: [Option<u64>; 2],
}

#[derive(Serialize)]
pub struct DependencyTrace {
    pub qpc_frequency: u64,
    pub frames: Vec<Dependencies>,
    pub truncated: u32,
}

impl DependencyTrace {
    pub fn new(qpc_frequency: u64) -> Self {
        Self {
            qpc_frequency,
            frames: Vec::new(),
            truncated: 0,
        }
    }

    pub fn record(&mut self, frame: Dependencies) {
        if self.frames.len() < MAX_SAMPLES {
            self.frames.push(frame);
        } else {
            self.truncated += 1;
        }
    }
}

#[cfg(test)]
mod tests;
