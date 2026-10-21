use super::super::midi_input::{InputDraft, InputPreparer, INPUT_EVENT_BYTES};
use super::*;
use golden_io::{bounded_pending_channel, PendingSender};
use std::sync::atomic::AtomicU64;

pub(super) trait InputConnection: 'static {
    fn is_present(&mut self) -> Result<bool, String>;
}
struct RawInput {
    bytes: Vec<u8>,
    received_at: Duration,
}
pub(super) struct InputSink {
    sender: PendingSender<RawInput>,
    rejected: Arc<AtomicU64>,
    origin: Instant,
}
impl InputSink {
    // A driver callback cannot wait for downstream capacity. Admission is explicit:
    // oversize/full input is counted before ownership is accepted by the runtime.
    pub(super) fn push(&self, bytes: &[u8]) -> bool {
        if bytes.len() > MESSAGE_BYTES {
            self.reject();
            return false;
        }
        let event = RawInput {
            bytes: bytes.to_vec(),
            received_at: self.origin.elapsed(),
        };
        let weight = std::mem::size_of::<RawInput>() + event.bytes.capacity();
        if self.sender.send_weighted(event, weight).is_err() {
            self.reject();
            return false;
        }
        true
    }
    fn reject(&self) {
        let _ = self
            .rejected
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| Some(n.saturating_add(1)));
    }
}

pub(crate) struct MidiInputHandle {
    worker: BoundedWorker<(), MidiInputEvent>,
    ready: Arc<AtomicBool>,
    log: Arc<AtomicBool>,
}
impl MidiInputHandle {
    pub(crate) fn spawn(config: MidiInputConfig, notifier: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_using(config, lifetimes(), notifier, native::open_input)
    }
    pub(super) fn spawn_using<G: InputConnection>(
        config: MidiInputConfig,
        pool: &RetirementPool,
        notifier: WorkerNotifier,
        open: impl FnOnce(&MidiInputConfig, InputSink) -> Result<G, String> + Send + 'static,
    ) -> Result<Self, String> {
        validate_selection(&config.port_variant)?;
        if config.port_variant.capacity() > 1024 {
            return Err("MIDI input selection exceeds 1024 retained bytes".into());
        }
        let ready = Arc::new(AtomicBool::new(false));
        let state = ready.clone();
        let status = notifier.clone();
        let origin = Instant::now();
        let log = Arc::new(AtomicBool::new(false));
        let native_log = log.clone();
        let worker = BoundedWorker::spawn_notified(
            "midi-input",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 256,
                event_weight: INPUT_EVENT_BYTES,
            },
            notifier,
            move |ctx| {
                let preparer = match InputPreparer::new() {
                    Ok(preparer) => preparer,
                    Err(error) => {
                        let event = MidiInputEvent::Error(error);
                        let weight = event.weight();
                        publish(&ctx, event, weight);
                        return;
                    }
                };
                let (sender, receiver) = bounded_pending_channel(256, 2 * 1024 * 1024);
                let rejected = Arc::new(AtomicU64::new(0));
                let sink = InputSink {
                    sender,
                    rejected: rejected.clone(),
                    origin,
                };
                let mut connection = match open(&config, sink) {
                    Ok(connection) => connection,
                    Err(error) => {
                        let event = MidiInputEvent::Error(diagnostic(error));
                        let weight = event.weight();
                        publish(&ctx, event, weight);
                        return;
                    }
                };
                let _readiness = Readiness {
                    state: state.clone(),
                    notifier: status.clone(),
                };
                if ctx.cancellation().is_cancelled() {
                    return;
                }
                state.store(true, Ordering::Release);
                if !publish(&ctx, MidiInputEvent::Ready, std::mem::size_of::<MidiInputEvent>()) {
                    return;
                }
                let mut clock = clock::MidiClockAnalyzer::default();
                let mut check_at = Instant::now() + Duration::from_secs(1);
                let mut terminal = false;
                loop {
                    if ctx.cancellation().is_cancelled() {
                        break;
                    }
                    if !terminal && Instant::now() >= check_at {
                        match connection.is_present() {
                            Ok(true) => check_at = Instant::now() + Duration::from_secs(1),
                            result => {
                                state.store(false, Ordering::Release);
                                status.notify();
                                receiver.close();
                                let error = result.err().unwrap_or_else(|| "selected MIDI input disappeared".into());
                                let event = MidiInputEvent::Error(diagnostic(error));
                                let weight = event.weight();
                                if !publish(&ctx, event, weight) {
                                    break;
                                }
                                terminal = true;
                            }
                        }
                    }
                    let mut raw = Vec::with_capacity(TURN);
                    receiver.drain_into(&mut raw, NonZeroUsize::new(TURN).unwrap());
                    for raw in raw {
                        let clock_timing = clock.observe(&raw.bytes, raw.received_at);
                        let event = match decode_midi_message(&raw.bytes) {
                            Some(message) => {
                                let mut draft = match InputDraft::new(
                                    message,
                                    origin.checked_add(raw.received_at),
                                    clock_timing,
                                    native_log.load(Ordering::Acquire),
                                ) {
                                    Ok(draft) => draft,
                                    Err(error) => {
                                        let event = MidiInputEvent::Error(error);
                                        let weight = event.weight();
                                        if !publish(&ctx, event, weight) {
                                            return;
                                        }
                                        continue;
                                    }
                                };
                                let input = loop {
                                    if ctx.cancellation().is_cancelled() {
                                        return;
                                    }
                                    if let Some(input) = preparer.retain(&mut draft) {
                                        break input;
                                    }
                                    if ctx.cancellation().wait(Duration::from_millis(10)) {
                                        return;
                                    }
                                };
                                MidiInputEvent::Message(input)
                            }
                            None => MidiInputEvent::UnsupportedMessage(format!(
                                "Ignored unsupported MIDI message {}{}",
                                format_midi_bytes(&raw.bytes[..raw.bytes.len().min(256)]),
                                if raw.bytes.len() > 256 { " ..." } else { "" }
                            )),
                        };
                        let weight = event.weight();
                        if !publish(&ctx, event, weight) {
                            return;
                        }
                    }
                    let count = rejected.swap(0, Ordering::AcqRel);
                    if count != 0
                        && !publish(
                            &ctx,
                            MidiInputEvent::Rejected(count),
                            std::mem::size_of::<MidiInputEvent>(),
                        )
                    {
                        break;
                    }
                    if terminal && !receiver.has_pending() {
                        break;
                    }
                    if ctx.cancellation().wait(Duration::from_millis(10)) {
                        break;
                    }
                }
                state.store(false, Ordering::Release);
                status.notify();
                // Native callback shutdown and final queued raw storage stay on this worker.
                drop(connection);
            },
        )
        .map_err(|error| format!("failed to start MIDI input: {error}"))?;
        Ok(Self { worker, ready, log })
    }
    pub(crate) fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    pub(crate) fn set_log(&self, enabled: bool) {
        self.log.store(enabled, Ordering::Release);
    }
    pub(crate) fn try_recv(&self) -> Result<MidiInputEvent, std::sync::mpsc::TryRecvError> {
        let mut events = Vec::with_capacity(1);
        let drain = self.worker.drain_events(&mut events, NonZeroUsize::MIN);
        events.pop().ok_or(if drain.state == PendingDrainState::Disconnected {
            std::sync::mpsc::TryRecvError::Disconnected
        } else {
            std::sync::mpsc::TryRecvError::Empty
        })
    }
    pub(crate) fn stop(&mut self) {
        self.ready.store(false, Ordering::Release);
        self.worker.cancel();
    }
}
impl Drop for MidiInputHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
