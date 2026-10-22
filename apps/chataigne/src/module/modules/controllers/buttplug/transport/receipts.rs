use super::*;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) enum CommandStatus {
    Completed(Result<String, String>),
    NotSent,
    Unconfirmed,
}
struct State {
    result: Mutex<Option<CommandStatus>>,
    complete: AtomicBool,
    started: AtomicBool,
    notifier: WorkerNotifier,
    _source: RetainedPayload<ButtplugWorkerCommand>,
}
pub(in super::super) struct CommandReceipt(Arc<State>);
impl CommandReceipt {
    pub(in super::super) fn take(&self) -> Option<CommandStatus> {
        self.0.result.lock().unwrap_or_else(|error| error.into_inner()).take()
    }
    pub(in super::super) fn complete(&self) -> bool {
        self.0.complete.load(Ordering::Acquire)
    }
}
pub(super) struct AdmittedCommand {
    pub(super) source: RetainedPayload<ButtplugWorkerCommand>,
    state: Arc<State>,
    admitted: bool,
}
impl AdmittedCommand {
    pub(super) fn new(
        source: RetainedPayload<ButtplugWorkerCommand>,
        notifier: WorkerNotifier,
    ) -> (Self, CommandReceipt) {
        let state = Arc::new(State {
            result: Mutex::new(None),
            complete: AtomicBool::new(false),
            started: AtomicBool::new(false),
            notifier,
            _source: source.clone(),
        });
        (
            Self {
                source,
                state: state.clone(),
                admitted: false,
            },
            CommandReceipt(state),
        )
    }
    pub(super) fn arm(&mut self) {
        self.admitted = true;
    }
    pub(super) fn disarm(&mut self) {
        self.admitted = false;
    }
    pub(super) fn started(&self) {
        self.state.started.store(true, Ordering::Release);
    }
    pub(super) fn finish(&self, result: CommandStatus) {
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
        if self.admitted && !self.state.complete.load(Ordering::Acquire) {
            self.finish(if self.state.started.load(Ordering::Acquire) {
                CommandStatus::Unconfirmed
            } else {
                CommandStatus::NotSent
            });
        }
    }
}
