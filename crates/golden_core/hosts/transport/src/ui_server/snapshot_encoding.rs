//! Bounded background materialization and JSON encoding for immutable UI snapshots.

use std::io::{Error, ErrorKind};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

use golden_engine::engine::EngineTime;
use golden_engine::ui_read_model::UiSnapshotCapture;
use golden_protocol::UiSubscriptionScope;

pub(super) const SNAPSHOT_ENCODING_CAPACITY: usize = 8;
pub(super) const MAX_ENCODED_SNAPSHOT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct SnapshotEncodingService {
    request_tx: SyncSender<SnapshotEncodingJob>,
    metrics: Arc<SnapshotEncodingMetrics>,
}

#[derive(Clone)]
pub(super) struct SnapshotEncodingResult {
    pub(super) encoded: Arc<str>,
    pub(super) node_count: usize,
    pub(super) version: u64,
    pub(super) revision: EngineTime,
    pub(super) project_generation: u64,
    pub(super) materialize_elapsed: Duration,
    pub(super) encode_elapsed: Duration,
    pub(super) cache_hit: bool,
}

pub(super) enum WsSnapshotCompletion {
    Encoded {
        client_id: u64,
        request_id: String,
        snapshot: SnapshotEncodingResult,
    },
    Failed {
        client_id: u64,
        request_id: String,
        message: String,
    },
}

#[derive(Default)]
pub(super) struct PendingWsSnapshots {
    entries: Vec<PendingWsSnapshot>,
}

struct PendingWsSnapshot {
    client_id: u64,
    request_id: String,
    receiver: Receiver<Result<SnapshotEncodingResult, String>>,
}

impl PendingWsSnapshots {
    pub(super) fn submit(
        &mut self,
        service: &SnapshotEncodingService,
        client_id: u64,
        request_id: String,
        capture: UiSnapshotCapture,
    ) -> Result<(), SnapshotEncodingAdmissionError> {
        let receiver = service.try_encode(capture)?;
        self.entries.push(PendingWsSnapshot {
            client_id,
            request_id,
            receiver,
        });
        Ok(())
    }

    pub(super) fn remove_client(&mut self, client_id: u64) {
        self.entries.retain(|pending| pending.client_id != client_id);
    }

    pub(super) fn drain_ready(&mut self, mut consume: impl FnMut(WsSnapshotCompletion)) {
        let mut index = 0;
        while index < self.entries.len() {
            let completion = match self.entries[index].receiver.try_recv() {
                Ok(completion) => Some(completion),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err("snapshot encoding service stopped".to_string())),
            };
            let Some(completion) = completion else {
                index += 1;
                continue;
            };
            let pending = self.entries.swap_remove(index);
            consume(match completion {
                Ok(snapshot) => WsSnapshotCompletion::Encoded {
                    client_id: pending.client_id,
                    request_id: pending.request_id,
                    snapshot,
                },
                Err(message) => WsSnapshotCompletion::Failed {
                    client_id: pending.client_id,
                    request_id: pending.request_id,
                    message,
                },
            });
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SnapshotEncodingMetricsSnapshot {
    pub(super) queued: usize,
    pub(super) active: usize,
    pub(super) peak_active: usize,
    pub(super) accepted: u64,
    pub(super) rejected: u64,
    pub(super) materialized: u64,
    pub(super) cache_hits: u64,
    pub(super) retained_cache_bytes: usize,
}

#[derive(Default)]
struct SnapshotEncodingMetrics {
    queued: AtomicUsize,
    active: AtomicUsize,
    peak_active: AtomicUsize,
    accepted: AtomicU64,
    rejected: AtomicU64,
    materialized: AtomicU64,
    cache_hits: AtomicU64,
    retained_cache_bytes: AtomicUsize,
}

impl SnapshotEncodingMetrics {
    #[cfg(test)]
    fn snapshot(&self) -> SnapshotEncodingMetricsSnapshot {
        SnapshotEncodingMetricsSnapshot {
            queued: self.queued.load(Ordering::Acquire),
            active: self.active.load(Ordering::Acquire),
            peak_active: self.peak_active.load(Ordering::Acquire),
            accepted: self.accepted.load(Ordering::Acquire),
            rejected: self.rejected.load(Ordering::Acquire),
            materialized: self.materialized.load(Ordering::Acquire),
            cache_hits: self.cache_hits.load(Ordering::Acquire),
            retained_cache_bytes: self.retained_cache_bytes.load(Ordering::Acquire),
        }
    }
}

struct SnapshotEncodingJob {
    capture: UiSnapshotCapture,
    reply_tx: SyncSender<Result<SnapshotEncodingResult, String>>,
}

struct CachedWholeGraph {
    version: u64,
    result: SnapshotEncodingResult,
}

type BeforeEncodeHook = Arc<dyn Fn() + Send + Sync>;

impl SnapshotEncodingService {
    pub(super) fn spawn() -> std::io::Result<Self> {
        Self::spawn_with(SNAPSHOT_ENCODING_CAPACITY, None)
    }

    fn spawn_with(capacity: usize, before_encode: Option<BeforeEncodeHook>) -> std::io::Result<Self> {
        assert!(capacity > 0, "snapshot encoding capacity must be non-zero");
        let (request_tx, request_rx) = mpsc::sync_channel(capacity);
        let metrics = Arc::new(SnapshotEncodingMetrics::default());
        let worker_metrics = metrics.clone();
        thread::Builder::new()
            .name("golden-ui-snapshot-encoder".to_string())
            .spawn(move || snapshot_encoding_loop(request_rx, worker_metrics, before_encode))
            .map_err(|error| Error::other(format!("failed to spawn UI snapshot encoder: {error}")))?;
        Ok(Self { request_tx, metrics })
    }

    pub(super) fn try_encode(
        &self,
        capture: UiSnapshotCapture,
    ) -> Result<Receiver<Result<SnapshotEncodingResult, String>>, SnapshotEncodingAdmissionError> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        let job = SnapshotEncodingJob { capture, reply_tx };
        self.metrics.queued.fetch_add(1, Ordering::Release);
        match self.request_tx.try_send(job) {
            Ok(()) => {
                self.metrics.accepted.fetch_add(1, Ordering::Relaxed);
                Ok(reply_rx)
            }
            Err(TrySendError::Full(_)) => {
                self.metrics.queued.fetch_sub(1, Ordering::AcqRel);
                self.metrics.rejected.fetch_add(1, Ordering::Relaxed);
                Err(SnapshotEncodingAdmissionError::Full)
            }
            Err(TrySendError::Disconnected(_)) => {
                self.metrics.queued.fetch_sub(1, Ordering::AcqRel);
                Err(SnapshotEncodingAdmissionError::Unavailable)
            }
        }
    }

    #[cfg(test)]
    pub(super) fn metrics(&self) -> SnapshotEncodingMetricsSnapshot {
        self.metrics.snapshot()
    }

    #[cfg(test)]
    pub(super) fn spawn_with_test_hook(capacity: usize, before_encode: BeforeEncodeHook) -> std::io::Result<Self> {
        Self::spawn_with(capacity, Some(before_encode))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SnapshotEncodingAdmissionError {
    Full,
    Unavailable,
}

impl std::fmt::Display for SnapshotEncodingAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full => formatter.write_str("snapshot encoding capacity is exhausted; retry later"),
            Self::Unavailable => formatter.write_str("snapshot encoding service is unavailable"),
        }
    }
}

fn snapshot_encoding_loop(
    request_rx: Receiver<SnapshotEncodingJob>,
    metrics: Arc<SnapshotEncodingMetrics>,
    before_encode: Option<BeforeEncodeHook>,
) {
    let mut cache = None::<CachedWholeGraph>;
    while let Ok(job) = request_rx.recv() {
        metrics.queued.fetch_sub(1, Ordering::AcqRel);
        let active = metrics.active.fetch_add(1, Ordering::AcqRel) + 1;
        metrics.peak_active.fetch_max(active, Ordering::AcqRel);
        if let Some(hook) = before_encode.as_ref() {
            hook();
        }
        let result = encode_snapshot(job.capture, &mut cache, &metrics);
        let _ = job.reply_tx.send(result);
        metrics.active.fetch_sub(1, Ordering::AcqRel);
    }
}

fn encode_snapshot(
    capture: UiSnapshotCapture,
    cache: &mut Option<CachedWholeGraph>,
    metrics: &SnapshotEncodingMetrics,
) -> Result<SnapshotEncodingResult, String> {
    let version = capture.version();
    let revision = capture.revision();
    let project_generation = capture.project_generation().get();
    let whole_graph = matches!(capture.scope(), UiSubscriptionScope::WholeGraph);
    if whole_graph
        && let Some(cached) = cache.as_ref()
        && cached.version == version
    {
        metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
        let mut result = cached.result.clone();
        result.cache_hit = true;
        return Ok(result);
    }

    let materialize_started = Instant::now();
    let snapshot = capture.materialize();
    let materialize_elapsed = materialize_started.elapsed();
    if snapshot.at != revision {
        return Err("snapshot materialization changed its captured revision".to_string());
    }
    let node_count = snapshot.nodes.len();
    let encode_started = Instant::now();
    let encoded = serde_json::to_string(&snapshot).map_err(|error| format!("failed to encode UI snapshot: {error}"))?;
    let encode_elapsed = encode_started.elapsed();
    if encoded.len() > MAX_ENCODED_SNAPSHOT_BYTES {
        return Err(format!(
            "encoded UI snapshot exceeds the {} byte transport limit",
            MAX_ENCODED_SNAPSHOT_BYTES
        ));
    }

    metrics.materialized.fetch_add(1, Ordering::Relaxed);
    let result = SnapshotEncodingResult {
        encoded: Arc::from(encoded),
        node_count,
        version,
        revision,
        project_generation,
        materialize_elapsed,
        encode_elapsed,
        cache_hit: false,
    };
    if whole_graph {
        let should_publish = cache.as_ref().is_none_or(|cached| cached.version < version);
        if should_publish {
            metrics
                .retained_cache_bytes
                .store(result.encoded.len(), Ordering::Release);
            *cache = Some(CachedWholeGraph {
                version,
                result: result.clone(),
            });
        }
    }
    Ok(result)
}

pub(super) fn receive_encoded_snapshot(
    receiver: Receiver<Result<SnapshotEncodingResult, String>>,
) -> std::io::Result<SnapshotEncodingResult> {
    receiver
        .recv()
        .map_err(|_| Error::new(ErrorKind::BrokenPipe, "snapshot encoding service stopped"))?
        .map_err(|message| Error::new(ErrorKind::InvalidData, message))
}

pub(super) fn encode_websocket_message(request_id: &str, snapshot: &SnapshotEncodingResult) -> std::io::Result<String> {
    let request_id = serde_json::to_string(request_id).map_err(|error| {
        Error::new(
            ErrorKind::InvalidData,
            format!("failed to serialize snapshot request id: {error}"),
        )
    })?;
    Ok(format!(
        "{{\"kind\":\"snapshot\",\"request_id\":{request_id},\"snapshot\":{}}}",
        snapshot.encoded
    ))
}
