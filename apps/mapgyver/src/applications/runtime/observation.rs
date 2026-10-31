//! App-owned metadata publication. Neither status changes nor subscriber wakes poll the actor.

use super::*;
use golden_io::WorkerContext;
use std::time::Duration;

#[derive(Clone)]
pub struct ApplicationObservations {
    project: uuid::Uuid,
    guard: RevisionGuard,
    view: RuntimeView,
}
pub struct ApplicationSubscription {
    owner: uuid::Uuid,
    view: RuntimeView,
    _worker: BoundedWorker<(), ()>,
}
impl ApplicationObservations {
    pub(super) fn new(project: uuid::Uuid, guard: RevisionGuard, view: RuntimeView) -> Self {
        Self { project, guard, view }
    }
    pub fn project(&self) -> uuid::Uuid {
        self.project
    }
    pub fn is_current(&self) -> bool {
        self.guard.is_current()
    }
    /// One host publisher per project; UI clients share its stream. Call outside the actor.
    /// Sink must return promptly. Drop cancels without joining; blocked sinks keep a lifetime slot.
    /// Full snapshots coalesce at 10 Hz; they are observations, not a reliable transition journal.
    pub fn subscribe(
        &self,
        sink: impl Fn(ApplicationRuntimeSnapshot) + Send + 'static,
    ) -> Result<ApplicationSubscription, String> {
        if !self.guard.is_current() {
            return Err("application observation project is retired".into());
        }
        let mut observer = self.view.observer.lock().expect("application observer lock poisoned");
        if observer.is_some() {
            return Err("application observation publisher is already owned".into());
        }
        let owner = uuid::Uuid::new_v4();
        *observer = Some((owner, WorkerNotifier::new(|| {})));
        drop(observer);
        static POOL: OnceLock<RetirementPool> = OnceLock::new();
        let pool = POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(2).unwrap()));
        let source = self.clone();
        let worker = match BoundedWorker::spawn(
            "mapgyver-app-observations",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1,
            },
            move |context| source.run(context, sink),
        ) {
            Ok(worker) => worker,
            Err(error) => {
                self.view
                    .observer
                    .lock()
                    .expect("application observer lock poisoned")
                    .take();
                return Err(error.to_string());
            }
        };
        let notifier = worker.wake_notifier();
        *self.view.observer.lock().expect("application observer lock poisoned") = Some((owner, notifier.clone()));
        notifier.notify();
        Ok(ApplicationSubscription {
            owner,
            view: self.view.clone(),
            _worker: worker,
        })
    }
    fn run(&self, context: WorkerContext<(), ()>, sink: impl Fn(ApplicationRuntimeSnapshot)) {
        let mut last = None;
        let mut next = Instant::now();
        while !context.cancellation().is_cancelled() && self.guard.is_current() {
            let checkpoint = context.wake_checkpoint();
            let now = Instant::now();
            if now >= next && last != Some(self.view.revision.load(std::sync::atomic::Ordering::Acquire)) {
                let snapshot = self.view.snapshot(self.project);
                if context.cancellation().is_cancelled() || !self.guard.is_current() {
                    return;
                }
                last = Some(snapshot.revision);
                sink(snapshot);
                next = Instant::now() + Duration::from_millis(100);
            }
            let deadline = if Instant::now() < next {
                next
            } else {
                Instant::now() + Duration::from_secs(60)
            };
            context.wait_until(checkpoint, deadline);
        }
    }
}
impl Drop for ApplicationSubscription {
    fn drop(&mut self) {
        let mut observer = self.view.observer.lock().expect("application observer lock poisoned");
        if observer.as_ref().is_some_and(|(owner, _)| *owner == self.owner) {
            observer.take();
        }
    }
}
