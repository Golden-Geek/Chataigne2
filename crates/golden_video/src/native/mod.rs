//! Worker-local D3D11 execution with explicit adapter and immutable native output bindings.
//!
//! Only configuration and bounded observations leave this module's worker. The host owns HWND
//! lifetime; neither COM objects nor textures enter engine/UI mailboxes. Dynamic preview geometry
//! remains separate integration work. Bounded source workers own their SDK and device work;
//! renderer imports retain allocation leases through their own matching completion.

mod allocations;
mod backend;
mod capture;
mod completion;
mod config;
mod device;
mod discovery;
#[cfg(feature = "qualification")]
pub mod fixtures;
mod frames;
mod inputs;
mod memory;
mod observation;
mod outputs;
mod pipeline;
mod probe;
mod process;
mod selection;
mod spout_layout;
mod texture;
pub(crate) mod timing;

pub use allocations::{
    NativeAllocationSnapshot, OutputTextureAllocation, OutputTextureSnapshot, OwnedTextureSnapshot,
    SourceTextureSnapshot,
};
pub use capture::{DiagnosticCapture, DiagnosticCaptureLimits, DiagnosticCaptureSummary};
pub use config::{AdapterId, D3d11Config, InputLimits, NativeConfigError, OutputBinding, WindowTarget};
pub use device::{AdapterInfo, AdapterOutput, AdapterOutputs, adapters, preferred_adapters};
pub use discovery::{DiscoveryError, MAX_DISCOVERY_SENDERS, SpoutDiscovery, SpoutSenderCatalog};
pub use frames::{GpuFrameBudget, GpuFrameClock, GpuFramePublication, GpuFrameStream, GpuImage};
#[cfg(feature = "ffmpeg")]
pub use frames::{GpuFrameError, GpuFramePublisher};
pub use inputs::{SourceSnapshot, SourceState};
pub use memory::{
    AdapterMemorySnapshot, MemoryAdmissionError, MemoryQueryError, MemorySegmentSnapshot, query_adapter_memory,
};
pub use observation::{
    D3d11Control, D3d11Snapshot, NativeFailure, OutputSnapshot, OutputState, SourceObservationSnapshot,
};
#[cfg(feature = "qualification")]
pub use probe::ProbePixels;
pub use probe::{SpoutProbe, SpoutProbeSnapshot};
pub use process::{
    ProcessMemorySnapshot, ProcessQueryError, ProcessResourceSnapshot, ProcessTimeSnapshot, query_process_resources,
};
pub use selection::{AdapterPreference, AdapterSelectionError, select_adapter};
pub use timing::{
    CpuInterval, DiagnosticContext, DiagnosticDelivery, DiagnosticEvent, DiagnosticsConfig, DiagnosticsSnapshot,
    FrameCpuTrace, GpuSampleStatus, GpuTrace, ImageUploadTrace, OutputQueueBound, OutputTrace, PublicationTiming,
    ScheduleClock, ScheduleClockError, ScheduleDisposition, ScheduleTrace, SoftwareUploadTrace, SourceCpuTrace,
    SourceUseTrace, SpoutImageVersion, ThreadCpuDelta, TimedFrameTrace,
};

pub(crate) use backend::D3d11Backend;

#[cfg(test)]
pub(crate) mod tests;
