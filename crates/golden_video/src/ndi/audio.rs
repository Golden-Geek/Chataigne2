use std::num::NonZeroU32;
use std::ptr::NonNull;

use ndi_sdk::sys;

use super::{NdiError, NdiTiming, Runtime};

const MAX_AUDIO_CHANNELS: usize = 256;
const MAX_AUDIO_BYTES: usize = 4 * 1024 * 1024;

/// One owned, tightly packed planar block. The samples use NDI's native float level convention.
#[derive(Debug)]
pub struct NdiAudioFrame {
    sample_rate: NonZeroU32,
    frames: usize,
    samples: Box<[f32]>,
    timing: NdiTiming,
}

impl NdiAudioFrame {
    /// Source sample rate in Hz.
    pub fn sample_rate(&self) -> NonZeroU32 {
        self.sample_rate
    }

    /// Samples per channel.
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Number of discrete channels in this packet.
    pub fn channels(&self) -> usize {
        self.samples.len() / self.frames
    }

    /// One source channel in SDK order. Invalid indices return no samples.
    pub fn plane(&self, channel: usize) -> Option<&[f32]> {
        let start = channel.checked_mul(self.frames)?;
        self.samples.get(start..start.checked_add(self.frames)?)
    }

    /// Original timing for the first sample in this packet.
    pub fn timing(&self) -> NdiTiming {
        self.timing
    }
}

pub(super) struct CapturedAudio {
    pub instance: NonNull<sys::NDIlib_recv_instance_type>,
    pub runtime: &'static Runtime,
    pub frame: sys::NDIlib_audio_frame_v3_t,
}

impl CapturedAudio {
    pub fn copy_frame(&self) -> Result<NdiAudioFrame, NdiError> {
        let frame = &self.frame;
        if frame.FourCC != sys::NDIlib_FourCC_audio_type_e::NDIlib_FourCC_audio_type_FLTP {
            return Err(NdiError::AudioFrame("expected planar float samples"));
        }
        let sample_rate = u32::try_from(frame.sample_rate)
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or(NdiError::AudioFrame("invalid sample rate"))?;
        let channels = frame.no_channels.max(0) as usize;
        let frames = frame.no_samples.max(0) as usize;
        if channels == 0 || channels > MAX_AUDIO_CHANNELS || frames == 0 || frame.p_data.is_null() {
            return Err(NdiError::AudioFrame("invalid sample layout"));
        }
        let plane_bytes = frames * size_of::<f32>();
        if plane_bytes * channels > MAX_AUDIO_BYTES {
            return Err(NdiError::AudioFrame("packet exceeds the 4 MiB sample limit"));
        }
        let sdk_stride = unsafe { frame.__bindgen_anon_1.channel_stride_in_bytes };
        let stride = if channels == 1 {
            plane_bytes
        } else {
            usize::try_from(sdk_stride).map_err(|_| NdiError::AudioFrame("negative channel stride"))?
        };
        let extent = (channels - 1) * stride + plane_bytes;
        if stride < plane_bytes || extent > isize::MAX as usize {
            return Err(NdiError::AudioFrame("invalid channel stride"));
        }
        let mut samples = Vec::with_capacity(channels * frames);
        for channel in 0..channels {
            // The SDK retains each plane until Drop. Read bytes so the incoming stride need
            // not imply Rust's f32 alignment.
            let payload = unsafe { std::slice::from_raw_parts(frame.p_data.add(channel * stride), plane_bytes) };
            samples.extend(payload.chunks_exact(size_of::<f32>()).map(|sample| {
                let value = f32::from_ne_bytes(sample.try_into().unwrap());
                if value.is_finite() { value } else { 0.0 }
            }));
        }
        Ok(NdiAudioFrame {
            sample_rate,
            frames,
            samples: samples.into_boxed_slice(),
            timing: NdiTiming::new(frame.timecode, frame.timestamp),
        })
    }
}

impl Drop for CapturedAudio {
    fn drop(&mut self) {
        unsafe { self.runtime.api.recv_free_audio_v3.unwrap()(self.instance.as_ptr(), &self.frame) };
    }
}
