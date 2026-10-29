use std::io;
use std::num::NonZeroUsize;
use std::time::{Duration, Instant};

use golden_io::{
    BoundedWorker, PayloadPool, PendingDrain, PendingSendError, RetainedPayload, RetirementPool, WorkerContext,
    WorkerLimits, WorkerNotifier, WorkerStartError,
};
use thiserror::Error;

use crate::{AssetCapture, AssetId, AssetInput, MAX_ASSET_PATH_BYTES, SourceFingerprint};

/// Trusted backend codec contract. Enforce `maximum_bytes` while allocating, including decode
/// scratch and the result. Retained size includes the value and all owned heap allocations.
/// Keep format-specific caches and native/GPU objects outside this CPU preparation boundary.
pub trait AssetDecoder: Send + 'static {
    type Output: Send + Sync + 'static;
    fn decode(&mut self, input: &mut AssetInput, maximum_bytes: usize) -> Result<Self::Output, PreparationFailure>;
    fn retained_bytes(&self, value: &Self::Output) -> usize;
}

#[derive(Clone, Copy, Debug)]
pub struct PreparationLimits {
    pub command_items: NonZeroUsize,
    pub event_items: NonZeroUsize,
    pub max_source_bytes: u64,
    /// Maximum working allocation admitted before invoking the decoder.
    pub max_decode_bytes: NonZeroUsize,
    /// Includes results retained by consumers and their final destruction.
    pub max_prepared_items: NonZeroUsize,
    pub max_prepared_bytes: NonZeroUsize,
}

impl Default for PreparationLimits {
    fn default() -> Self {
        Self {
            command_items: NonZeroUsize::new(8).unwrap(),
            event_items: NonZeroUsize::new(8).unwrap(),
            max_source_bytes: 64 * 1024 * 1024,
            max_decode_bytes: NonZeroUsize::new(32 * 1024 * 1024).unwrap(),
            max_prepared_items: NonZeroUsize::new(8).unwrap(),
            max_prepared_bytes: NonZeroUsize::new(256 * 1024 * 1024).unwrap(),
        }
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PreparationFailure {
    #[error("asset source is missing")]
    Missing,
    #[error("asset IO failed: {0:?}")]
    Io(io::ErrorKind),
    #[error("asset source is not a regular file")]
    NotAFile,
    #[error("asset source exceeds its admitted byte limit")]
    SourceTooLarge,
    #[error("asset content changed during preparation")]
    SourceChanged,
    #[error("asset project or binding changed")]
    Stale,
    #[error("asset preparation was cancelled")]
    Cancelled,
    #[error("asset decoder rejected the source")]
    Decode,
    #[error("asset preparation resource capacity is exhausted")]
    ResourcePressure,
}

impl From<io::Error> for PreparationFailure {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::NotFound {
            Self::Missing
        } else {
            Self::Io(error.kind())
        }
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum PreparationAdmissionError {
    #[error("asset worker is closed")]
    Closed,
    #[error("asset command capacity is exhausted")]
    Full,
    #[error("asset input exceeds admitted metadata storage")]
    InputTooLarge,
    #[error("asset capture is stale")]
    Stale,
    #[error("asset request identity exhausted")]
    Exhausted,
}

#[derive(Debug, Error)]
pub enum PreparationStartError {
    #[error("invalid asset preparation limits")]
    InvalidLimits,
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Worker(#[from] WorkerStartError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetRequestId(u64);
impl AssetRequestId {
    pub fn get(self) -> u64 {
        self.0
    }
}

struct Prepared<T> {
    retained_bytes: usize,
    capture: AssetCapture,
    fingerprint: SourceFingerprint,
    value: T,
}

/// Cloneable preparation lease. Its reservation survives all clones, queued outcomes and final
/// disposal. Check `is_current` immediately before installing it into a consumer's generation.
pub struct PreparedAsset<T: Send + Sync + 'static>(RetainedPayload<Prepared<T>>);

impl<T: Send + Sync + 'static> Clone for PreparedAsset<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: Send + Sync + 'static> std::fmt::Debug for PreparedAsset<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedAsset")
            .field("id", &self.id())
            .field("fingerprint", &self.fingerprint())
            .finish()
    }
}
impl<T: Send + Sync + 'static> PreparedAsset<T> {
    pub fn id(&self) -> AssetId {
        self.0.capture.id()
    }
    pub fn is_current(&self) -> bool {
        self.0.capture.is_current()
    }
    pub fn value(&self) -> &T {
        &self.0.value
    }
    pub fn fingerprint(&self) -> SourceFingerprint {
        self.0.fingerprint
    }
    /// Conservative sealed CPU storage charge, including captured source/guard metadata.
    pub fn retained_bytes(&self) -> usize {
        self.0.retained_bytes
    }
    /// Forward these admission fences when a downstream preparation/installation outlives this
    /// call. Hold a prepared lease separately for as long as its data is needed by that consumer.
    pub fn guards(&self) -> &[golden_io::RevisionGuard] {
        &self.0.capture.guards
    }
}

#[derive(Debug)]
pub struct PreparationOutcome<T: Send + Sync + 'static> {
    pub request: AssetRequestId,
    pub asset: AssetId,
    pub result: Result<PreparedAsset<T>, PreparationFailure>,
}

struct Job {
    request: AssetRequestId,
    capture: AssetCapture,
}

/// Host-shutdown observation. Waiting belongs off the control/UI thread and requires releasing
/// queued outcomes and consumer leases. Cancellation alone is not resource retirement.
#[derive(Clone)]
pub struct AssetRetirement {
    workers: RetirementPool,
    payloads: PayloadPool,
}
impl AssetRetirement {
    pub fn wait(&self, timeout: Duration) -> bool {
        let started = Instant::now();
        self.workers.wait_for_idle(timeout) && self.payloads.wait_for_idle(timeout.saturating_sub(started.elapsed()))
    }
}

/// One bounded decoder worker and one Golden IO payload disposer. Neither drop nor cancel joins.
/// A blocked OS call/decoder keeps its worker slot; this owner never spawns a replacement worker.
pub struct AssetPreparer<D: AssetDecoder> {
    worker: BoundedWorker<Job, PreparationOutcome<D::Output>>,
    retirement: AssetRetirement,
    next_request: u64,
    command_weight: usize,
}

impl<D: AssetDecoder> AssetPreparer<D> {
    pub fn new(decoder: D, limits: PreparationLimits) -> Result<Self, PreparationStartError> {
        Self::start(decoder, limits, None)
    }

    /// Event/disconnection wake for an existing host/preparation owner; no polling is required.
    pub fn new_notified(
        decoder: D,
        limits: PreparationLimits,
        notifier: WorkerNotifier,
    ) -> Result<Self, PreparationStartError> {
        Self::start(decoder, limits, Some(notifier))
    }

    fn start(
        decoder: D,
        limits: PreparationLimits,
        notifier: Option<WorkerNotifier>,
    ) -> Result<Self, PreparationStartError> {
        let command_weight = 4 * MAX_ASSET_PATH_BYTES + size_of::<Job>() + 512;
        let commands = command_weight
            .checked_mul(limits.command_items.get())
            .ok_or(PreparationStartError::InvalidLimits)?;
        let events = size_of::<PreparationOutcome<D::Output>>()
            .checked_mul(limits.event_items.get())
            .ok_or(PreparationStartError::InvalidLimits)?;
        if limits.max_source_bytes == 0
            || limits.max_source_bytes.checked_mul(4).is_none()
            || usize::try_from(limits.max_source_bytes).is_err()
        {
            return Err(PreparationStartError::InvalidLimits);
        }
        let payloads = PayloadPool::new(
            "golden-assets-dispose",
            limits.max_prepared_items.get(),
            limits.max_prepared_bytes.get(),
        )?;
        let workers = RetirementPool::new(NonZeroUsize::MIN);
        let retirement = AssetRetirement {
            workers: workers.clone(),
            payloads: payloads.clone(),
        };
        let bounds = WorkerLimits {
            command_items: limits.command_items.get(),
            command_weight: commands,
            event_items: limits.event_items.get(),
            event_weight: events,
        };
        let run = move |context| run(decoder, limits, payloads, context);
        let worker = match notifier {
            Some(notifier) => BoundedWorker::spawn_notified("golden-assets-prepare", &workers, bounds, notifier, run)?,
            None => BoundedWorker::spawn("golden-assets-prepare", &workers, bounds, run)?,
        };
        Ok(Self {
            worker,
            retirement,
            next_request: 0,
            command_weight,
        })
    }

    pub fn submit(&mut self, capture: AssetCapture) -> Result<AssetRequestId, PreparationAdmissionError> {
        if !capture.is_current() {
            return Err(PreparationAdmissionError::Stale);
        }
        let weight = capture.retained_bytes() + size_of::<Job>();
        if weight > self.command_weight {
            return Err(PreparationAdmissionError::InputTooLarge);
        }
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(PreparationAdmissionError::Exhausted)?;
        let request = AssetRequestId(self.next_request);
        self.worker
            .send_weighted(Job { request, capture }, weight)
            .map_err(|error| match error {
                PendingSendError::Full(_) => PreparationAdmissionError::Full,
                PendingSendError::Disconnected(_) => PreparationAdmissionError::Closed,
            })?;
        Ok(request)
    }

    pub fn drain(&self, outcomes: &mut Vec<PreparationOutcome<D::Output>>, budget: NonZeroUsize) -> PendingDrain {
        self.worker.drain_events(outcomes, budget)
    }
    pub fn cancel(&self) {
        self.worker.cancel();
    }
    pub fn retirement(&self) -> AssetRetirement {
        self.retirement.clone()
    }
}

fn run<D: AssetDecoder>(
    mut decoder: D,
    limits: PreparationLimits,
    payloads: PayloadPool,
    context: WorkerContext<Job, PreparationOutcome<D::Output>>,
) {
    while let Some(job) = context.receive() {
        let asset = job.capture.id();
        let request = job.request;
        let result = prepare(&mut decoder, job.capture, limits, &payloads, context.cancellation());
        let outcome = PreparationOutcome { request, asset, result };
        if context
            .publish_weighted(outcome, size_of::<PreparationOutcome<D::Output>>())
            .is_err()
        {
            break;
        }
    }
}

fn prepare<D: AssetDecoder>(
    decoder: &mut D,
    capture: AssetCapture,
    limits: PreparationLimits,
    payloads: &PayloadPool,
    cancel: &golden_io::WorkerCancellation,
) -> Result<PreparedAsset<D::Output>, PreparationFailure> {
    crate::input::check(&capture, cancel)?;
    let weight = limits
        .max_decode_bytes
        .get()
        .checked_add(capture.retained_bytes() * 2)
        .and_then(|weight| weight.checked_add(size_of::<Prepared<D::Output>>() + 256))
        .ok_or(PreparationFailure::ResourcePressure)?;
    let reservation = payloads
        .try_reserve(
            weight
                .checked_add(
                    usize::try_from(limits.max_source_bytes).map_err(|_| PreparationFailure::ResourcePressure)?,
                )
                .ok_or(PreparationFailure::ResourcePressure)?,
        )
        .map_err(|_| PreparationFailure::ResourcePressure)?;
    let mut input = AssetInput::open(capture.clone(), cancel.clone(), limits.max_source_bytes)?;
    let fingerprint = input.fingerprint()?;
    let decoded = decoder.decode(&mut input, limits.max_decode_bytes.get());
    input.check()?;
    let value = decoded?;
    if decoder.retained_bytes(&value) > limits.max_decode_bytes.get() {
        return Err(PreparationFailure::ResourcePressure);
    }
    drop(input);
    // Reopen the resolved path: rewinding the original handle misses atomic path replacement.
    let current = AssetInput::open(capture.clone(), cancel.clone(), limits.max_source_bytes)?;
    if fingerprint != current.fingerprint()? {
        return Err(PreparationFailure::SourceChanged);
    }
    // Release encoded staging before shrinking the reservation to retained decoded storage.
    drop(current);
    crate::input::check(&capture, cancel)?;
    let prepared = Prepared {
        retained_bytes: weight,
        capture,
        fingerprint,
        value,
    };
    Ok(PreparedAsset(
        reservation
            .retain(prepared, weight)
            .map_err(|_| PreparationFailure::ResourcePressure)?,
    ))
}
