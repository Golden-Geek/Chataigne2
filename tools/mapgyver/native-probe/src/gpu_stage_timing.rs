//! Bounded timestamp boundaries from one renderer context and one disjoint-query scope.
//!
//! GPU ticks and CPU QPC observations are separate clocks. The latter identify submission
//! and observation, not execution timestamps or cross-device critical-path duration.

use crate::statistics::MAX_SAMPLES;
use serde::Serialize;

pub const INTERMEDIATE_COUNT: usize = 5;
pub const BOUNDARY_COUNT: usize = INTERMEDIATE_COUNT + 2;
pub const STAGE_NAMES: [&str; BOUNDARY_COUNT - 1] = [
    "source_handoff",
    "composition",
    "spout_send",
    "native_output",
    "editor",
    "tail",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boundary {
    Sources,
    Composition,
    Spout,
    NativeOutput,
    Editor,
}

impl Boundary {
    pub const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Frame {
    pub frame: u64,
    pub submitted_qpc: u64,
    pub observed_completed_qpc: u64,
    pub frequency: u64,
    pub collected: bool,
    pub boundaries: [u64; BOUNDARY_COUNT],
}

#[derive(Debug, Serialize)]
pub struct Trace {
    pub stage_names: [&'static str; BOUNDARY_COUNT - 1],
    pub rows: Vec<Frame>,
    pub skipped: u32,
    pub invalid: u32,
    pub pending: usize,
    pub truncated: u32,
}

impl Default for Trace {
    fn default() -> Self {
        Self {
            stage_names: STAGE_NAMES,
            rows: Vec::new(),
            skipped: 0,
            invalid: 0,
            pending: 0,
            truncated: 0,
        }
    }
}

impl Trace {
    /// The query owner counts an invalid frame once, including failures before a row exists.
    pub fn record(&mut self, frame: Frame) -> Result<(), String> {
        if frame.frequency == 0 {
            return Err("GPU stage timestamp frequency must be nonzero".into());
        }
        if frame.submitted_qpc == 0 || frame.observed_completed_qpc < frame.submitted_qpc {
            return Err("GPU stage completion observation precedes a valid submission QPC".into());
        }
        if frame.boundaries.windows(2).any(|pair| pair[0] > pair[1]) {
            return Err("GPU stage timestamp boundaries are not monotonic".into());
        }
        if self.rows.len() < MAX_SAMPLES {
            self.rows.push(frame);
        } else {
            self.truncated += 1;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
