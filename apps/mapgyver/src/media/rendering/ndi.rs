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
    ndi::{NdiDiscovery, NdiReceiver},
};
use std::{
    collections::{HashMap, HashSet},
    num::NonZeroUsize,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

const MAX_RECEIVERS: usize = 8;
const CAPTURE_TIMEOUT: Duration = Duration::from_millis(25);
const RECEIVER_RETRY: Duration = Duration::from_millis(100);

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

struct CapacityWait {
    guard: RevisionGuard,
    _subscription: Option<RevisionSubscription>,
}

pub(super) struct Receivers {
    entries: HashMap<ChannelReference, Entry>,
    pool: RetirementPool,
    pending: Option<CapacityWait>,
    notify: Option<WorkerNotifier>,
}

impl Receivers {
    pub fn new(notify: Option<WorkerNotifier>) -> Self {
        // A replacement renderer shares admission with its still-retiring receivers.
        static POOL: OnceLock<RetirementPool> = OnceLock::new();
        Self {
            entries: HashMap::new(),
            pool: POOL
                .get_or_init(|| RetirementPool::new(NonZeroUsize::new(MAX_RECEIVERS).unwrap()))
                .clone(),
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
        let availability = self.pool.availability();
        let before = availability.current();
        self.entries.retain(|reference, entry| {
            wanted
                .iter()
                .any(|(candidate, sender)| candidate == reference && *sender == entry.sender.as_ref())
        });
        let mut pending = false;
        for (reference, sender) in wanted {
            if self.entries.contains_key(&reference) {
                continue;
            }
            let sender: Arc<str> = Arc::from(sender);
            let output = Arc::new(Mutex::new(Output::default()));
            let producer = Producer {
                sender: sender.clone(),
                output: output.clone(),
                notify: self.notify.clone(),
            };
            let worker = BoundedWorker::spawn(
                "mapgyver-ndi-receiver",
                &self.pool,
                WorkerLimits {
                    command_items: 1,
                    command_weight: 1,
                    event_items: 1,
                    event_weight: 1,
                },
                move |worker| producer.receive(&worker),
            );
            match worker {
                Ok(worker) => {
                    self.entries.insert(
                        reference,
                        Entry {
                            sender,
                            output,
                            _worker: worker,
                        },
                    );
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
                let stream = entry.output.lock().ok()?.stream.clone()?;
                Some((
                    reference,
                    Observation {
                        sender: entry.sender.clone(),
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
    notify: Option<WorkerNotifier>,
}

impl Producer {
    fn receive(self, worker: &WorkerContext<(), ()>) {
        let mut finder = None;
        let mut receiver: Option<NdiReceiver> = None;
        let mut error = None;
        let mut backoff = ReconnectBackoff::new(RECEIVER_RETRY, Duration::from_secs(1));
        while !worker.cancellation().is_cancelled() {
            let checkpoint = worker.wake_checkpoint();
            if let Some(session) = &mut receiver {
                match session.capture_video(CAPTURE_TIMEOUT) {
                    Ok(Some(image)) => {
                        if !self.publish(image) {
                            break;
                        }
                        error = None;
                        backoff.reset();
                    }
                    Ok(None) => {
                        if session.connection_count() == 0 {
                            // The SDK reconnects its exact bound source on return. A lost source
                            // withdraws its pixels while the existing receiver stays subscribed.
                            self.withdraw();
                        }
                    }
                    Err(failure) => {
                        self.report(&mut error, failure.to_string());
                        receiver = None;
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

    fn publish(&self, image: ImageData) -> bool {
        let mut output = self.output.lock().unwrap_or_else(|error| error.into_inner());
        if output.retired {
            return false;
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
        drop(output);
        if changed && let Some(notify) = &self.notify {
            notify.notify();
        }
        drop(retired);
        previous.is_ok()
    }

    fn withdraw(&self) {
        let mut output = self.output.lock().unwrap_or_else(|error| error.into_inner());
        let Some(stream) = output.stream.take() else { return };
        let retired = stream.replace(None);
        drop(output);
        drop(retired);
        if let Some(notify) = &self.notify {
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
