use super::super::midi_output::{MidiOutputRequest, PreparedOutput};
use super::*;
use golden_io::{PayloadPool, PayloadReservation, PendingDrain, PendingSendError, RetirementPermit};
use std::sync::LazyLock;
mod schedule;
mod worker;

pub(super) trait OutputConnection: 'static {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String>;
    fn is_present(&mut self) -> Result<bool, String>;
}
struct AdmittedRequest {
    request: MidiOutputRequest,
    local: RetirementPermit,
    reservation: PayloadReservation,
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
    worker: BoundedWorker<AdmittedRequest, MidiOutputEvent>,
    ready: Arc<AtomicBool>,
    admission: RetirementPool,
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
            move |ctx| worker::run(ctx, config, state, status, open),
        )
        .map_err(|error| format!("failed to start MIDI output: {error}"))?;
        Ok(Self {
            worker,
            ready,
            admission: RetirementPool::new(NonZeroUsize::new(2048).unwrap()),
        })
    }
    pub(crate) fn has_pending(&self) -> bool {
        self.worker.has_pending()
    }
    pub(crate) fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
    pub(crate) fn enqueue(&self, request: MidiOutputRequest) -> Result<(), String> {
        if !self.is_ready() || !self.worker.is_accepting_commands() {
            return Err("MIDI output is not accepting commands; request was not admitted".into());
        }
        if !request.is_live() {
            return Err("MIDI source retired; request was not admitted".into());
        }
        let weight = request.reservation_weight();
        let local = self
            .admission
            .try_reserve()
            .map_err(|_| "MIDI output has 2048 live requests; request was not admitted")?;
        let reservation = request_pool()?
            .try_reserve(weight)
            .map_err(|_| "MIDI global output capacity exhausted; request was not admitted")?;
        self.worker
            .send_weighted(
                AdmittedRequest {
                    request,
                    local,
                    reservation,
                },
                weight,
            )
            .map_err(|error| match error {
                PendingSendError::Full(_) => "MIDI output capacity exhausted; request was not admitted".into(),
                PendingSendError::Disconnected(_) => "MIDI output stopped; request was not admitted".into(),
            })
    }
    #[cfg(test)]
    pub(crate) fn send(&self, bytes: &[u8], description: &str, log: bool) -> Result<(), String> {
        if bytes.len() > MESSAGE_BYTES || description.len() > 512 {
            return Err("MIDI packet exceeds 64 KiB / 512-byte description; packet was not admitted".into());
        }
        let mut request = MidiOutputRequest::for_test(crate::app::MidiSendRequest {
            packets: vec![crate::app::MidiSendPacket::immediate_bytes(bytes.to_vec(), description)],
        });
        request.log = log;
        self.enqueue(request)
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
fn request_pool() -> Result<&'static PayloadPool, String> {
    static POOL: LazyLock<Result<PayloadPool, std::io::Error>> =
        LazyLock::new(|| PayloadPool::new("midi-output-requests", 4096, 128 * 1024 * 1024));
    POOL.as_ref()
        .map_err(|error| format!("MIDI output admission unavailable: {error}"))
}
