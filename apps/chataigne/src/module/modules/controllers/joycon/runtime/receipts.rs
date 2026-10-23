use super::*;

#[derive(Debug, PartialEq, Eq)]
pub(in super::super) enum Outcome {
    Completed(Result<String, String>),
    NotSent,
    Unconfirmed,
}

struct State {
    result: Mutex<Option<Outcome>>,
    complete: AtomicBool,
    started: AtomicBool,
    notifier: golden_io::WorkerNotifier,
    source: RetainedPayload<Source>,
}

pub(in super::super) struct Receipt(Arc<State>);
impl Receipt {
    pub(in super::super) fn attempted(&self) -> bool {
        self.0.started.load(Ordering::Acquire)
    }
    pub(in super::super) fn take(&self) -> Option<Outcome> {
        self.0.result.lock().unwrap_or_else(|error| error.into_inner()).take()
    }
    pub(in super::super) fn complete(&self) -> bool {
        self.0.complete.load(Ordering::Acquire)
    }
    pub(in super::super) fn source(&self) -> RetainedPayload<Source> {
        self.0.source.clone()
    }
}

pub(super) struct AdmittedCommand {
    state: Arc<State>,
    pub(super) armed: bool,
}
impl AdmittedCommand {
    pub(super) fn new(source: RetainedPayload<Source>, notifier: golden_io::WorkerNotifier) -> (Self, Receipt) {
        let state = Arc::new(State {
            result: Mutex::new(None),
            complete: AtomicBool::new(false),
            started: AtomicBool::new(false),
            notifier,
            source,
        });
        (
            Self {
                state: state.clone(),
                armed: true,
            },
            Receipt(state),
        )
    }
    pub(super) fn current(&self) -> bool {
        self.state.source.live()
    }
    pub(super) fn prepare(&self) -> Result<JoyConWorkerCommand, String> {
        self.state.source.prepare()
    }
    pub(super) fn started(&self) {
        self.state.started.store(true, Ordering::Release);
    }
    pub(super) fn finish(&self, result: Outcome) {
        let mut slot = self.state.result.lock().unwrap_or_else(|error| error.into_inner());
        if self.state.complete.load(Ordering::Acquire) {
            return;
        }
        *slot = Some(result);
        self.state.complete.store(true, Ordering::Release);
        drop(slot);
        self.state.notifier.notify();
    }
}
impl Drop for AdmittedCommand {
    fn drop(&mut self) {
        if self.armed && !self.state.complete.load(Ordering::Acquire) {
            self.finish(if self.state.started.load(Ordering::Acquire) {
                Outcome::Unconfirmed
            } else {
                Outcome::NotSent
            });
        }
    }
}
