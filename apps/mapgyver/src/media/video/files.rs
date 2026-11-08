//! Demand-owned local file workers. Decode, conversion and waits never run on the engine/UI.
use super::{VideoControl, reader::Reader};
use crate::media::{
    capture::CapturedChannels,
    protocol::{ChannelReference, ChannelSource},
};
use golden_io::{BoundedWorker, RetirementPool, WorkerContext, WorkerLimits, WorkerNotifier};
use golden_video::{
    decode::{DecodeLimits, SoftwareDecoder},
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
    stream: Option<ImageStream>,
    error: Option<String>,
}
struct Entry {
    file: PathBuf,
    binding: crate::media::protocol::AssetLocator,
    control: Arc<VideoControl>,
    output: Arc<Mutex<Output>>,
    _worker: BoundedWorker<(), ()>,
}
pub(crate) struct Files {
    root: PathBuf,
    project: Option<uuid::Uuid>,
    entries: HashMap<ChannelReference, Entry>,
    pool: RetirementPool,
    budget: Arc<Budget>,
    changes: Arc<golden_io::RevisionSource>,
    observed: u64,
    notifier: Option<WorkerNotifier>,
}
/// Completion of file-worker cancellation and all retained decoded-image storage.
#[derive(Clone)]
pub struct FileRetirement {
    pool: RetirementPool,
    budget: Arc<Budget>,
}
impl FileRetirement {
    /// Workers retain their slot until decoder cleanup has finished.
    pub fn active_workers(&self) -> usize {
        self.pool.metrics().active
    }
    /// Admitted software storage, including images retained by old render observers.
    pub fn retained_bytes(&self) -> u64 {
        *self.budget.used.lock().unwrap()
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
        *used == 0
    }
}
impl Files {
    pub fn retirement(&self) -> FileRetirement {
        FileRetirement {
            pool: self.pool.clone(),
            budget: self.budget.clone(),
        }
    }
    pub fn new(root: &Path, notifier: Option<WorkerNotifier>) -> Self {
        Self {
            root: root.into(),
            project: None,
            entries: Default::default(),
            pool: RetirementPool::new(NonZeroUsize::new(MAX_SOURCES).unwrap()),
            budget: Default::default(),
            changes: Default::default(),
            observed: 0,
            notifier,
        }
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
        for (reference, (file, control, binding)) in wanted {
            if self.entries.contains_key(&reference) {
                continue;
            }
            let output = Arc::new(Mutex::new(Output::default()));
            let context = Run {
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
            let worker = BoundedWorker::spawn(
                "mapgyver-video-file",
                &self.pool,
                WorkerLimits {
                    command_items: 1,
                    command_weight: 1,
                    event_items: 1,
                    event_weight: 1,
                },
                move |worker| context.run(worker),
            )
            .map_err(|e| {
                control.release_channel(reference.channel, owner);
                e.to_string()
            })?;
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
        Ok(())
    }
    pub fn stream(&self, reference: ChannelReference) -> Option<ImageStream> {
        self.entries.get(&reference)?.output.lock().ok()?.stream.clone()
    }
    pub fn service(&mut self) -> bool {
        let revision = self.changes.reader().current();
        let changed = self.observed != revision;
        self.observed = revision;
        changed
    }
}
struct Run {
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
        if let Some(stream) = output.stream.take() {
            let _ = stream.publish(None);
        }
        output.error = error;
        drop(output);
        if changed {
            self.changed();
        }
    }
    fn run(self, worker: WorkerContext<(), ()>) {
        while !worker.cancellation().is_cancelled() {
            if let Err(error) = self.play(&worker, self.owner) {
                self.withdraw(Some(error.chars().take(256).collect()));
                let checkpoint = worker.wake_checkpoint();
                worker.wait_until(checkpoint, Instant::now() + Duration::from_secs(1));
            }
        }
    }
    fn play(&self, worker: &WorkerContext<(), ()>, owner: uuid::Uuid) -> Result<(), String> {
        let cancel = worker.cancellation().clone();
        let decoder = SoftwareDecoder::open_with_interrupt(
            &self.file,
            DecodeLimits {
                max_pixels: 16384 * 16384,
                max_packet_bytes: 16 * 1024 * 1024,
                max_packets_per_frame: 256,
            },
            move || cancel.is_cancelled(),
        )
        .map_err(|e| e.to_string())?;
        let lease = Arc::new(
            self.budget
                .reserve(u64::from(decoder.info().width) * u64::from(decoder.info().height))?,
        );
        let mut reader = Reader::new(decoder)?;
        self.control
            .channel_duration(self.reference.channel, owner, reader.duration)?;
        let mut stream: Option<ImageStream> = None;
        while !worker.cancellation().is_cancelled() {
            let checkpoint = worker.wake_checkpoint();
            let revision = self.control.revisions();
            let _subscription = revision.guard_at(revision.current()).subscribe(worker.wake_notifier());
            let sample = self.control.sample()?;
            if let Some(image) = reader.poll(sample.channel_position(reader.duration))? {
                let image = image.retain(lease.clone());
                let next = stream.get_or_insert_with(|| ImageStream::new(image.descriptor()));
                if self
                    .control
                    .publish_frame(self.reference.channel, owner, sample, next, image)?
                {
                    let mut output = self.output.lock().unwrap();
                    if output.stream.is_none() {
                        output.stream = Some(next.clone());
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
            let delay = if reader.catching_up {
                Duration::from_millis(1)
            } else if sample.playing {
                Duration::from_millis(8)
            } else {
                Duration::from_secs(3600)
            };
            worker.wait_until(checkpoint, Instant::now() + delay);
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
