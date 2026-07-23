use serde::{Deserialize, Serialize};

use crate::{
    AudioBufferPolicy, AudioDeviceDescriptor, AudioDirection, AudioError, AudioErrorCategory, AudioSampleFormat,
    NegotiatedStreamFormat, SampleRate, SupportedStreamConfiguration,
};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "channels", rename_all = "snake_case")]
pub enum ChannelCountPolicy {
    Exact(u16),
    AtLeast(u16),
    Maximum,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "sample_rate", rename_all = "snake_case")]
pub enum SampleRatePolicy {
    Exact(SampleRate),
    ClosestTo(SampleRate),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "sample_format", rename_all = "snake_case")]
pub enum SampleFormatPolicy {
    Exact(AudioSampleFormat),
    PreferF32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StreamNegotiationRequest {
    pub direction: AudioDirection,
    pub channels: ChannelCountPolicy,
    pub sample_rate: SampleRatePolicy,
    pub sample_format: SampleFormatPolicy,
    pub buffer: AudioBufferPolicy,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DeviceNegotiator;

impl DeviceNegotiator {
    pub fn negotiate(
        &self,
        device: &AudioDeviceDescriptor,
        request: StreamNegotiationRequest,
    ) -> Result<NegotiatedStreamFormat, AudioError> {
        request.buffer.validate()?;
        let mut best: Option<Candidate> = None;
        for supported in &device.supported_configurations {
            supported.validate()?;
            let Some(candidate) = Candidate::new(supported, request) else {
                continue;
            };
            if best.as_ref().is_none_or(|current| candidate.score < current.score) {
                best = Some(candidate);
            }
        }
        best.map(Candidate::into_format).ok_or_else(|| {
            AudioError::new(
                AudioErrorCategory::UnsupportedFormat,
                "no supported device configuration satisfies the requested stream policy",
            )
            .with_context("device", device.label.clone())
            .with_context("direction", format!("{:?}", request.direction).to_lowercase())
            .with_context("channels", format!("{:?}", request.channels))
            .with_context("sample_rate", format!("{:?}", request.sample_rate))
            .with_context("sample_format", format!("{:?}", request.sample_format))
            .with_context("buffer", format!("{:?}", request.buffer))
        })
    }
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    sample_rate: u32,
    channels: u16,
    sample_format: AudioSampleFormat,
    buffer_frames: u32,
    score: (u32, u16, u8, u32, u32, u32, u16),
}

impl Candidate {
    fn new(supported: &SupportedStreamConfiguration, request: StreamNegotiationRequest) -> Option<Self> {
        if supported.direction != request.direction {
            return None;
        }
        let channel_penalty = match request.channels {
            ChannelCountPolicy::Exact(channels) if channels == supported.channels => 0,
            ChannelCountPolicy::Exact(_) => return None,
            ChannelCountPolicy::AtLeast(minimum) if supported.channels >= minimum => supported.channels - minimum,
            ChannelCountPolicy::AtLeast(_) => return None,
            ChannelCountPolicy::Maximum => u16::MAX - supported.channels,
        };
        let (sample_rate, rate_penalty) = match request.sample_rate {
            SampleRatePolicy::Exact(rate)
                if (supported.min_sample_rate..=supported.max_sample_rate).contains(&rate.get()) =>
            {
                (rate.get(), 0)
            }
            SampleRatePolicy::Exact(_) => return None,
            SampleRatePolicy::ClosestTo(rate) => {
                let chosen = rate.get().clamp(supported.min_sample_rate, supported.max_sample_rate);
                (chosen, rate.get().abs_diff(chosen))
            }
        };
        let format_penalty = match request.sample_format {
            SampleFormatPolicy::Exact(format) if format == supported.sample_format => 0,
            SampleFormatPolicy::Exact(_) => return None,
            SampleFormatPolicy::PreferF32 => format_rank(supported.sample_format),
        };
        let buffer_frames = match request.buffer {
            AudioBufferPolicy::Automatic => supported.buffer_frames.preferred,
            AudioBufferPolicy::Fixed(frames)
                if (supported.buffer_frames.min..=supported.buffer_frames.max).contains(&frames) =>
            {
                frames
            }
            AudioBufferPolicy::Fixed(_) => return None,
        };
        Some(Self {
            sample_rate,
            channels: supported.channels,
            sample_format: supported.sample_format,
            buffer_frames,
            score: (
                rate_penalty,
                channel_penalty,
                format_penalty,
                buffer_frames,
                supported.min_sample_rate,
                supported.max_sample_rate,
                supported.channels,
            ),
        })
    }

    fn into_format(self) -> NegotiatedStreamFormat {
        NegotiatedStreamFormat {
            sample_rate: self.sample_rate,
            channels: self.channels,
            sample_format: self.sample_format,
            buffer_frames: self.buffer_frames,
            estimated_latency_ms: self.buffer_frames as f32 * 1_000.0 / self.sample_rate as f32,
        }
    }
}

const fn format_rank(format: AudioSampleFormat) -> u8 {
    match format {
        AudioSampleFormat::F32 => 0,
        AudioSampleFormat::F64 => 1,
        AudioSampleFormat::I32 => 2,
        AudioSampleFormat::I24 => 3,
        AudioSampleFormat::I16 => 4,
        AudioSampleFormat::U32 => 5,
        AudioSampleFormat::U24 => 6,
        AudioSampleFormat::U16 => 7,
    }
}
