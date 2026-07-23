#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

//! Backend-neutral audio engine, device, render, playback, and analysis contracts.
//!
//! Native backend and application-domain types are intentionally excluded from the public API.

pub mod backend;
pub mod config;
#[cfg(feature = "codegen")]
pub mod contract;
pub mod control;
pub mod device;
pub mod diagnostics;
pub mod error;
pub mod ids;
pub mod limits;
pub mod render;

pub use backend::{
    AudioBackend, AudioStream, BackendDescriptor, BackendPolicy, MockBackend, MockBackendControl, NullBackend,
    StreamRequest,
};
pub use config::{
    AnalysisTapConfiguration, AudioConfiguration, AudioEngineConfig, DirectionConfiguration, GainDb, InputPatchRoute,
    MonitorRoute, OutputPatchRoute, PlaybackRoute, VirtualInputChannel, VirtualOutputChannel,
};
pub use control::{
    AudioCommand, AudioControl, AudioEngine, AudioEngineBuilder, AudioEvent, AudioEventReceiver,
    AudioObservationReader, AudioObservationSnapshot, ChannelObservation, PlayFileRequest, PlaybackFailure,
    PlaybackInfo, PlaybackStopInfo, PlaybackStopReason,
};
pub use device::{
    AudioBackendState, AudioBackendStatus, AudioBufferPolicy, AudioDeviceDescriptor, AudioDeviceInspectorState,
    AudioDeviceReadiness, AudioDeviceTargetId, AudioDirection, AudioInspectorError, AudioPermissionState,
    AudioRecoveryPolicy, AudioSampleFormat, AudioStreamStatus, NegotiatedStreamFormat, PhysicalChannelDescriptor,
};
pub use diagnostics::{DiagnosticEvent, DiagnosticSeverity};
pub use error::{AudioError, AudioErrorCategory};
pub use ids::{
    AnalysisTapId, AudioChannelId, AudioDeviceId, AudioRouteId, BackendId, CommandSequence, ConfigGeneration,
    InvalidIdentifier, PhysicalChannelKey, PlaybackId, VoiceId,
};
pub use limits::{EngineLimits, FrameCount, SampleRate};
pub use render::OfflineClock;

#[cfg(test)]
mod tests;
