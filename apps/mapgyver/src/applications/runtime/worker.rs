use super::{
    agent::{Agent, Desired, Native},
    *,
};
use golden_io::{WorkerContext, WorkerWake};
use std::time::Duration;

pub(super) fn current_target_is_admitted(shared: &Shared, id: ApplicationId, captured: &Target) -> bool {
    captured.guard.as_ref().is_some_and(RevisionGuard::is_current)
        && shared
            .desired
            .lock()
            .expect("application demand lock poisoned")
            .targets
            .get(&id)
            .is_some_and(|current| {
                current.epoch == captured.epoch
                    && current.guard.as_ref().is_some_and(RevisionGuard::is_current)
                    && current.admission.iter().all(RevisionGuard::is_current)
            })
}

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
    let mut readiness = super::readiness::Observer::default();
    let lifecycle = std::cell::RefCell::new(None);
    let publish = |status: &ApplicationRuntimeStatus| {
        let mut current = output.lock().expect("application status lock poisoned");
        if *current != *status {
            *current = status.clone();
            observations.publish(
                id,
                owner,
                status,
                lifecycle.borrow_mut().as_mut(),
                shared.project,
                u64::try_from(shared.origin.elapsed().as_millis()).unwrap_or(u64::MAX),
            );
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
        if cancelled && !agent.has_process() {
            return;
        }
        if !agent.has_process() && (!agent.idle() || demanded) && lifecycle.borrow().is_none() {
            *lifecycle.borrow_mut() = observations.callbacks.reserve();
            if lifecycle.borrow().is_none() {
                agent.status.callback_backpressure = true;
                publish(&agent.status);
                context.wait_until(checkpoint, Instant::now() + Duration::from_secs(1));
                continue;
            }
        }
        agent.status.callback_backpressure = false;
        agent.status.definition_revision = target.as_ref().and_then(|target| target.definition_revision);
        agent.status.definition_instance = target.as_ref().and_then(|target| target.definition_instance);
        let definition = (!cancelled)
            .then(|| target.as_ref().and_then(|target| target.definition.clone()))
            .flatten();
        if !cancelled
            && agent.status.phase == ApplicationPhase::WaitingForSender
            && target
                .as_ref()
                .is_some_and(|target| current_target_is_admitted(&shared, id, target))
            && let (Some(launch), Some(rule)) = (agent.status.launch, agent.readiness_rule())
            && let Some((sender, first_frame)) = readiness.sample(launch, rule)
            && shared.guard.is_current()
            && !context.cancellation().is_cancelled()
            && target
                .as_ref()
                .is_some_and(|target| current_target_is_admitted(&shared, id, target))
        {
            shared
                .readiness
                .lock()
                .expect("application readiness lock poisoned")
                .insert(
                    id,
                    ReadinessEvidence {
                        project: shared.project,
                        application: id,
                        launch,
                        sender,
                        first_frame,
                    },
                );
        }
        let ready = !cancelled
            && shared.guard.is_current()
            && target
                .as_ref()
                .is_some_and(|target| current_target_is_admitted(&shared, id, target))
            && shared
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
                && target
                    .as_ref()
                    .is_some_and(|target| current_target_is_admitted(&shared, id, target))
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
        if !agent.has_process() {
            lifecycle.borrow_mut().take();
        }
        if agent.status.phase == ApplicationPhase::Ready {
            // Tagged external evidence can arrive before our own receive has completed.
            // Keep servicing process ownership while the probe retires without blocking here.
            readiness.release_completed();
        } else if matches!(
            agent.status.phase,
            ApplicationPhase::Idle | ApplicationPhase::Failed | ApplicationPhase::Backoff
        ) {
            // These launches have already stopped their process. Cancellation must not delay
            // process termination behind potentially blocked GPU cleanup.
            readiness = super::readiness::Observer::default();
        }
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
