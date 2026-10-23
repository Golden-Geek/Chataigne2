use super::*;
#[cfg(any(windows, test))]
use golden_io::{bounded_pending_channel, PendingReceiver, PendingSender};
#[cfg(any(windows, test))]
mod broker;
#[cfg(not(windows))]
mod global;
#[cfg(windows)]
mod windows;
pub(super) fn create(config: MouseInputConfig, guard: RevisionGuard) -> Result<impl Provider, String> {
    if cfg!(test) {
        return Err("mouse native input is disabled in test builds".into());
    }
    #[cfg(windows)]
    return windows::create(config, guard);
    #[cfg(not(windows))]
    {
        let _ = guard;
        global::Global::create(config)
    }
}
#[cfg(any(windows, test))]
type Failure = Arc<NativeState>;
#[cfg(any(windows, test))]
#[derive(Default)]
struct NativeState {
    error: Mutex<Option<String>>,
    stopped: AtomicBool,
}
#[cfg(any(windows, test))]
trait Record {
    fn weight(&self) -> Result<usize, String>;
}
#[cfg(any(windows, test))]
#[derive(Clone)]
struct Sender<E> {
    inner: PendingSender<E>,
    failure: Failure,
}
#[cfg(any(windows, test))]
impl<E: Record> Sender<E> {
    fn send(&self, event: E) -> Result<(), String> {
        if self.failure.stopped.load(Ordering::Acquire) {
            return Err("mouse native input has stopped".into());
        }
        let result = event.weight().and_then(|weight| {
            self.inner.send_weighted(event, weight).map_err(|error| match error {
                PendingSendError::Full(_) => "mouse native input overload; event rejected".into(),
                PendingSendError::Disconnected(_) => "mouse native input consumer retired".into(),
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
#[cfg(any(windows, test))]
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
fn drain(receiver: &PendingReceiver<MouseRuntimeEvent>, failure: &Failure) -> Result<Vec<MouseRuntimeEvent>, String> {
    let mut events = Vec::new();
    let result = receiver.drain_into(&mut events, NonZeroUsize::new(ACTOR_TURN).unwrap());
    if events.is_empty() && (result.state == PendingDrainState::Disconnected || failure.stopped.load(Ordering::Acquire))
    {
        return Err(failure
            .error
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .unwrap_or_else(|| "mouse native input stopped unexpectedly".into()));
    }
    Ok(events)
}
#[cfg(all(test, windows))]
pub(crate) use windows::{capture_target_matches_public_selection, next_mouse_position_from_raw_input};
#[cfg(all(test, windows))]
pub(crate) fn should_swallow_captured_mouse_input(flags: u32, active: bool) -> bool {
    windows::should_swallow_low_level_mouse_event(flags, active)
}
