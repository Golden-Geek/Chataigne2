//! One bounded NDI receiver per demanded channel. SDK calls stay on the receiver worker.
use crate::media::{
    capture::CapturedChannels,
    protocol::{ChannelReference, ChannelSource},
};
use golden_io::{
    BoundedWorker, ReconnectBackoff, RetirementPool, RevisionGuard, RevisionSubscription, WorkerContext, WorkerLimits,
    WorkerNotifier, WorkerStartError,
};
use golden_video::{
    image::{ImageData, ImageStream},
    ndi::{NdiAudioFrame, NdiDiscovery, NdiError, NdiFrame, NdiReceiver},
};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    num::NonZeroUsize,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::{Duration, Instant},
};

mod timing;

const MAX_RECEIVERS: usize = 8;
const CAPTURE_TIMEOUT: Duration = Duration::from_millis(25);
const RECEIVER_RETRY: Duration = Duration::from_millis(100);
const AUDIO_PACKETS: usize = 4;

pub(super) type Observations = HashMap<ChannelReference, Observation>;

#[derive(Clone, Debug)]
pub(super) struct Observation {
    pub sender: Arc<str>,
    pub stream: ImageStream,
}

impl PartialEq for Observation {
    fn eq(&self, other: &Self) -> bool {
        self.sender == other.sender && self.stream.id() == other.stream.id()
    }
}

#[derive(Default)]
struct Output {
    stream: Option<ImageStream>,
    video_users: usize,
    notify: Option<WorkerNotifier>,
    audio_users: usize,
    audio_connected: bool,
    audio: VecDeque<(Instant, NdiAudioFrame)>,
    audio_epoch: u64,
    retired: bool,
}

struct Entry {
    sender: Arc<str>,
    output: Arc<Mutex<Output>>,
    _worker: BoundedWorker<(), ()>,
}

impl Drop for Entry {
    fn drop(&mut self) {
        // Withdraw before cancelling: a late SDK return cannot republish into a retired plan.
        let mut output = self.output.lock().unwrap_or_else(|error| error.into_inner());
        output.retired = true;
        let displaced = output
            .stream
            .take()
            .and_then(|stream| stream.replace(None).ok().flatten());
        drop(output);
        drop(displaced);
    }
}

type ReceiverEntries = HashMap<(ChannelReference, Arc<str>), Weak<Entry>>;

/// One project-owned receiver directory for both the video and audio consumers.
#[derive(Default)]
pub(crate) struct Inputs {
    entries: Mutex<ReceiverEntries>,
    #[cfg(feature = "video-files")]
    bindings: Mutex<std::collections::BTreeMap<uuid::Uuid, Weak<crate::audio::voice::Binding>>>,
}

fn pool() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(MAX_RECEIVERS).unwrap()))
}

impl Inputs {
    fn acquire(&self, reference: ChannelReference, sender: &str) -> Result<Arc<Entry>, WorkerStartError> {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|_, entry| entry.strong_count() != 0);
        let sender: Arc<str> = Arc::from(sender);
        let key = (reference, sender.clone());
        if let Some(entry) = entries.get(&key).and_then(Weak::upgrade) {
            return Ok(entry);
        }
        let output = Arc::new(Mutex::new(Output::default()));
        let producer = Producer {
            sender: sender.clone(),
            output: output.clone(),
        };
        let worker = BoundedWorker::spawn(
            "mapgyver-ndi-receiver",
            pool(),
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1,
            },
            move |worker| producer.receive(&worker),
        )?;
        let entry = Arc::new(Entry {
            sender,
            output,
            _worker: worker,
        });
        entries.insert(key, Arc::downgrade(&entry));
        Ok(entry)
    }

    #[cfg(feature = "video-files")]
    pub(crate) fn audio(&self, reference: ChannelReference, sender: &str) -> Result<AudioInput, WorkerStartError> {
        let entry = self.acquire(reference, sender)?;
        entry.output.lock().unwrap().audio_users += 1;
        Ok(AudioInput(entry))
    }

    #[cfg(feature = "video-files")]
    pub(crate) fn register_audio(&self, uuid: uuid::Uuid, binding: &Arc<crate::audio::voice::Binding>) {
        self.bindings.lock().unwrap().insert(uuid, Arc::downgrade(binding));
    }

    #[cfg(feature = "video-files")]
    pub(crate) fn audio_bindings(&self) -> Vec<(uuid::Uuid, Arc<crate::audio::voice::Binding>)> {
        let mut bindings = self.bindings.lock().unwrap();
        bindings.retain(|_, binding| binding.strong_count() != 0);
        bindings
            .iter()
            .filter_map(|(&id, binding)| binding.upgrade().map(|binding| (id, binding)))
            .collect()
    }
}

#[cfg(feature = "video-files")]
pub(crate) struct AudioInput(Arc<Entry>);

#[cfg(feature = "video-files")]
impl AudioInput {
    pub(crate) fn drain(&self, packets: &mut VecDeque<(Instant, NdiAudioFrame)>) -> u64 {
        let mut output = self.0.output.lock().unwrap();
        packets.append(&mut output.audio);
        output.audio_epoch
    }
}

#[cfg(feature = "video-files")]
impl Drop for AudioInput {
    fn drop(&mut self) {
        let mut output = self.0.output.lock().unwrap();
        output.audio_users -= 1;
        if output.audio_users == 0 {
            output.audio.clear();
            output.audio_connected = false;
        }
    }
}

struct VideoInput(Arc<Entry>);

impl VideoInput {
    fn new(entry: Arc<Entry>, notify: Option<WorkerNotifier>) -> Self {
        let mut output = entry.output.lock().unwrap();
        output.video_users += 1;
        output.notify = notify;
        drop(output);
        Self(entry)
    }
}

impl Drop for VideoInput {
    fn drop(&mut self) {
        let mut output = self.0.output.lock().unwrap();
        output.video_users -= 1;
        if output.video_users == 0 {
            if let Some(stream) = output.stream.take() {
                let _ = stream.replace(None);
            }
            output.notify = None;
        }
    }
}

struct CapacityWait {
    guard: RevisionGuard,
    _subscription: Option<RevisionSubscription>,
}

pub(super) struct Receivers {
    entries: HashMap<ChannelReference, VideoInput>,
    pending: Option<CapacityWait>,
    notify: Option<WorkerNotifier>,
}

impl Receivers {
    pub fn new(notify: Option<WorkerNotifier>) -> Self {
        Self {
            entries: HashMap::new(),
            pending: None,
            notify,
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.pending = None;
    }

    pub fn sync(&mut self, capture: &CapturedChannels, demanded: &[ChannelReference]) -> Result<(), String> {
        let requested: HashSet<_> = demanded.iter().copied().collect();
        let wanted: Vec<_> = capture
            .channels
            .iter()
            .filter(|channel| channel.enabled && requested.contains(&channel.reference))
            .filter_map(
                |channel| match channel.definition.as_ref().map(|definition| &definition.source) {
                    Some(ChannelSource::Ndi { sender: Some(sender) }) => Some((channel.reference, sender.as_str())),
                    _ => None,
                },
            )
            .take(MAX_RECEIVERS)
            .collect();
        let availability = pool().availability();
        let before = availability.current();
        self.entries.retain(|reference, entry| {
            wanted
                .iter()
                .any(|(candidate, sender)| candidate == reference && *sender == entry.0.sender.as_ref())
        });
        let mut pending = false;
        for (reference, sender) in wanted {
            if self.entries.contains_key(&reference) {
                continue;
            }
            match capture.ndi_inputs.acquire(reference, sender) {
                Ok(entry) => {
                    self.entries
                        .insert(reference, VideoInput::new(entry, self.notify.clone()));
                }
                Err(WorkerStartError::Capacity(_)) => pending = true,
                Err(error) => return Err(error.to_string()),
            }
        }
        self.pending = pending.then(|| {
            let guard = availability.guard_at(before);
            let subscription = self.notify.as_ref().map(|notify| guard.subscribe(notify.clone()));
            CapacityWait {
                guard,
                _subscription: subscription,
            }
        });
        Ok(())
    }

    pub fn ready_to_retry(&self) -> bool {
        self.pending.as_ref().is_some_and(|pending| !pending.guard.is_current())
    }

    pub fn observations(&self) -> Observations {
        self.entries
            .iter()
            .filter_map(|(&reference, entry)| {
                let stream = entry.0.output.lock().ok()?.stream.clone()?;
                Some((
                    reference,
                    Observation {
                        sender: entry.0.sender.clone(),
                        stream,
                    },
                ))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;

struct Producer {
    sender: Arc<str>,
    output: Arc<Mutex<Output>>,
}

impl Producer {
    fn receive(self, worker: &WorkerContext<(), ()>) {
        let mut finder = None;
        let mut receiver: Option<NdiReceiver> = None;
        let mut error = None;
        let mut backoff = ReconnectBackoff::new(RECEIVER_RETRY, Duration::from_secs(1));
        let mut clock = timing::Clock::default();
        let mut pending_video = VecDeque::<(Instant, ImageData)>::new();
        while !worker.cancellation().is_cancelled() {
            let checkpoint = worker.wake_checkpoint();
            if let Some(session) = &mut receiver {
                let (video, audio, synchronized) = {
                    let output = self.output.lock().unwrap();
                    (output.video_users != 0, output.audio_users != 0, output.audio_connected)
                };
                if !audio {
                    clock = timing::Clock::default();
                }
                if !video || !audio {
                    pending_video.clear();
                }
                let now = Instant::now();
                let mut due = None;
                while pending_video.front().is_some_and(|(at, _)| *at <= now) {
                    due = pending_video.pop_front();
                }
                if let Some((_, image)) = due
                    && !self.publish(image)
                {
                    break;
                }
                let timeout = pending_video.front().map_or(CAPTURE_TIMEOUT, |(at, _)| {
                    at.saturating_duration_since(now).min(CAPTURE_TIMEOUT)
                });
                let result = match (video, audio) {
                    (true, true) => session.capture_av(timeout),
                    (true, false) => match session.capture_video(CAPTURE_TIMEOUT) {
                        Ok(Some(image)) => {
                            if !self.publish(image) {
                                break;
                            }
                            error = None;
                            backoff.reset();
                            continue;
                        }
                        Ok(None) => Ok(None),
                        Err(failure) => Err(failure),
                    },
                    (false, true) => session
                        .capture_audio(CAPTURE_TIMEOUT)
                        .map(|audio| audio.map(NdiFrame::Audio)),
                    (false, false) => {
                        worker.wait_until(checkpoint, Instant::now() + CAPTURE_TIMEOUT);
                        continue;
                    }
                };
                match result {
                    Ok(Some(NdiFrame::Video(frame))) => {
                        if synchronized {
                            let (at, reset) = clock.playout(frame.timing, Instant::now());
                            if reset {
                                pending_video.clear();
                                self.reset_audio();
                            }
                            if pending_video.len() == 8 {
                                pending_video.pop_front();
                            }
                            pending_video.push_back((at, frame.image));
                        } else if !self.publish(frame.image) {
                            break;
                        }
                        error = None;
                        backoff.reset();
                    }
                    Ok(Some(NdiFrame::Audio(frame))) => {
                        let (at, reset) = clock.playout(frame.timing(), Instant::now());
                        if reset {
                            pending_video.clear();
                            self.reset_audio();
                        }
                        let mut output = self.output.lock().unwrap();
                        if output.audio_users != 0 {
                            output.audio_connected = true;
                            if output.audio.len() == AUDIO_PACKETS {
                                output.audio.pop_front();
                            }
                            output.audio.push_back((at, frame));
                        }
                        error = None;
                        backoff.reset();
                    }
                    Ok(None) => {
                        if session.connection_count() == 0 {
                            // The SDK reconnects its exact bound source on return. A lost source
                            // withdraws its pixels while the existing receiver stays subscribed.
                            pending_video.clear();
                            clock = timing::Clock::default();
                            self.withdraw();
                        }
                    }
                    Err(failure @ NdiError::AudioFrame(_)) => self.report(&mut error, failure.to_string()),
                    Err(failure) => {
                        self.report(&mut error, failure.to_string());
                        receiver = None;
                        pending_video.clear();
                        clock = timing::Clock::default();
                        self.withdraw();
                        worker.wait_until(checkpoint, backoff.schedule(Instant::now()));
                    }
                }
                continue;
            }
            if finder.is_none() {
                match NdiDiscovery::new() {
                    Ok(discovery) => finder = Some(discovery),
                    Err(failure) => self.report(&mut error, failure.to_string()),
                }
            }
            if let Some(discovery) = &mut finder
                && let Some(source) = discovery
                    .sources()
                    .into_iter()
                    .find(|source| source.name() == self.sender.as_ref())
            {
                match NdiReceiver::connect(&source) {
                    Ok(connected) => {
                        receiver = Some(connected);
                        continue;
                    }
                    Err(failure) => self.report(&mut error, failure.to_string()),
                }
            }
            worker.wait_until(checkpoint, backoff.schedule(Instant::now()));
        }
        self.withdraw();
    }

    fn reset_audio(&self) {
        let mut output = self.output.lock().unwrap();
        output.audio.clear();
        output.audio_epoch = output.audio_epoch.wrapping_add(1);
    }

    fn publish(&self, image: ImageData) -> bool {
        let mut output = self.output.lock().unwrap_or_else(|error| error.into_inner());
        if output.retired {
            return false;
        }
        if output.video_users == 0 {
            return true;
        }
        let descriptor = image.descriptor();
        let changed = output
            .stream
            .as_ref()
            .is_none_or(|stream| stream.descriptor() != descriptor);
        let retired = if changed {
            output
                .stream
                .replace(ImageStream::new(descriptor))
                .and_then(|stream| stream.replace(None).ok().flatten())
        } else {
            None
        };
        let previous = output.stream.as_ref().expect("stream installed").replace(Some(image));
        let notify = output.notify.clone();
        drop(output);
        if changed && let Some(notify) = notify {
            notify.notify();
        }
        drop(retired);
        previous.is_ok()
    }

    fn withdraw(&self) {
        let mut output = self.output.lock().unwrap_or_else(|error| error.into_inner());
        let retired = output.stream.take().map(|stream| stream.replace(None));
        if output.audio_connected {
            output.audio_connected = false;
            output.audio.clear();
            output.audio_epoch = output.audio_epoch.wrapping_add(1);
        }
        let notify = output.notify.clone();
        drop(output);
        let changed = retired.is_some();
        drop(retired);
        if changed && let Some(notify) = notify {
            notify.notify();
        }
    }

    fn report(&self, previous: &mut Option<String>, error: String) {
        if previous.as_ref() != Some(&error) {
            eprintln!("[mapgyver-ndi] {}: {error}", self.sender);
            *previous = Some(error);
        }
    }
}
