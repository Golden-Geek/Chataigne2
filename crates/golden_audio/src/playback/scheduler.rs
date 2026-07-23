use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, TrySendError, sync_channel},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{
    AudioEngineConfig, AudioError, AudioErrorCategory, CommandSequence, EngineLimits, PlayFileRequest, PlaybackId,
    SampleRate, assert_not_realtime,
};

use super::{
    AssetCache, AudioFileProbe, AudioSourceFingerprint, ResidentAssetKey, ResidentAudioAsset, StreamPlaybackReader,
    StreamPlaybackWriter, decoder::DecodingSession, decoder::decode_session_cancellable, resample::StreamingResampler,
    streaming_playback_ring,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackSchedulerConfig {
    pub engine_sample_rate: SampleRate,
    pub resident_asset_threshold_bytes: u64,
    pub resident_cache_budget_bytes: u64,
    pub stream_ring_frames: usize,
    pub worker_count: u16,
    pub job_capacity: usize,
    pub result_capacity: usize,
}

impl PlaybackSchedulerConfig {
    #[must_use]
    pub fn from_engine(config: &AudioEngineConfig, limits: &EngineLimits) -> Self {
        Self {
            engine_sample_rate: config.sample_rate,
            resident_asset_threshold_bytes: limits.resident_asset_threshold_bytes,
            resident_cache_budget_bytes: limits.resident_cache_budget_bytes,
            stream_ring_frames: limits.stream_ring_frames as usize,
            worker_count: limits.decoder_worker_count,
            job_capacity: usize::from(limits.max_voices),
            result_capacity: usize::from(limits.max_voices),
        }
    }

    pub fn validate(self) -> Result<(), AudioError> {
        if self.worker_count == 0
            || self.stream_ring_frames == 0
            || self.job_capacity == 0
            || self.result_capacity == 0
            || self.resident_asset_threshold_bytes == 0
            || self.resident_cache_budget_bytes == 0
            || self.resident_asset_threshold_bytes > self.resident_cache_budget_bytes
        {
            return Err(AudioError::invalid_configuration(
                "playback scheduler capacities and worker count must be positive and cache threshold must fit its budget",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackSchedulerRequest {
    pub sequence: CommandSequence,
    pub request: PlayFileRequest,
}

#[derive(Debug)]
pub enum PlaybackPreparation {
    Resident(Arc<ResidentAudioAsset>),
    Stream {
        probe: AudioFileProbe,
        reader: StreamPlaybackReader,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackPreparationFailure {
    pub sequence: CommandSequence,
    pub playback_id: PlaybackId,
    pub path: PathBuf,
    pub error: AudioError,
}

#[derive(Debug)]
pub enum PlaybackPreparationResult {
    Prepared {
        sequence: CommandSequence,
        request: PlayFileRequest,
        preparation: PlaybackPreparation,
    },
    Failed(PlaybackPreparationFailure),
}

impl PlaybackPreparationResult {
    #[must_use]
    pub const fn sequence(&self) -> CommandSequence {
        match self {
            Self::Prepared { sequence, .. } => *sequence,
            Self::Failed(failure) => failure.sequence,
        }
    }

    #[must_use]
    pub fn playback_id(&self) -> &PlaybackId {
        match self {
            Self::Prepared { request, .. } => &request.playback_id,
            Self::Failed(failure) => &failure.playback_id,
        }
    }
}

#[derive(Debug)]
struct WorkerRequest {
    scheduled: PlaybackSchedulerRequest,
    cancellation: Arc<AtomicBool>,
}

#[derive(Debug)]
struct ActiveRequest {
    sequence: CommandSequence,
    cancellation: Arc<AtomicBool>,
}

#[derive(Debug)]
pub struct PlaybackScheduler {
    requests: Option<SyncSender<WorkerRequest>>,
    results: Receiver<PlaybackPreparationResult>,
    active: HashMap<PlaybackId, ActiveRequest>,
    cache: Arc<Mutex<AssetCache>>,
    shutdown: Arc<AtomicBool>,
    workers: Vec<JoinHandle<()>>,
}

impl PlaybackScheduler {
    pub fn new(config: PlaybackSchedulerConfig) -> Result<Self, AudioError> {
        assert_not_realtime("playback scheduler creation");
        config.validate()?;
        let cache = Arc::new(Mutex::new(AssetCache::new(
            config.resident_asset_threshold_bytes,
            config.resident_cache_budget_bytes,
        )?));
        let (request_sender, request_receiver) = sync_channel(config.job_capacity);
        let request_receiver = Arc::new(Mutex::new(request_receiver));
        let (result_sender, result_receiver) = sync_channel(config.result_capacity);
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut workers = Vec::with_capacity(usize::from(config.worker_count));
        for index in 0..config.worker_count {
            let requests = Arc::clone(&request_receiver);
            let results = result_sender.clone();
            let worker_cache = Arc::clone(&cache);
            let worker_shutdown = Arc::clone(&shutdown);
            let worker = thread::Builder::new()
                .name(format!("golden-audio-decoder-{index}"))
                .spawn(move || run_decoder_worker(config, requests, results, worker_cache, worker_shutdown))
                .map_err(|error| {
                    shutdown.store(true, Ordering::Release);
                    AudioError::new(
                        AudioErrorCategory::InternalInvariant,
                        format!("failed to start playback decoder worker {index}: {error}"),
                    )
                })?;
            workers.push(worker);
        }
        drop(result_sender);
        Ok(Self {
            requests: Some(request_sender),
            results: result_receiver,
            active: HashMap::new(),
            cache,
            shutdown,
            workers,
        })
    }

    pub fn try_schedule(&mut self, scheduled: PlaybackSchedulerRequest) -> Result<(), AudioError> {
        assert_not_realtime("playback decode scheduling");
        if let Some(previous) = self.active.remove(&scheduled.request.playback_id) {
            previous.cancellation.store(true, Ordering::Release);
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        let request = WorkerRequest {
            scheduled: scheduled.clone(),
            cancellation: Arc::clone(&cancellation),
        };
        let Some(sender) = &self.requests else {
            return Err(AudioError::shutting_down());
        };
        match sender.try_send(request) {
            Ok(()) => {
                self.active.insert(
                    scheduled.request.playback_id,
                    ActiveRequest {
                        sequence: scheduled.sequence,
                        cancellation,
                    },
                );
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(AudioError::queue_full("playback decoder job")),
            Err(TrySendError::Disconnected(_)) => Err(AudioError::shutting_down()),
        }
    }

    pub fn stop(&mut self, playback_id: &PlaybackId) -> bool {
        assert_not_realtime("playback decode cancellation");
        let Some(active) = self.active.remove(playback_id) else {
            return false;
        };
        active.cancellation.store(true, Ordering::Release);
        true
    }

    pub fn stop_all(&mut self) -> usize {
        assert_not_realtime("playback decoder stop all");
        let count = self.active.len();
        for (_, active) in self.active.drain() {
            active.cancellation.store(true, Ordering::Release);
        }
        count
    }

    pub fn complete(&mut self, playback_id: &PlaybackId, sequence: CommandSequence) -> bool {
        let Some(active) = self.active.get(playback_id) else {
            return false;
        };
        if active.sequence != sequence {
            return false;
        }
        self.active.remove(playback_id);
        true
    }

    pub fn try_recv(&mut self) -> Option<PlaybackPreparationResult> {
        assert_not_realtime("playback preparation result consumption");
        loop {
            let result = self.results.try_recv().ok()?;
            let is_current = self
                .active
                .get(result.playback_id())
                .is_some_and(|active| active.sequence == result.sequence());
            if !is_current {
                continue;
            }
            if matches!(result, PlaybackPreparationResult::Failed(_)) {
                self.active.remove(result.playback_id());
            }
            return Some(result);
        }
    }

    #[must_use]
    pub fn cache_observation(&self) -> super::CacheObservation {
        self.cache
            .lock()
            .map_or_else(|_| super::CacheObservation::default(), |cache| cache.observation())
    }

    #[must_use]
    pub fn active_request_count(&self) -> usize {
        self.active.len()
    }

    pub fn shutdown(&mut self) -> Result<(), AudioError> {
        if self.workers.is_empty() {
            return Ok(());
        }
        self.shutdown.store(true, Ordering::Release);
        self.requests.take();
        for worker in self.workers.drain(..) {
            worker.join().map_err(|_| {
                AudioError::new(
                    AudioErrorCategory::InternalInvariant,
                    "playback decoder worker panicked during shutdown",
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for PlaybackScheduler {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn run_decoder_worker(
    config: PlaybackSchedulerConfig,
    requests: Arc<Mutex<Receiver<WorkerRequest>>>,
    results: SyncSender<PlaybackPreparationResult>,
    cache: Arc<Mutex<AssetCache>>,
    shutdown: Arc<AtomicBool>,
) {
    let mut streams = Vec::<ActiveStream>::new();
    let mut pending_results = VecDeque::new();
    while !shutdown.load(Ordering::Acquire) {
        flush_results(&results, &mut pending_results);
        if pending_results.len() < config.result_capacity
            && let Some(request) = try_receive(&requests, streams.is_empty())
            && !request.cancellation.load(Ordering::Acquire)
        {
            match prepare_request(config, request, &cache) {
                WorkerPreparation::Result(result) => pending_results.push_back(result),
                WorkerPreparation::Stream(stream) => streams.push(stream),
                WorkerPreparation::Cancelled => {}
            }
        }

        let mut index = 0;
        while index < streams.len() {
            let outcome = streams[index].pump(config.stream_ring_frames / 2);
            match outcome {
                StreamPump::Keep => index += 1,
                StreamPump::Result(result) => {
                    pending_results.push_back(result);
                    index += 1;
                }
                StreamPump::Finished => {
                    streams.swap_remove(index);
                }
                StreamPump::ResultAndFinished(result) => {
                    pending_results.push_back(result);
                    streams.swap_remove(index);
                }
            }
        }
        if streams.is_empty() {
            thread::yield_now();
        }
    }
}

fn try_receive(requests: &Mutex<Receiver<WorkerRequest>>, may_wait: bool) -> Option<WorkerRequest> {
    let Ok(receiver) = requests.lock() else {
        return None;
    };
    if may_wait {
        receiver.recv_timeout(Duration::from_millis(5)).ok()
    } else {
        receiver.try_recv().ok()
    }
}

fn flush_results(results: &SyncSender<PlaybackPreparationResult>, pending: &mut VecDeque<PlaybackPreparationResult>) {
    while let Some(result) = pending.pop_front() {
        match results.try_send(result) {
            Ok(()) => {}
            Err(TrySendError::Full(result)) => {
                pending.push_front(result);
                break;
            }
            Err(TrySendError::Disconnected(_)) => {
                pending.clear();
                break;
            }
        }
    }
}

enum WorkerPreparation {
    Result(PlaybackPreparationResult),
    Stream(ActiveStream),
    Cancelled,
}

fn prepare_request(
    config: PlaybackSchedulerConfig,
    request: WorkerRequest,
    cache: &Mutex<AssetCache>,
) -> WorkerPreparation {
    let path = request.scheduled.request.path.clone();
    let source = match AudioSourceFingerprint::from_path(&path) {
        Ok(source) => source,
        Err(error) => return WorkerPreparation::Result(failure(&request.scheduled, error)),
    };
    let session = match DecodingSession::open(&path) {
        Ok(session) => session,
        Err(error) => return WorkerPreparation::Result(failure(&request.scheduled, error)),
    };
    let key = ResidentAssetKey {
        source: source.clone(),
        track: session.probe.track,
        engine_sample_rate: config.engine_sample_rate,
    };
    if let Ok(mut cache) = cache.lock()
        && let Some(asset) = cache.get(&key)
    {
        return WorkerPreparation::Result(PlaybackPreparationResult::Prepared {
            sequence: request.scheduled.sequence,
            request: request.scheduled.request,
            preparation: PlaybackPreparation::Resident(asset),
        });
    }
    let resident = session
        .probe
        .estimated_decoded_bytes(config.engine_sample_rate)
        .is_some_and(|bytes| bytes <= config.resident_asset_threshold_bytes);
    if resident {
        match decode_session_cancellable(session, source, config.engine_sample_rate, || {
            request.cancellation.load(Ordering::Acquire)
        }) {
            Ok(Some(asset)) => {
                if let Ok(mut cache) = cache.lock() {
                    cache.insert(Arc::clone(&asset));
                }
                WorkerPreparation::Result(PlaybackPreparationResult::Prepared {
                    sequence: request.scheduled.sequence,
                    request: request.scheduled.request,
                    preparation: PlaybackPreparation::Resident(asset),
                })
            }
            Ok(None) => WorkerPreparation::Cancelled,
            Err(error) => WorkerPreparation::Result(failure(&request.scheduled, error)),
        }
    } else {
        match ActiveStream::new(config, request, session) {
            Ok(stream) => WorkerPreparation::Stream(stream),
            Err(error) => {
                let (scheduled, error) = *error;
                WorkerPreparation::Result(failure(&scheduled, error))
            }
        }
    }
}

fn failure(scheduled: &PlaybackSchedulerRequest, error: AudioError) -> PlaybackPreparationResult {
    PlaybackPreparationResult::Failed(PlaybackPreparationFailure {
        sequence: scheduled.sequence,
        playback_id: scheduled.request.playback_id.clone(),
        path: scheduled.request.path.clone(),
        error,
    })
}

#[derive(Debug)]
struct ActiveStream {
    scheduled: Option<PlaybackSchedulerRequest>,
    cancellation: Arc<AtomicBool>,
    session: DecodingSession,
    writer: StreamPlaybackWriter,
    reader: Option<StreamPlaybackReader>,
    resampler: StreamingResampler,
    pending: Vec<f32>,
    pending_offset: usize,
    source_ended: bool,
}

impl ActiveStream {
    fn new(
        config: PlaybackSchedulerConfig,
        request: WorkerRequest,
        session: DecodingSession,
    ) -> Result<Self, Box<(PlaybackSchedulerRequest, AudioError)>> {
        let channels = session.probe.channels;
        let (writer, reader) = streaming_playback_ring(channels, config.stream_ring_frames)
            .map_err(|error| Box::new((request.scheduled.clone(), error)))?;
        Ok(Self {
            scheduled: Some(request.scheduled),
            cancellation: request.cancellation,
            resampler: StreamingResampler::new(channels, session.probe.sample_rate, config.engine_sample_rate),
            session,
            writer,
            reader: Some(reader),
            pending: Vec::new(),
            pending_offset: 0,
            source_ended: false,
        })
    }

    fn pump(&mut self, prime_frames: usize) -> StreamPump {
        if self.cancellation.load(Ordering::Acquire) || self.writer.is_cancelled() || !self.writer.is_reader_connected()
        {
            return StreamPump::Finished;
        }
        self.flush_pending();
        if self.pending_offset < self.pending.len() {
            return self.announce_if_primed(prime_frames);
        }
        if self.source_ended {
            self.writer.finish();
            return match self.take_prepared_result() {
                Some(result) => StreamPump::ResultAndFinished(result),
                None => StreamPump::Finished,
            };
        }
        match self.session.next_planar_chunk() {
            Ok(Some(planes)) => {
                if let Err(error) = self.resampler.process(&planes, &mut self.pending) {
                    return self.fail(error);
                }
                self.pending_offset = 0;
                self.flush_pending();
                self.announce_if_primed(prime_frames)
            }
            Ok(None) => {
                self.resampler.finish(&mut self.pending);
                self.pending_offset = 0;
                self.source_ended = true;
                self.flush_pending();
                if self.pending_offset >= self.pending.len() {
                    self.writer.finish();
                    match self.take_prepared_result() {
                        Some(result) => StreamPump::ResultAndFinished(result),
                        None => StreamPump::Finished,
                    }
                } else {
                    self.announce_if_primed(prime_frames)
                }
            }
            Err(error) => self.fail(error),
        }
    }

    fn flush_pending(&mut self) {
        if self.pending_offset >= self.pending.len() {
            self.pending.clear();
            self.pending_offset = 0;
            return;
        }
        let channels = usize::from(self.session.probe.channels);
        let remaining_frames = (self.pending.len() - self.pending_offset) / channels;
        let frames = remaining_frames.min(self.writer.writable_frames());
        if frames == 0 {
            return;
        }
        let samples = frames * channels;
        let end = self.pending_offset + samples;
        if self
            .writer
            .write_interleaved(&self.pending[self.pending_offset..end])
            .is_ok()
        {
            self.pending_offset = end;
        }
    }

    fn announce_if_primed(&mut self, prime_frames: usize) -> StreamPump {
        if self.reader.is_some() && self.writer.state().fill_frames >= prime_frames.max(1) {
            self.take_prepared_result().map_or(StreamPump::Keep, StreamPump::Result)
        } else {
            StreamPump::Keep
        }
    }

    fn take_prepared_result(&mut self) -> Option<PlaybackPreparationResult> {
        let scheduled = self.scheduled.take()?;
        let reader = self.reader.take()?;
        Some(PlaybackPreparationResult::Prepared {
            sequence: scheduled.sequence,
            request: scheduled.request,
            preparation: PlaybackPreparation::Stream {
                probe: self.session.probe.clone(),
                reader,
            },
        })
    }

    fn fail(&mut self, error: AudioError) -> StreamPump {
        self.writer.fail();
        let Some(scheduled) = self.scheduled.take() else {
            return StreamPump::Finished;
        };
        StreamPump::ResultAndFinished(failure(&scheduled, error))
    }
}

enum StreamPump {
    Keep,
    Result(PlaybackPreparationResult),
    Finished,
    ResultAndFinished(PlaybackPreparationResult),
}
