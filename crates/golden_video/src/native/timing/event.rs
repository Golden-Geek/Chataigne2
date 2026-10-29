use std::num::NonZeroUsize;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::runtime::{CompletionToken, DeviceGeneration};
use crate::{OutputId, TextureId};

/// Opt-in telemetry storage. Query pools always contain two slots per native context.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct DiagnosticsConfig {
    /// Fixed-size row capacity, from one through 65,536. Full rings drop and count rows.
    pub capacity: NonZeroUsize,
}

/// Identity of one context's work; timestamps from different contexts are not subtracted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticContext {
    /// The renderer's immediate context; the row token identifies its device lifetime.
    Renderer,
    /// An independently owned source worker/device.
    Source {
        /// Exact subscription identity, including replacement on the same renderer device.
        binding: Uuid,
        /// Logical source in the renderer's graph.
        texture: TextureId,
        /// Renderer device demanding this subscription.
        renderer_device: DeviceGeneration,
    },
    /// Qualification-owned producer context, independent from the renderer.
    FixtureProducer {
        /// Fixture-local producer identity.
        id: Uuid,
    },
    /// Qualification-owned consumer context, independent from the renderer.
    FixtureConsumer {
        /// Fixture-local consumer identity.
        id: Uuid,
    },
}

/// Inclusive wall interval in machine-wide QueryPerformanceCounter ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuInterval {
    /// First observed QPC coordinate.
    pub begin: u64,
    /// Last observed QPC coordinate.
    pub end: u64,
}

/// GetThreadTimes deltas for this worker. Resolution is the operating system's accounting
/// granularity; these are not inferred by subtracting consumer calls from wall time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadCpuDelta {
    /// Kernel-mode CPU time, in 100-nanosecond accounting units.
    pub kernel_100ns: u64,
    /// User-mode CPU time, in 100-nanosecond accounting units.
    pub user_100ns: u64,
}

/// Renderer CPU observations for one submitted batch, including output API wall time.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FrameCpuTrace {
    /// Exact renderer device and batch serial.
    pub token: CompletionToken,
    /// Inclusive submission wall interval, not active CPU time.
    pub wall: Option<CpuInterval>,
    /// Optional OS-accounted thread CPU delta for the inclusive interval.
    pub thread_cpu: Option<ThreadCpuDelta>,
    /// Source import/copy and missing-input clears.
    pub inputs: Option<CpuInterval>,
    /// Logical render passes, before per-output conversion and delivery.
    pub render: Option<CpuInterval>,
    /// Completion-query End/Flush wall interval, including driver API elapsed time.
    pub completion: Option<CpuInterval>,
}

/// API delivery disposition; acceptance is not presentation, freshness or scanout proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticDelivery {
    /// Consumer API accepted the submission.
    Accepted,
    /// Consumer capacity was busy without waiting for it.
    Busy,
    /// Consumer was unavailable, hidden, occluded or awaiting recovery.
    Unavailable,
    /// Ordinary endpoint-local failure.
    Failed {
        /// Native HRESULT supplied by the endpoint.
        hresult: i32,
    },
    /// Device removal confirmed by the owning device.
    DeviceLost {
        /// Actual GetDeviceRemovedReason HRESULT.
        hresult: i32,
    },
}

/// Per-output conversion and API wall brackets for exact cadence/acceptance joins.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OutputTrace {
    /// Renderer batch supplied to this output.
    pub token: CompletionToken,
    /// Fixed native consumer identity.
    pub output: OutputId,
    /// Conversion draw submission wall interval.
    pub conversion: Option<CpuInterval>,
    /// Entire endpoint delivery wall interval, including validation and consumer API work.
    pub delivery: Option<CpuInterval>,
    /// Exact Window Present or Spout SendTexture API interval. It can contain CPU work and
    /// waiting; the elapsed interval alone does not distinguish those components.
    pub consumer_api: Option<CpuInterval>,
    /// Consumer API disposition.
    pub outcome: DiagnosticDelivery,
}

/// Source publication metadata retained with the renderer texture actually containing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationTiming {
    /// Source-context batch; source GPU rows use this exact token.
    pub source_token: CompletionToken,
    /// Before the SDK receive call. This is not a producer-frame timestamp.
    pub receive_begin_qpc: Option<u64>,
    /// Before publishing the owned GPU bridge lease. This is not GPU completion.
    pub publication_qpc: Option<u64>,
}

/// Source frame actually used by one renderer batch, including unchanged retained frames.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SourceUseTrace {
    /// Renderer batch using the retained texture contents.
    pub token: CompletionToken,
    /// Logical external texture.
    pub texture: TextureId,
    /// Runtime-local publication sequence; zero means the missing-input clear.
    pub sequence: u64,
    /// Exact retained source metadata, absent for missing or uninstrumented publication.
    pub publication: Option<PublicationTiming>,
}

/// Source SDK/copy CPU wall observations for one source-context batch.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SourceCpuTrace {
    /// Source-context batch identity, distinct from renderer-device completion.
    pub token: CompletionToken,
    /// Exact source subscription/context.
    pub context: DiagnosticContext,
    /// Inclusive source batch wall interval.
    pub wall: Option<CpuInterval>,
    /// Optional OS-accounted thread CPU delta.
    pub thread_cpu: Option<ThreadCpuDelta>,
    /// SDK receive API wall interval; no inference about freshness or blocking is made.
    pub receive_api: Option<CpuInterval>,
    /// Bridge copy, flush and keyed-mutex handoff wall interval.
    pub bridge: Option<CpuInterval>,
    /// Completion query End/Flush wall interval.
    pub completion: Option<CpuInterval>,
}

/// GPU sample validity. Missing/invalid observations remain visible in coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuSampleStatus {
    /// Non-disjoint, nonzero-frequency, ordered timestamp pair on this context.
    Valid,
    /// Driver reported discontinuity in this timestamp scope.
    Disjoint,
    /// Driver returned zero frequency or reversed timestamps.
    Invalid,
    /// Both fixed query slots were still retained; rendering continued without timing.
    PoolBusy,
    /// Query allocation/read failed. This does not fault the renderer.
    Failed {
        /// Native HRESULT from the diagnostic query API.
        hresult: i32,
    },
    /// Context retired before query results could be collected.
    Uncollected,
}

/// One context-local GPU span. It is neither a cross-context critical path nor display age.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct GpuTrace {
    /// Exact context-local batch/device lifetime.
    pub token: CompletionToken,
    /// Context whose commands enclose the timestamps.
    pub context: DiagnosticContext,
    /// Readiness/validity outcome.
    pub status: GpuSampleStatus,
    /// Raw start timestamp, absent when unavailable.
    pub begin: Option<u64>,
    /// Raw end timestamp, absent when unavailable.
    pub end: Option<u64>,
    /// GPU counter ticks per second for this disjoint scope.
    pub frequency: Option<u64>,
}

/// Fixed-size Rust telemetry row for offline qualification. No UI event is emitted.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DiagnosticEvent {
    /// Inclusive renderer CPU phases.
    FrameCpu(FrameCpuTrace),
    /// Per-consumer CPU brackets and acceptance.
    Output(OutputTrace),
    /// Exact retained source publication used by a renderer batch.
    SourceUse(SourceUseTrace),
    /// Independent source worker CPU phases.
    SourceCpu(SourceCpuTrace),
    /// Context-local GPU timing and explicit coverage status.
    Gpu(GpuTrace),
}

/// Bounded telemetry health. Lost rows invalidate complete-coverage claims.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct DiagnosticsSnapshot {
    /// Whether telemetry is configured for this runtime.
    pub enabled: bool,
    /// Fixed row capacity, zero while disabled.
    pub capacity: usize,
    /// Rows currently retained in the ring.
    pub queued: usize,
    /// Rows successfully written, including warmup.
    pub written: u64,
    /// Rows drained by a consumer.
    pub drained: u64,
    /// Rows dropped because the ring was full.
    pub dropped_full: u64,
    /// Rows dropped because a producer could not acquire the metadata lock immediately.
    pub dropped_contended: u64,
    /// Failed QPC calls; associated timestamps are absent.
    pub clock_failures: u64,
    /// Machine-wide QPC tick frequency, absent if unavailable.
    pub qpc_frequency: Option<u64>,
}
