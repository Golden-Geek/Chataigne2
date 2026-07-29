#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]

//! Backend-neutral audio engine, device, render, playback, and analysis contracts.
//!
//! Native backend and application-domain types are intentionally excluded from the public API.

pub mod analysis;
pub mod backend;
pub mod clock;
pub mod config;
#[cfg(feature = "codegen")]
pub mod contract;
pub mod control;
pub mod device;
pub mod diagnostics;
pub mod error;
pub mod ids;
pub mod limits;
pub mod playback;
#[cfg(all(feature = "analysis", feature = "playback"))]
pub mod qualification;
pub mod realtime;
pub mod render;

#[cfg(feature = "analysis")]
pub use analysis::{
    AnalysisController, AnalysisObservationReader, AnalysisRenderer, AnalysisRendererRetirement, PitchAnalyzer,
    SpectrumAnalyzer, SpectrumBandGeometry, analysis_pipeline, spectrum_band_geometry,
};
pub use analysis::{
    AnalysisDiagnosticsObservation, AnalysisObservationSnapshot, AnalysisProcessorConfiguration, AnalysisResult,
    AnalysisTapObservation, ChannelObservation, MeterAccumulator, PitchAnalysisConfiguration, PitchObservation,
    SpectrumAnalysisConfiguration, SpectrumBandObservation, SpectrumBandSpacing, SpectrumObservation, SpectrumOverlap,
    SpectrumWindow, linear_to_dbfs,
};
pub use backend::{
    AudioBackend, AudioCallbackTimestamp, AudioStream, AudioStreamHandler, BackendDescriptor, BackendPolicy,
    MockBackend, MockBackendControl, MockBackendEvent, MockBackendEventKind, NullBackend, StreamRequest,
};
#[cfg(feature = "desktop")]
pub use backend::{
    NativeAudioBackendInfo, compiled_cpal_backend_catalog, compiled_cpal_backends, cpal_backend_by_id,
    probe_cpal_backends,
};
pub use clock::{
    ClockAuthority, ClockBlock, ClockBridgeConfig, ClockBridgeObservation, ClockHandoffPhase, ClockSource,
    DriftController, DriftControllerConfig, InputClockReader, InputClockWriter, InputReadError, InputReadResult,
    InputWriteError, InputWriteResult, NullClockDriver, RenderClockCoordinator, input_clock_bridge,
};
pub use config::{
    AnalysisTapConfiguration, AudioConfiguration, AudioEngineConfig, DirectionConfiguration, GainDb, InputPatchRoute,
    MonitorRoute, OutputPatchRoute, PlaybackRoute, VirtualInputChannel, VirtualOutputChannel,
};
pub use control::{
    AudioCommand, AudioControl, AudioEngine, AudioEngineBuilder, AudioEvent, AudioEventReceiver,
    AudioObservationReader, AudioObservationSnapshot, AudioQueueKind, PlayFileRequest, PlaybackCommandIgnored,
    PlaybackCommandIgnoredReason, PlaybackCommandKind, PlaybackFailure, PlaybackInfo, PlaybackObservation,
    PlaybackStopInfo, PlaybackStopReason, QueuePressureEvent, RenderRuntimeObservation,
};
pub use device::{
    AudioBackendState, AudioBackendStatus, AudioBufferPolicy, AudioDeviceCatalogEntry, AudioDeviceDescriptor,
    AudioDeviceFingerprint, AudioDeviceInspectorState, AudioDeviceInventory, AudioDeviceMatch, AudioDeviceProfile,
    AudioDeviceReadiness, AudioDeviceSelection, AudioDeviceTargetId, AudioDirection, AudioInspectorError,
    AudioPermissionState, AudioRecoveryPolicy, AudioSampleFormat, AudioStreamStatus, ChannelCountPolicy,
    DeviceNegotiator, DeviceProfileStore, DeviceSupervisor, DeviceSupervisorConfig, DeviceSwitchPhase,
    NegotiatedStreamFormat, PhysicalChannelDescriptor, RetryBackoff, SampleFormatPolicy, SampleRatePolicy,
    StreamNegotiationRequest, SupervisorDirection, SupportedBufferFrames, SupportedStreamConfiguration,
    match_device_selection, profile_key_for,
};
pub use diagnostics::{DiagnosticEvent, DiagnosticSeverity};
pub use error::{AudioError, AudioErrorCategory};
pub use ids::{
    AnalysisTapId, AudioChannelId, AudioDeviceId, AudioDeviceProfileKey, AudioRouteId, BackendId, CommandSequence,
    ConfigGeneration, InvalidIdentifier, PhysicalChannelKey, PlaybackId, VoiceId,
};
pub use limits::{EngineLimits, FrameCount, SampleRate};
pub use playback::{
    AssetCache, AudioFileFormat, AudioFileFormatDescriptor, AudioSourceFingerprint, CacheInsertResult,
    CacheObservation, DefaultPlaybackRoute, PlaybackRenderEvent, PlaybackRenderMetrics, PlaybackRendererRetirement,
    PlaybackVoice, PlaybackVoiceController, PlaybackVoiceRenderer, PlaybackVoiceSource, ResidentAssetKey,
    ResidentAudioAsset, StreamPlaybackReader, StreamPlaybackState, StreamPlaybackWriter, StreamWriteError,
    StreamWriteResult, audio_file_format_for_extension, default_playback_routes, playback_voice_pool,
    streaming_playback_ring, supported_audio_extensions, supported_audio_formats,
};
#[cfg(feature = "playback")]
pub use playback::{
    AudioFileProbe, PlaybackPreparation, PlaybackPreparationFailure, PlaybackPreparationResult, PlaybackScheduler,
    PlaybackSchedulerConfig, PlaybackSchedulerRequest, decode_audio_file, probe_audio_file,
};
pub use realtime::{
    AnalysisCaptureError, AnalysisFrame, AnalysisFrameReader, AnalysisFrameTag, AnalysisFrameWriter,
    AnalysisRecycleError, AnalysisWriterRetirement, GainMailboxTarget, OrderedRealtimeControlReader,
    OrderedRealtimeControlWriter, PlanPublishError, PlanSwapResult, PreparedVoice, QueuePressureCounters,
    QueuePressureSnapshot, RealtimeBarrier, RealtimeBarrierKind, RealtimeControlUpdate, RealtimePlanRetirement,
    RealtimePlanSlot, RealtimePlanSlotMetrics, RealtimeScope, RealtimeVoiceRetirement, RealtimeVoiceSlots,
    RenderPlanPublisher, RetiredVoice, VoiceRetirementReason, VoiceSlotController, acknowledged_plan_exchange,
    analysis_frame_pool, assert_not_realtime, is_realtime_thread, ordered_realtime_controls, voice_slot_pool,
};
pub use render::{
    CompiledAnalysisTap, CompiledRoute, CompiledRouteMatrix, ConversionStats, GainSmoother, InterleavedInput,
    InterleavedOutput, OfflineClock, OfflineRenderer, PlanarBuffer, RenderCompileContext, RenderPlan,
    RenderPlanCompilation, RenderPlanCompiler, RenderProcessor, RenderProcessorMetrics, RenderWarning,
    RenderWarningCode, RouteSpan, deinterleave, interleave, render_scalar_reference,
};

#[cfg(test)]
mod tests;
