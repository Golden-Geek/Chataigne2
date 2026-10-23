use super::*;
use golden_io::{BoundedWorker, RevisionReader, RevisionSource, WorkerContext, WorkerLimits};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, OnceLock,
};

pub(super) trait Provider: 'static {
    fn execute(&mut self, request: Request) -> Result<String, String>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Request {
    pub(super) key: KeyboardKey,
    pub(super) action: KeyboardKeyAction,
}
pub(super) struct Shared {
    running: AtomicBool,
    ready: AtomicBool,
    error: Mutex<Option<String>>,
    pub(super) native: RevisionReader,
    pub(super) notifier: WorkerNotifier,
}
struct Stopped {
    shared: Arc<Shared>,
    native: Option<RevisionSource>,
}
impl Drop for Stopped {
    fn drop(&mut self) {
        self.native.take();
        self.shared.ready.store(false, Ordering::Release);
        self.shared.running.store(false, Ordering::Release);
        self.shared.notifier.notify();
    }
}
type Context = WorkerContext<Command, ()>;
struct Owned<P> {
    _stopped: Stopped,
    context: Context,
    provider: P,
    _owner: golden_io::RetirementPermit,
}
pub(super) struct Runtime {
    pub(super) worker: BoundedWorker<Command, ()>,
    pub(super) actor: Option<RevisionSource>,
    pub(super) shared: Arc<Shared>,
    pub(super) pool: golden_io::PayloadPool,
    pub(super) coordinator: Arc<coordination::Coordinator>,
}
fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(4).unwrap()))
}
fn sources() -> Result<golden_io::PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<golden_io::PayloadPool>>> = OnceLock::new();
    let mut slot = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if slot.is_none() {
        *slot = Some(
            golden_io::PayloadPool::new("keyboard-output-sources", 512, 32 * 1024 * 1024)
                .map_err(|error| error.to_string())?,
        );
    }
    Ok(slot.as_ref().unwrap().clone())
}
impl Runtime {
    pub(super) fn create(owner: &RetirementPool, notifier: WorkerNotifier) -> Result<Self, String> {
        Self::spawn_shared_using(
            owner,
            lifetimes(),
            sources()?,
            notifier,
            coordination::Coordinator::process(),
            native::KeyboardOutputController::create,
        )
    }
    #[cfg(test)]
    pub(super) fn spawn_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: golden_io::PayloadPool,
        notifier: WorkerNotifier,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::spawn_shared_using(owner, slots, pool, notifier, coordination::Coordinator::new(), open)
    }
    pub(super) fn spawn_shared_using<P: Provider>(
        owner: &RetirementPool,
        slots: &RetirementPool,
        pool: golden_io::PayloadPool,
        notifier: WorkerNotifier,
        coordinator: Arc<coordination::Coordinator>,
        open: impl FnOnce() -> Result<P, String> + Send + 'static,
    ) -> Result<Self, String> {
        let permit = owner.try_reserve().map_err(|error| error.to_string())?;
        let native = RevisionSource::default();
        let shared = Arc::new(Shared {
            running: AtomicBool::new(true),
            ready: AtomicBool::new(false),
            error: Mutex::new(None),
            native: native.reader(),
            notifier: notifier.clone(),
        });
        let state = shared.clone();
        let worker = BoundedWorker::spawn_notified(
            "keyboard-output-owner",
            slots,
            WorkerLimits {
                command_items: 64,
                command_weight: 4 * 1024 * 1024,
                event_items: 1,
                event_weight: 1,
            },
            notifier,
            move |context| {
                let stopped = Stopped {
                    shared: state,
                    native: Some(native),
                };
                let provider = match open() {
                    Ok(provider) => provider,
                    Err(error) => {
                        *stopped.shared.error.lock().unwrap_or_else(|error| error.into_inner()) = Some(bounded(error));
                        return;
                    }
                };
                stopped.shared.ready.store(true, Ordering::Release);
                stopped.shared.notifier.notify();
                run(Owned {
                    _stopped: stopped,
                    context,
                    provider,
                    _owner: permit,
                });
            },
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            worker,
            actor: Some(RevisionSource::default()),
            shared,
            pool,
            coordinator,
        })
    }
    pub(super) fn is_running(&self) -> bool {
        self.shared.running.load(Ordering::Acquire)
    }
    pub(super) fn is_ready(&self) -> bool {
        self.shared.ready.load(Ordering::Acquire)
    }
    pub(super) fn status_guards(&self) -> [RevisionGuard; 2] {
        [
            self.actor.as_ref().expect("live runtime handle").reader().guard_at(0),
            self.shared.native.guard_at(0),
        ]
    }
    pub(super) fn error(&self) -> String {
        self.shared
            .error
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
            .unwrap_or_else(|| "Keyboard output worker stopped unexpectedly".into())
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        self.actor.take();
        self.worker.cancel();
    }
}
fn bounded(value: String) -> String {
    let mut end = value.len().min(1024);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}
fn run<P: Provider>(mut owned: Owned<P>) {
    while let Some(command) = owned.context.receive() {
        if owned.context.cancellation().is_cancelled() || !command.current() {
            command.finish(Outcome::NotSent);
            continue;
        }
        let request = match command.prepare() {
            Ok(request) => request,
            Err(error) => {
                command.finish(Outcome::Completed(Err(bounded(error))));
                continue;
            }
        };
        if owned.context.cancellation().is_cancelled() || !command.current() {
            command.finish(Outcome::NotSent);
            continue;
        }
        let Some(_permit) = command.acquire(|| owned.context.cancellation().is_cancelled()) else {
            command.finish(Outcome::NotSent);
            continue;
        };
        if owned.context.cancellation().is_cancelled() || !command.current() {
            command.finish(Outcome::NotSent);
            continue;
        }
        command.started();
        command.finish(Outcome::Completed(
            owned.provider.execute(request).map(bounded).map_err(bounded),
        ));
    }
}
