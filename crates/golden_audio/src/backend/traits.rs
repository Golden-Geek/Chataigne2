use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    AudioBackendState, AudioBufferPolicy, AudioDeviceDescriptor, AudioDeviceTargetId, AudioDirection, AudioError,
    AudioSampleFormat, AudioStreamStatus, BackendId, ChannelCountPolicy, SampleFormatPolicy, SampleRate,
    SampleRatePolicy, StreamNegotiationRequest,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BackendDescriptor {
    pub id: BackendId,
    pub label: String,
    pub state: AudioBackendState,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BackendPolicy {
    pub preferred: Vec<BackendId>,
    pub allow_null_fallback: bool,
}

impl Default for BackendPolicy {
    fn default() -> Self {
        Self {
            preferred: vec![BackendId::from_static("null")],
            allow_null_fallback: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StreamRequest {
    pub direction: AudioDirection,
    pub target: AudioDeviceTargetId,
    pub engine_sample_rate: SampleRate,
    pub channels: u16,
    pub buffer_policy: AudioBufferPolicy,
}

impl StreamRequest {
    pub fn validate(&self) -> Result<(), AudioError> {
        if self.channels == 0 {
            return Err(AudioError::invalid_configuration(
                "stream request channel count must be greater than zero",
            ));
        }
        self.buffer_policy.validate()
    }

    #[must_use]
    pub fn negotiation_request(&self) -> StreamNegotiationRequest {
        StreamNegotiationRequest {
            direction: self.direction,
            channels: ChannelCountPolicy::Exact(self.channels),
            sample_rate: SampleRatePolicy::Exact(self.engine_sample_rate),
            sample_format: SampleFormatPolicy::Exact(AudioSampleFormat::F32),
            buffer: self.buffer_policy,
        }
    }
}

pub trait AudioStream: Send {
    fn status(&self) -> AudioStreamStatus;
    fn start(&mut self) -> Result<(), AudioError>;
    fn stop(&mut self) -> Result<(), AudioError>;
}

impl fmt::Debug for dyn AudioStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AudioStream")
    }
}

pub trait AudioBackend: Send + Sync {
    fn descriptor(&self) -> BackendDescriptor;
    fn discover(&self) -> Result<Vec<AudioDeviceDescriptor>, AudioError>;
    fn open_stream(&self, request: &StreamRequest) -> Result<Box<dyn AudioStream>, AudioError>;
}

impl fmt::Debug for dyn AudioBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let descriptor = self.descriptor();
        formatter
            .debug_struct("AudioBackend")
            .field("id", &descriptor.id)
            .field("state", &descriptor.state)
            .finish()
    }
}
