//! Fixture CPU-stamp to independent-consumer GPU-copy completion, never a scanout timestamp.

use crate::{
    sample_window::Window,
    statistics::{Distribution, Series},
};
use serde::{Deserialize, Serialize};

pub const WORDS_PER_CHANNEL: usize = 5;
pub const PIXELS_PER_SAMPLE: usize = WORDS_PER_CHANNEL * 2;
pub const CHUNK_SAMPLES: usize = 12;
const MAGIC: u32 = 0xa671_c300;
const BACKGROUND: u32 = 0xff0d_0d0d;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub submitted_qpc: u64,
    pub completed_qpc: u64,
    pub words: [[u32; WORDS_PER_CHANNEL]; 2],
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub valid: usize,
    pub absent: usize,
    pub invalid: usize,
    pub repeated: usize,
    pub unique_frames: usize,
    pub skipped_source_frame_ids: u64,
    pub frame_age: Option<Distribution>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    pub config: Window,
    pub captured_samples: usize,
    pub marker_copy_pixels: usize,
    pub final_log_copy_bytes: usize,
    pub log_readbacks: u32,
    pub no_new_frame: u32,
    pub ring_full: u32,
    pub late_sampling_ticks: u32,
    pub channels: [Channel; 2],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub samples: Vec<Sample>,
}

/// Packed RGBA words; native BGRA staging must reorder bytes before calling this decoder.
pub fn marker_words(frame: u32, channel: u32, qpc: u64) -> [u32; WORDS_PER_CHANNEL] {
    let low = qpc as u32;
    let high = (qpc >> 32) as u32;
    let magic = MAGIC | channel;
    [
        frame,
        magic,
        low,
        high,
        frame.rotate_left(7) ^ low ^ high.rotate_left(13) ^ magic ^ 0x9e37_79b9,
    ]
}

/// Validate raw evidence instead of silently clamping future, corrupt or regressing markers.
pub fn analyze(config: Window, samples: &[Sample]) -> Result<[Channel; 2], String> {
    config.validate()?;
    if samples.len() > config.capacity() {
        return Err("frame-age sample capacity exceeded".into());
    }
    let mut channels: [Channel; 2] = Default::default();
    let mut series: [Series; 2] = Default::default();
    let mut previous: [Option<(u32, u64)>; 2] = [None, None];
    let mut previous_completed = 0;
    for sample in samples {
        if !config.collecting(sample.submitted_qpc)
            || sample.completed_qpc < sample.submitted_qpc
            || sample.completed_qpc < previous_completed
        {
            return Err("invalid frame-age capture clock/order".into());
        }
        previous_completed = sample.completed_qpc;
        for channel in 0..2 {
            let words = sample.words[channel];
            let report = &mut channels[channel];
            if channel == 0 && words == [BACKGROUND; WORDS_PER_CHANNEL] {
                report.absent += 1;
                continue;
            }
            let frame = words[0];
            let qpc = u64::from(words[2]) | (u64::from(words[3]) << 32);
            if words != marker_words(frame, channel as u32, qpc)
                || qpc == 0
                || qpc > sample.completed_qpc
                || previous[channel]
                    .is_some_and(|(prior, stamp)| frame < prior || qpc < stamp || (frame == prior && qpc != stamp))
            {
                report.invalid += 1;
                continue;
            }
            if let Some((prior, _)) = previous[channel] {
                if frame == prior {
                    report.repeated += 1;
                } else {
                    report.unique_frames += 1;
                    report.skipped_source_frame_ids += u64::from(frame - prior - 1);
                }
            } else {
                report.unique_frames += 1;
            }
            previous[channel] = Some((frame, qpc));
            report.valid += 1;
            series[channel].push((sample.completed_qpc - qpc) as f64 * 1000.0 / config.frequency as f64);
        }
    }
    for channel in 0..2 {
        channels[channel].frame_age = series[channel].distribution();
    }
    Ok(channels)
}

#[cfg(test)]
mod tests;
