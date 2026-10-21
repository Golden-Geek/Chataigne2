use super::*;
use golden_io::{PendingDrain, PendingSendError, WorkerReceive};

pub(super) trait OutputConnection: 'static {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn is_present(&mut self) -> Result<bool, String>;
}
struct Packet {
    bytes: Vec<u8>,
    description: String,
    log: bool,
}
#[derive(Debug)]
pub(crate) enum MidiOutputEvent {
    Ready,
    Sent(Option<String>),
    Error(String),
}
impl MidiOutputEvent {
    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Sent(Some(s)) | Self::Error(s) => s.capacity(),
                _ => 0,
            }
    }
}
pub(crate) struct MidiOutputHandle {
    worker: BoundedWorker<Packet, MidiOutputEvent>,
    ready: Arc<AtomicBool>,
}
impl MidiOutputHandle {
    pub(crate) fn spawn(config: MidiOutputConfig, notifier: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_using(config, lifetimes(), notifier, native::open_output)
    }
    pub(super) fn spawn_using<G: OutputConnection>(
        config: MidiOutputConfig,
        pool: &RetirementPool,
        notifier: WorkerNotifier,
        open: impl FnOnce(&MidiOutputConfig) -> Result<G, String> + Send + 'static,
    ) -> Result<Self, String> {
        validate_selection(&config.port_variant)?;
        if config.port_variant.capacity() > 1024 {
            return Err("MIDI output selection exceeds 1024 retained bytes".into());
        }
        let ready = Arc::new(AtomicBool::new(false));
        let state = ready.clone();
        let status = notifier.clone();
        let worker = BoundedWorker::spawn_notified(
            "midi-output",
            pool,
            WorkerLimits {
                command_items: 256,
                command_weight: 2 * 1024 * 1024,
                event_items: 256,
                event_weight: 2 * 1024 * 1024,
            },
            notifier,
            move |ctx| {
                let mut connection = match open(&config) {
                    Ok(connection) => connection,
                    Err(error) => {
                        let e = MidiOutputEvent::Error(diagnostic(error));
                        let w = e.weight();
                        publish(&ctx, e, w);
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
                if !publish(&ctx, MidiOutputEvent::Ready, std::mem::size_of::<MidiOutputEvent>()) {
                    return;
                }
                let mut check_at = Instant::now() + Duration::from_secs(1);
                let mut failed = false;
                loop {
                    if ctx.cancellation().is_cancelled() {
                        break;
                    }
                    if !failed && !ctx.command_admission_closed() && Instant::now() >= check_at {
                        match connection.is_present() {
                            Ok(true) => check_at = Instant::now() + Duration::from_secs(1),
                            result => {
                                state.store(false, Ordering::Release);
                                status.notify();
                                ctx.close_command_admission();
                                failed = true;
                                let e = MidiOutputEvent::Error(diagnostic(
                                    result
                                        .err()
                                        .unwrap_or_else(|| "selected MIDI output disappeared".into()),
                                ));
                                let w = e.weight();
                                if !publish(&ctx, e, w) {
                                    break;
                                }
                            }
                        }
                    }
                    let command: Packet = match ctx.receive_until(check_at) {
                        WorkerReceive::Command(command) => command,
                        WorkerReceive::Deadline => continue,
                        WorkerReceive::Disconnected => break,
                    };
                    let event = if failed {
                        MidiOutputEvent::Error("accepted MIDI packet was not sent after output failure".into())
                    } else {
                        match connection.send(&command.bytes) {
                            Ok(()) => {
                                let log = command.log.then(|| {
                                    let shown = &command.bytes[..command.bytes.len().min(256)];
                                    format!(
                                        "Sent MIDI {} ({}{}); {} bytes",
                                        command.description,
                                        format_midi_bytes(shown),
                                        if shown.len() < command.bytes.len() { " ..." } else { "" },
                                        command.bytes.len()
                                    )
                                });
                                MidiOutputEvent::Sent(log)
                            }
                            Err(error) => {
                                state.store(false, Ordering::Release);
                                status.notify();
                                ctx.close_command_admission();
                                failed = true;
                                MidiOutputEvent::Error(format!(
                                    "{}; native outcome unconfirmed, packet is not replayed",
                                    diagnostic(error)
                                ))
                            }
                        }
                    };
                    let weight = event.weight();
                    if !publish(&ctx, event, weight) {
                        break;
                    }
                }
                state.store(false, Ordering::Release);
                status.notify();
                drop(connection);
            },
        )
        .map_err(|error| format!("failed to start MIDI output: {error}"))?;
        Ok(Self { worker, ready })
    }
    pub(crate) fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    pub(crate) fn send(&self, bytes: &[u8], description: &str, log: bool) -> Result<(), String> {
        if !self.is_ready() {
            return Err("MIDI output is not connected; packet was not admitted".into());
        }
        if bytes.len() > MESSAGE_BYTES || description.len() > 512 {
            return Err("MIDI packet exceeds 64 KiB / 512-byte description; packet was not admitted".into());
        }
        let packet = Packet {
            bytes: bytes.to_vec(),
            description: description.into(),
            log,
        };
        let weight = std::mem::size_of::<Packet>() + packet.bytes.capacity() + packet.description.capacity();
        self.worker.send_weighted(packet, weight).map_err(|error| match error {
            PendingSendError::Full(_) => "MIDI output capacity exhausted; packet was not admitted".into(),
            PendingSendError::Disconnected(_) => "MIDI output stopped; packet was not admitted".into(),
        })
    }
    pub(crate) fn drain(&self, events: &mut Vec<MidiOutputEvent>) -> PendingDrain {
        self.worker.drain_events(events, NonZeroUsize::new(TURN).unwrap())
    }
    pub(crate) fn finish_admission(&mut self) {
        self.worker.finish_admission();
    }
    pub(crate) fn stop(&self) {
        self.ready.store(false, Ordering::Release);
        self.worker.cancel();
    }
}
impl Drop for MidiOutputHandle {
    fn drop(&mut self) {
        self.stop();
    }
}
