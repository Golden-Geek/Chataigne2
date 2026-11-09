//! Demand-owned local file workers. Decode, conversion and waits never run on the engine/UI.
use super::{VideoControl, reader::Reader};
use super::{
    failure::Failure,
    protocol::{VideoSourceError, VideoSourceErrorKind},
};
use crate::media::{
    capture::CapturedChannels,
    protocol::{ChannelReference, ChannelSource},
};
use golden_io::{
    BoundedWorker, RetirementPool, RevisionGuard, RevisionSubscription, WorkerContext, WorkerLimits, WorkerNotifier,
    WorkerStartError,
};
use golden_video::{
    decode::{DecodeLimits, Decoder},
    image::ImageStream,
};
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

const MAX_SOURCES: usize = 8;
const MAX_BYTES: u64 = 768 * 1024 * 1024;
// The measured software GPU bridge improves HD cadence, but its extra shared-image
// transfer lowers 4K throughput. Keep large software rasters on the direct upload path
// until the unchanged 4K workload qualifies the bridge; this does not waive that gate.
#[cfg(all(windows, feature = "native-video"))]
const MAX_SOFTWARE_GPU_PIXELS: u64 = 1920 * 1080;
#[cfg(all(windows, feature = "native-video"))]
mod hardware;
mod pacing;
#[cfg(all(windows, feature = "native-video"))]
#[derive(Clone)]
pub(crate) struct Hardware {
    pub adapter: golden_video::native::AdapterId,
    pub budget: golden_video::native::GpuFrameBudget,
}
#[derive(Clone)]
enum Stream {
    Software(ImageStream),
    #[cfg(all(windows, feature = "native-video"))]
    Hardware(golden_video::native::GpuFrameStream),
}
impl Stream {
    fn source(&self) -> (golden_video::TextureDescriptor, golden_video::plan::ExternalSource) {
        match self {
            Self::Software(stream) => (
                stream.descriptor(),
                golden_video::plan::ExternalSource::Frames(stream.clone()),
            ),
            #[cfg(all(windows, feature = "native-video"))]
            Self::Hardware(stream) => (
                stream.descriptor(),
                golden_video::plan::ExternalSource::GpuFrames(stream.clone()),
            ),
        }
    }
    fn withdraw(&self) {
        match self {
            Self::Software(stream) => {
                let _ = stream.publish(None);
            }
            #[cfg(all(windows, feature = "native-video"))]
            Self::Hardware(stream) => {
                let _ = stream.publish(None);
            }
        }
    }
}
#[derive(Default)]
struct Budget {
    used: Mutex<u64>,
    idle: Condvar,
}
struct Lease {
    budget: Arc<Budget>,
    bytes: u64,
}
impl Drop for Lease {
    fn drop(&mut self) {
        *self.budget.used.lock().unwrap() -= self.bytes;
        self.budget.idle.notify_all();
    }
}
impl Budget {
    fn reserve(self: &Arc<Self>, pixels: u64) -> Result<Lease, String> {
        // Two retained decoded frames, scaler storage, latest RGBA and an upload observer.
        // This bounds our admitted storage, not every internal libavcodec allocation.
        let bytes = pixels
            .checked_mul(40)
            .and_then(|bytes| bytes.checked_add(16 * 1024 * 1024))
            .ok_or("Video storage overflow")?;
        self.reserve_bytes(bytes)
    }
    fn reserve_bytes(self: &Arc<Self>, bytes: u64) -> Result<Lease, String> {
        let mut used = self.used.lock().map_err(|_| "Video storage budget poisoned")?;
        if bytes > MAX_BYTES.saturating_sub(*used) {
            return Err("Video file storage capacity retained".into());
        }
        *used += bytes;
        Ok(Lease {
            budget: self.clone(),
            bytes,
        })
    }
}
#[derive(Default)]
struct Output {
    stream: Option<Stream>,
    error: Option<String>,
}
struct Entry {
    file: PathBuf,
    binding: crate::media::protocol::AssetLocator,
    control: Arc<VideoControl>,
    output: Arc<Mutex<Output>>,
    _worker: BoundedWorker<(), ()>,
}
struct CapacityWait {
    guard: RevisionGuard,
    _subscription: Option<RevisionSubscription>,
}
pub(crate) struct Files {
    #[cfg(all(windows, feature = "native-video"))]
    hardware: Option<Hardware>,
    root: PathBuf,
    project: Option<uuid::Uuid>,
    entries: HashMap<ChannelReference, Entry>,
    pool: RetirementPool,
    budget: Arc<Budget>,
    changes: Arc<golden_io::RevisionSource>,
    observed: u64,
    capacity_wait: Option<CapacityWait>,
    notifier: Option<WorkerNotifier>,
}
/// Completion of file-worker cancellation and all retained decoded-image storage.
#[derive(Clone)]
pub struct FileRetirement {
    #[cfg(all(windows, feature = "native-video"))]
    hardware: Option<Hardware>,
    pool: RetirementPool,
    budget: Arc<Budget>,
}
impl FileRetirement {
    /// Workers retain their slot until decoder cleanup has finished.
    pub fn active_workers(&self) -> usize {
        self.pool.metrics().active
    }
    /// Admitted CPU/GPU source storage, including old pools and render observers.
    pub fn retained_bytes(&self) -> u64 {
        let bytes = *self.budget.used.lock().unwrap();
        #[cfg(all(windows, feature = "native-video"))]
        let bytes = bytes
            + self
                .hardware
                .as_ref()
                .map_or(0, |hardware| hardware.budget.retained_bytes());
        bytes
    }
    /// Wait on a shutdown/test owner, never an engine or UI callback.
    pub fn wait(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        if !self.pool.wait_for_idle(timeout) {
            return false;
        }
        let used = self.budget.used.lock().unwrap();
        let (used, _) = self
            .budget
            .idle
            .wait_timeout_while(used, deadline.saturating_duration_since(Instant::now()), |used| {
                *used != 0
            })
            .unwrap();
        if *used != 0 {
            return false;
        }
        drop(used);
        while self.retained_bytes() != 0 {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        true
    }
}
impl Files {
    pub fn retirement(&self) -> FileRetirement {
        FileRetirement {
            #[cfg(all(windows, feature = "native-video"))]
            hardware: self.hardware.clone(),
            pool: self.pool.clone(),
            budget: self.budget.clone(),
        }
    }
    pub fn new(root: &Path, notifier: Option<WorkerNotifier>) -> Self {
        Self {
            #[cfg(all(windows, feature = "native-video"))]
            hardware: None,
            root: root.into(),
            project: None,
            entries: Default::default(),
            pool: RetirementPool::new(NonZeroUsize::new(MAX_SOURCES).unwrap()),
            budget: Default::default(),
            changes: Default::default(),
            observed: 0,
            capacity_wait: None,
            notifier,
        }
    }
    #[cfg(all(windows, feature = "native-video"))]
    pub fn set_hardware(&mut self, hardware: Hardware) {
        self.hardware = Some(hardware);
    }
    pub fn set_root(&mut self, root: &Path) -> bool {
        if self.root == root {
            return false;
        }
        self.entries.clear();
        self.root = root.into();
        true
    }
    pub fn sync(&mut self, capture: &CapturedChannels, demanded: &[ChannelReference]) -> Result<(), String> {
        // Capture before cancellation or admission: a release between failure and subscription
        // must invalidate this guard and wake immediately rather than strand the replacement.
        let availability = self.pool.availability();
        let available_at = availability.current();
        if self.project != Some(capture.project) {
            self.entries.clear();
            self.project = Some(capture.project);
        }
        let demanded: HashSet<_> = demanded.iter().copied().collect();
        let mut wanted = HashMap::new();
        for channel in capture
            .channels
            .iter()
            .filter(|channel| channel.enabled && demanded.contains(&channel.reference))
        {
            if let Some(ChannelSource::Video { file: Some(file) }) = channel.definition.as_ref().map(|d| &d.source) {
                let control = channel.video.clone().ok_or("Video capture has no transport owner")?;
                wanted.insert(channel.reference, (file.resolve(&self.root), control, file.clone()));
            }
        }
        if wanted.len() > MAX_SOURCES {
            return Err("Video demand exceeds eight admitted file workers".into());
        }
        self.entries.retain(|reference, entry| {
            wanted.get(reference).is_some_and(|(file, control, binding)| {
                *file == entry.file && *binding == entry.binding && Arc::ptr_eq(control, &entry.control)
            })
        });
        let mut pending = false;
        for (reference, (file, control, binding)) in wanted {
            if self.entries.contains_key(&reference) {
                continue;
            }
            let output = Arc::new(Mutex::new(Output::default()));
            let context = Run {
                #[cfg(all(windows, feature = "native-video"))]
                hardware: self.hardware.clone(),
                owner: uuid::Uuid::new_v4(),
                file: file.clone(),
                reference,
                control: control.clone(),
                output: output.clone(),
                budget: self.budget.clone(),
                changes: self.changes.clone(),
                notifier: self.notifier.clone(),
            };
            let owner = context.owner;
            control.claim_channel(reference.channel, owner, binding.clone())?;
            let worker = match BoundedWorker::spawn(
                "mapgyver-video-file",
                &self.pool,
                WorkerLimits {
                    command_items: 1,
                    command_weight: 1,
                    event_items: 1,
                    event_weight: 1,
                },
                move |worker| context.run(worker),
            ) {
                Ok(worker) => worker,
                Err(WorkerStartError::Capacity(_)) => {
                    // Retiring workers keep their slots. The desired capture remains admitted,
                    // with this source preparing until actual retirement makes a slot available.
                    pending = true;
                    continue;
                }
                Err(error) => {
                    control.release_channel(reference.channel, owner);
                    return Err(error.to_string());
                }
            };
            self.entries.insert(
                reference,
                Entry {
                    file,
                    binding,
                    control,
                    output,
                    _worker: worker,
                },
            );
        }
        self.capacity_wait = pending.then(|| {
            let guard = availability.guard_at(available_at);
            let subscription = self.notifier.as_ref().map(|notifier| guard.subscribe(notifier.clone()));
            CapacityWait {
                guard,
                _subscription: subscription,
            }
        });
        Ok(())
    }
    pub fn source(
        &self,
        reference: ChannelReference,
    ) -> Option<(golden_video::TextureDescriptor, golden_video::plan::ExternalSource)> {
        Some(
            self.entries
                .get(&reference)?
                .output
                .lock()
                .ok()?
                .stream
                .as_ref()?
                .source(),
        )
    }
    #[cfg(test)]
    pub fn stream(&self, reference: ChannelReference) -> Option<ImageStream> {
        match self.entries.get(&reference)?.output.lock().ok()?.stream.as_ref()? {
            Stream::Software(stream) => Some(stream.clone()),
            #[cfg(all(windows, feature = "native-video"))]
            Stream::Hardware(_) => None,
        }
    }
    pub fn service(&mut self) -> bool {
        let revision = self.changes.reader().current();
        let changed = self.observed != revision
            || self
                .capacity_wait
                .as_ref()
                .is_some_and(|waiting| !waiting.guard.is_current());
        self.observed = revision;
        changed
    }
}
struct Run {
    #[cfg(all(windows, feature = "native-video"))]
    hardware: Option<Hardware>,
    owner: uuid::Uuid,
    file: PathBuf,
    reference: ChannelReference,
    control: Arc<VideoControl>,
    output: Arc<Mutex<Output>>,
    budget: Arc<Budget>,
    changes: Arc<golden_io::RevisionSource>,
    notifier: Option<WorkerNotifier>,
}
impl Run {
    fn changed(&self) {
        self.changes.advance();
        if let Some(wake) = &self.notifier {
            wake.notify();
        }
    }
    fn withdraw(&self, error: Option<String>) {
        let mut output = self.output.lock().unwrap();
        let changed = output.error != error || output.stream.is_some();
        let stream = output.stream.take();
        output.error = error;
        drop(output);
        if let Some(stream) = stream {
            stream.withdraw();
        }
        if changed {
            self.changed();
        }
    }
    fn run(self, mut worker: WorkerContext<(), ()>) {
        while !worker.cancellation().is_cancelled() {
            let result = worker
                .enable_precise_deadlines()
                .map_err(|error| Failure(VideoSourceError::new(VideoSourceErrorKind::Io, error.to_string())))
                .and_then(|()| self.play(&worker, self.owner));
            if let Err(error) = result {
                let _ = self
                    .control
                    .source_failure(self.reference.channel, self.owner, error.0.clone());
                self.withdraw(Some(error.0.message));
                let checkpoint = worker.wake_checkpoint();
                worker.wait_until(checkpoint, Instant::now() + Duration::from_secs(1));
            }
        }
    }
    fn play(&self, worker: &WorkerContext<(), ()>, owner: uuid::Uuid) -> Result<(), Failure> {
        #[cfg(all(windows, feature = "native-video"))]
        if let Some(hardware) = &self.hardware {
            match self.play_hardware(worker, owner, hardware) {
                Ok(()) => return Ok(()),
                Err(hardware::NativeFailure::Retry(error)) => return Err(error),
                Err(hardware::NativeFailure::Fallback(error)) => {
                    let _ = self
                        .control
                        .source_failure(self.reference.channel, owner, error.0.clone());
                    self.withdraw(None);
                    eprintln!(
                        "[mapgyver-video] software fallback for {}: {}",
                        self.file.display(),
                        error.0.message
                    );
                }
            }
        }
        self.play_software(worker, owner)
    }
    fn play_software(&self, worker: &WorkerContext<(), ()>, owner: uuid::Uuid) -> Result<(), Failure> {
        let cancel = worker.cancellation().clone();
        let decoder = Decoder::open_with_interrupt(
            &self.file,
            DecodeLimits {
                max_pixels: 16384 * 16384,
                max_packet_bytes: 16 * 1024 * 1024,
                max_packets_per_frame: 256,
                software_threads: golden_video::decode::SoftwareDecodeThreads::Four,
            },
            move || cancel.is_cancelled(),
        )?;
        if std::env::var_os("MAPGYVER_NATIVE_DIAGNOSTICS").is_some_and(|value| !value.is_empty())
            && let Some(threads) = decoder.software_decode_threads()
        {
            let info = decoder.info();
            eprintln!(
                "[mapgyver-video] software decode: {} {}x{} threads={threads}",
                info.codec, info.width, info.height
            );
        }
        let lease = Arc::new(
            self.budget
                .reserve(u64::from(decoder.info().width) * u64::from(decoder.info().height))
                .map_err(|error| Failure(VideoSourceError::new(VideoSourceErrorKind::Capacity, error)))?,
        );
        #[cfg(all(windows, feature = "native-video"))]
        if let Some(hardware) = &self.hardware
            && u64::from(decoder.info().width) * u64::from(decoder.info().height) <= MAX_SOFTWARE_GPU_PIXELS
        {
            return self.drive_native(worker, owner, hardware, decoder, Some(lease));
        }
        let mut reader = Reader::new(decoder)?;
        self.control
            .channel_duration(self.reference.channel, owner, reader.duration)?;
        let mut stream: Option<ImageStream> = None;
        while !worker.cancellation().is_cancelled() {
            let turn_started = Instant::now();
            let checkpoint = worker.wake_checkpoint();
            let revision = self.control.revisions();
            let _subscription = revision.guard_at(revision.current()).subscribe(worker.wake_notifier());
            let sample = self.control.sample()?;
            if let Some(image) = reader.poll(sample.channel_position(reader.duration))? {
                let image = image.retain(lease.clone());
                let next = stream.get_or_insert_with(|| ImageStream::new(image.descriptor()));
                if self.control.publish_frame(
                    self.reference.channel,
                    owner,
                    sample,
                    next,
                    image,
                    reader.neighbours()?.ok_or("Selected Video timestamps unavailable")?,
                )? {
                    let mut output = self.output.lock().unwrap();
                    if output.stream.is_none() {
                        output.stream = Some(Stream::Software(next.clone()));
                        output.error = None;
                        drop(output);
                        self.changed();
                    }
                } else {
                    reader.invalidate_publication();
                }
            }
            if let Some(frame) = reader.neighbours()? {
                self.control
                    .observe_neighbours(self.reference.channel, owner, sample, frame)?;
            }
            if !sample.playing {
                reader.prepare_previous()?;
            }
            worker.wait_until(
                checkpoint,
                pacing::deadline(turn_started, sample.playing, reader.catching_up),
            );
        }
        Ok(())
    }
}
impl Drop for Run {
    fn drop(&mut self) {
        self.withdraw(None);
        self.control.release_channel(self.reference.channel, self.owner);
    }
}
