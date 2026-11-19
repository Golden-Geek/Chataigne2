//! A preallocated export of the managed renderer's post-gain virtual outputs.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{
    AnalysisFrameReader, AnalysisFrameTag, AnalysisFrameWriter, AudioEngineConfig, AudioError, ConfigGeneration,
    EngineLimits, PlanarBuffer, analysis_frame_pool,
};

#[cfg(test)]
mod tests;

/// Owned tap input for the managed audio renderer.
#[derive(Debug)]
pub struct VirtualOutputTapWriter {
    writer: AnalysisFrameWriter,
    enabled: Arc<AtomicBool>,
    anchor: Instant,
    channels: usize,
    frames: usize,
    rate: u32,
}

/// Non-realtime reader. Blocks are borrowed only for the duration of the drain callback.
#[derive(Debug)]
pub struct VirtualOutputTapReader {
    reader: AnalysisFrameReader,
    enabled: Arc<AtomicBool>,
    anchor: Instant,
    frames: usize,
    rate: u32,
}

/// Time and layout of one tightly packed, planar virtual-output block.
#[derive(Debug)]
pub struct VirtualOutputBlock<'a> {
    pub generation: u64,
    pub starts_at: Instant,
    pub sample_rate: u32,
    pub frames: usize,
    pub samples: &'a [f32],
}

/// Reserve an inactive bounded export, using the same realtime-safe recycling as analysis.
pub fn virtual_output_tap(
    config: &AudioEngineConfig,
    limits: &EngineLimits,
    slots: usize,
) -> Result<(VirtualOutputTapWriter, VirtualOutputTapReader), AudioError> {
    let channels = usize::from(limits.max_virtual_outputs);
    let frames = config.internal_block_frames.get() as usize;
    let rate = config.sample_rate.get();
    let samples = frames
        .checked_mul(channels)
        .ok_or_else(|| AudioError::capacity_exceeded("virtual output tap"))?;
    let (reader, writer) = analysis_frame_pool(slots, samples)?;
    let enabled = Arc::new(AtomicBool::new(false));
    let anchor = Instant::now();
    Ok((
        VirtualOutputTapWriter {
            writer,
            enabled: enabled.clone(),
            anchor,
            channels,
            frames,
            rate,
        },
        VirtualOutputTapReader {
            reader,
            enabled,
            anchor,
            frames,
            rate,
        },
    ))
}

impl VirtualOutputTapWriter {
    pub(crate) fn validate(&self, config: &AudioEngineConfig, limits: &EngineLimits) -> Result<(), AudioError> {
        if self.frames != config.internal_block_frames.get() as usize
            || self.rate != config.sample_rate.get()
            || self.channels != usize::from(limits.max_virtual_outputs)
        {
            return Err(AudioError::invalid_configuration(
                "virtual output tap must match managed renderer",
            ));
        }
        Ok(())
    }

    pub(crate) fn capture(&mut self, generation: ConfigGeneration, buffer: &PlanarBuffer, queued_frames: usize) {
        let channels = buffer.channels();
        if channels == 0 || !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let start = self.anchor.elapsed() + Duration::from_secs_f64(queued_frames as f64 / f64::from(self.rate));
        let _ = self.writer.capture_tagged_with(
            AnalysisFrameTag {
                topology_generation: generation.get(),
                // This adapter uses the pool's sequence field for nanoseconds from its anchor.
                render_frame: u64::try_from(start.as_nanos()).unwrap_or(u64::MAX),
                ..Default::default()
            },
            channels * self.frames,
            |destination| {
                for (index, plane) in destination.chunks_exact_mut(self.frames).enumerate() {
                    plane.copy_from_slice(&buffer.channel(index)[..self.frames]);
                }
            },
        );
    }
}

impl VirtualOutputTapReader {
    /// Resume or stop capture without an edit or a hardware stream.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Read up to the requested number of complete blocks, returning storage to the renderer.
    pub fn drain(&mut self, limit: usize, mut visit: impl FnMut(VirtualOutputBlock<'_>)) {
        for _ in 0..limit {
            let Some(frame) = self.reader.try_recv() else { break };
            let tag = frame.tag();
            let starts_at = self.anchor + Duration::from_nanos(tag.render_frame);
            visit(VirtualOutputBlock {
                generation: tag.topology_generation,
                starts_at,
                sample_rate: self.rate,
                frames: self.frames,
                samples: frame.samples(),
            });
            let _ = self.reader.recycle(frame);
        }
    }
}
