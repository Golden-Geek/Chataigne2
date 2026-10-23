use super::*;
use golden_io::{bounded_pending_channel, PendingReceiver, PendingSender};
#[cfg(not(windows))]
use std::sync::mpsc::TryRecvError;

#[cfg(any(windows, test))]
mod broker;
#[cfg(not(windows))]
mod global;
#[cfg(windows)]
mod windows;
#[cfg(not(windows))]
use global::GlobalKeyboardInputRuntime as NativeInput;

pub(super) fn create() -> Result<impl Provider, String> {
    if cfg!(test) {
        return Err("keyboard native input is disabled in test builds".into());
    }
    #[cfg(windows)]
    return windows::create();
    #[cfg(not(windows))]
    NativeInput::create()
}

type Failure = Arc<NativeState>;
#[derive(Default)]
struct NativeState {
    error: Mutex<Option<String>>,
    stopped: AtomicBool,
}
#[cfg(any(not(windows), test))]
struct NativeStopped(Failure);
#[cfg(any(not(windows), test))]
impl Drop for NativeStopped {
    fn drop(&mut self) {
        self.0.stopped.store(true, Ordering::Release);
    }
}
trait Record {
    fn weight(&self) -> Result<usize, String>;
}
#[derive(Clone)]
struct Sender<E> {
    inner: PendingSender<E>,
    failure: Failure,
}
impl<E: Record> Sender<E> {
    fn send(&self, event: E) -> Result<(), String> {
        if self.failure.stopped.load(Ordering::Acquire) {
            return Err("keyboard native input has stopped".into());
        }
        let result = event.weight().and_then(|weight| {
            self.inner.send_weighted(event, weight).map_err(|error| match error {
                PendingSendError::Full(_) => "keyboard native input overload; event rejected".into(),
                PendingSendError::Disconnected(_) => "keyboard native input consumer retired".into(),
            })
        });
        if let Err(error) = &result {
            self.fail(error.clone());
        }
        result
    }
    fn fail(&self, error: String) {
        let mut end = error.len().min(1024);
        while !error.is_char_boundary(end) {
            end -= 1;
        }
        let mut slot = self.failure.error.lock().unwrap_or_else(|error| error.into_inner());
        if slot.is_none() {
            *slot = Some(error[..end].into());
        }
        self.failure.stopped.store(true, Ordering::Release);
    }
}
fn channel<E>() -> (Sender<E>, PendingReceiver<E>, Failure) {
    let (inner, receiver) = bounded_pending_channel(64, 2 * 1024 * 1024);
    let failure = Arc::new(NativeState::default());
    (
        Sender {
            inner,
            failure: failure.clone(),
        },
        receiver,
        failure,
    )
}
#[cfg(any(windows, test))]
fn catalog_weight(devices: &[DiscoveredKeyboardDevice]) -> Result<usize, String> {
    if devices.len() > MAX_DEVICES
        || devices.iter().any(|device| {
            [&device.variant_id, &device.label, &device.details]
                .iter()
                .any(|text| text.len() > MAX_DEVICE_TEXT)
        })
    {
        return Err("keyboard native catalog exceeds 64 devices / 1024 bytes per field".into());
    }
    Ok(CATALOG_WEIGHT / 4)
}
fn drain<E>(
    receiver: &PendingReceiver<E>,
    pending: &mut Vec<KeyboardRuntimeEvent>,
    failure: &Failure,
    convert: impl Fn(E) -> KeyboardRuntimeEvent,
) -> Result<Vec<KeyboardRuntimeEvent>, String> {
    let mut events = std::mem::take(pending);
    let mut raw = Vec::new();
    let result = receiver.drain_into(&mut raw, NonZeroUsize::new(ACTOR_TURN - events.len()).unwrap());
    events.extend(raw.into_iter().map(convert));
    if events.is_empty() && (result.state == PendingDrainState::Disconnected || failure.stopped.load(Ordering::Acquire))
    {
        return Err(failure
            .error
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .unwrap_or_else(|| "keyboard native input worker stopped unexpectedly".into()));
    }
    Ok(events)
}

#[cfg(test)]
mod tests;
