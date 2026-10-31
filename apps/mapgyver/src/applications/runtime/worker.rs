use super::{
    agent::{Agent, Desired, Native},
    *,
};
use golden_io::{WorkerContext, WorkerWake};
use std::time::Duration;

pub(super) fn run(
    id: ApplicationId,
    owner: uuid::Uuid,
    shared: Arc<Shared>,
    output: Arc<Mutex<ApplicationRuntimeStatus>>,
    observations: RuntimeView,
    notifier: Option<WorkerNotifier>,
    context: WorkerContext<(), ()>,
) {
    let mut agent = Agent::new(id, Native);
    let publish = |status: &ApplicationRuntimeStatus| {
        let mut current = output.lock().expect("application status lock poisoned");
        if *current != *status {
            *current = status.clone();
            if let Some((registered, observed)) = observations
                .statuses
                .lock()
                .expect("application observation lock poisoned")
                .get_mut(&id)
                && *registered == owner
            {
                *observed = status.clone();
            }
            drop(current);
            if let Some(notifier) = &notifier {
                notifier.notify();
            }
        }
    };
    loop {
        let checkpoint = context.wake_checkpoint();
        let cancelled = context.cancellation().is_cancelled() || !shared.guard.is_current();
        let target = shared
            .desired
            .lock()
            .expect("application demand lock poisoned")
            .targets
            .get(&id)
            .cloned();
        let final_use = shared
            .final_use
            .lock()
            .expect("application final-use lock poisoned")
            .get(&id)
            .is_some_and(|lifetimes| lifetimes.iter().any(|lifetime| !lifetime.is_complete()));
        let demanded = (!cancelled && target.is_some()) || final_use;
        let definition = (!cancelled)
            .then(|| target.as_ref().and_then(|target| target.definition.clone()))
            .flatten();
        let ready = shared
            .readiness
            .lock()
            .expect("application readiness lock poisoned")
            .get(&id)
            .is_some_and(|evidence| {
                evidence.project == shared.project
                    && Some(evidence.launch) == agent.status.launch
                    && agent.readiness_rule().is_some_and(|readiness| match readiness {
                        ApplicationReadiness::Spout {
                            sender, first_frame, ..
                        } => evidence.sender == *sender && (!first_frame || evidence.first_frame),
                        _ => false,
                    })
            });
        let admitted = || {
            !context.cancellation().is_cancelled()
                && shared.guard.is_current()
                && target.as_ref().is_some_and(|target| {
                    target.guard.as_ref().is_some_and(RevisionGuard::is_current)
                        && target.admission.iter().all(RevisionGuard::is_current)
                        && shared
                            .desired
                            .lock()
                            .expect("application demand lock poisoned")
                            .targets
                            .get(&id)
                            .is_some_and(|current| current.epoch == target.epoch)
                })
        };
        let now = u64::try_from(shared.origin.elapsed().as_millis()).unwrap_or(u64::MAX);
        agent.tick(
            now,
            Desired {
                demanded,
                definition,
                retain_existing: final_use || target.as_ref().is_some_and(|target| target.retain_existing),
            },
            ready,
            admitted,
            publish,
        );
        publish(&agent.status);
        if cancelled && agent.idle() {
            return;
        }
        let delay = match agent.status.phase {
            ApplicationPhase::RetirementUnverified => 250,
            ApplicationPhase::Idle
                if target.as_ref().is_some_and(|target| {
                    target.definition.is_some() && target.guard.as_ref().is_some_and(RevisionGuard::is_current)
                }) =>
            {
                1
            }
            ApplicationPhase::Idle | ApplicationPhase::Unavailable | ApplicationPhase::Failed => 1000,
            _ => 25,
        };
        if context.cancellation().is_cancelled() {
            // Cancellation cannot busy-spin its already-signalled worker wait during cleanup.
            std::thread::sleep(Duration::from_millis(delay));
        } else if context.wait_until(checkpoint, Instant::now() + Duration::from_millis(delay))
            == WorkerWake::Disconnected
        {
            continue;
        }
    }
}
