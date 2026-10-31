use super::{agent::*, *};
use golden_io::owned_process::{ProcessSnapshot, ProcessSpec, RootProcessState};
use std::sync::atomic::{AtomicBool, Ordering};

struct Fake {
    creates: usize,
    resumes: usize,
    closes: usize,
    kills: usize,
    running: bool,
    retired: bool,
    retire_on_kill: bool,
    fail_resume: bool,
    cancel_create: Option<Arc<AtomicBool>>,
}
impl Default for Fake {
    fn default() -> Self {
        Self {
            creates: 0,
            resumes: 0,
            closes: 0,
            kills: 0,
            running: false,
            retired: false,
            retire_on_kill: true,
            fail_resume: false,
            cancel_create: None,
        }
    }
}
impl Backend for Fake {
    type Suspended = ();
    type Process = ();
    fn create(&mut self, _: &ProcessSpec) -> Result<(), String> {
        self.creates += 1;
        self.running = true;
        self.retired = false;
        if let Some(cancel) = &self.cancel_create {
            cancel.store(false, Ordering::Release);
        }
        Ok(())
    }
    fn resume(&mut self, _: ()) -> Result<(), ResumeFailure<()>> {
        self.resumes += 1;
        if self.fail_resume {
            Err(ResumeFailure {
                message: "resume denied".into(),
                process: Box::new(()),
            })
        } else {
            Ok(())
        }
    }
    fn abandon(&mut self, _: ()) {}
    fn id(&self, _: &()) -> u32 {
        42
    }
    fn snapshot(&mut self, _: &()) -> Result<ProcessSnapshot, String> {
        Ok(ProcessSnapshot {
            root_id: 42,
            root: if self.running {
                RootProcessState::Running
            } else {
                RootProcessState::Exited(259)
            },
            active_processes: u32::from(!self.retired),
            unobserved_processes: 0,
            pending_observed_processes: 0,
        })
    }
    fn close(&mut self, _: &()) -> Result<(), String> {
        self.closes += 1;
        Ok(())
    }
    fn terminate(&mut self, _: &()) -> Result<(), String> {
        self.kills += 1;
        self.running = false;
        self.retired = self.retire_on_kill;
        Ok(())
    }
}
fn definition() -> Arc<ApplicationDefinition> {
    Arc::new(ApplicationDefinition {
        executable: Some(std::env::current_exe().unwrap().to_string_lossy().into()),
        stop: ApplicationStop {
            graceful_ms: 10,
            retirement_ms: 20,
        },
        restart: ApplicationRestart {
            enabled: true,
            initial_ms: 1,
            maximum_ms: 4,
        },
        ..Default::default()
    })
}
fn agent() -> Agent<Fake> {
    Agent::new(
        ApplicationId(golden_core::model::NodeUuid(uuid::Uuid::new_v4())),
        Fake::default(),
    )
}
fn tick(
    agent: &mut Agent<Fake>,
    now: u64,
    demanded: bool,
    definition: Option<Arc<ApplicationDefinition>>,
    ready: bool,
) {
    agent.tick(
        now,
        Desired {
            demanded,
            definition,
            retain_existing: false,
        },
        ready,
        || true,
        |_| {},
    );
}

#[test]
fn shared_reconciliation_and_retained_missing_definition_never_restart_until_final_release() {
    let mut agent = agent();
    let definition = definition();
    tick(&mut agent, 0, true, Some(definition.clone()), false);
    let launch = agent.status.launch;
    for now in 1..10 {
        tick(&mut agent, now, true, Some(definition.clone()), false);
    }
    tick(&mut agent, 10, true, None, false);
    assert_eq!(agent.backend.creates, 1);
    assert_eq!(agent.status.launch, launch);
    assert_eq!(agent.status.phase, ApplicationPhase::Ready);
    tick(&mut agent, 11, false, None, false);
    assert_eq!(agent.backend.closes, 1);
    tick(&mut agent, 20, false, None, false);
    assert_eq!(agent.backend.kills, 0);
    tick(&mut agent, 21, false, None, false);
    assert_eq!(agent.backend.kills, 1);
    tick(&mut agent, 22, false, None, false);
    tick(&mut agent, 1000, false, None, false);
    assert!(agent.idle());
    assert_eq!(agent.backend.creates, 1);
}

#[test]
fn sender_timeout_stops_owned_tree_then_backs_off_and_requires_fresh_launch_evidence() {
    let mut agent = agent();
    let definition = Arc::new(ApplicationDefinition {
        readiness: ApplicationReadiness::Spout {
            sender: "renderer".into(),
            first_frame: true,
            timeout_ms: 20,
        },
        ..(*definition()).clone()
    });
    tick(&mut agent, 0, true, Some(definition.clone()), false);
    let first = agent.status.launch;
    assert_eq!(agent.status.phase, ApplicationPhase::WaitingForSender);
    tick(&mut agent, 19, true, Some(definition.clone()), false);
    assert_eq!(agent.backend.closes, 0);
    tick(&mut agent, 20, true, Some(definition.clone()), false);
    assert_eq!(agent.status.phase, ApplicationPhase::Stopping);
    tick(&mut agent, 30, true, Some(definition.clone()), false);
    tick(&mut agent, 31, true, Some(definition.clone()), false);
    assert_eq!(agent.status.next_action_ms, Some(32));
    tick(&mut agent, 32, true, Some(definition.clone()), false);
    assert_ne!(agent.status.launch, first);
    assert_eq!(agent.status.phase, ApplicationPhase::WaitingForSender);
    tick(&mut agent, 33, true, Some(definition), true);
    assert_eq!(agent.status.phase, ApplicationPhase::Ready);
}

#[test]
fn crash_loops_back_off_exponentially_and_intentional_unload_cancels_retry() {
    let mut agent = agent();
    let definition = definition();
    tick(&mut agent, 0, true, Some(definition.clone()), false);
    agent.backend.running = false;
    agent.backend.retired = true;
    tick(&mut agent, 1, true, Some(definition.clone()), false);
    assert_eq!(agent.status.exit_code, Some(259));
    assert_eq!(agent.status.next_action_ms, Some(2));
    tick(&mut agent, 2, true, Some(definition.clone()), false);
    agent.backend.running = false;
    agent.backend.retired = true;
    tick(&mut agent, 3, true, Some(definition.clone()), false);
    assert_eq!(agent.status.next_action_ms, Some(5));
    tick(&mut agent, 4, false, None, false);
    tick(&mut agent, 100, false, None, false);
    assert_eq!(agent.backend.creates, 2);
    assert!(agent.idle());
}

#[test]
fn cancelled_suspended_creation_is_never_resumed_and_retains_ownership_until_retirement() {
    let mut agent = agent();
    let admitted = Arc::new(AtomicBool::new(true));
    agent.backend.cancel_create = Some(admitted.clone());
    agent.tick(
        0,
        Desired {
            demanded: true,
            definition: Some(definition()),
            retain_existing: false,
        },
        false,
        || admitted.load(Ordering::Acquire),
        |_| {},
    );
    assert_eq!(agent.backend.resumes, 0);
    assert_eq!(agent.status.phase, ApplicationPhase::Stopping);
    tick(&mut agent, 1, false, None, false);
    tick(&mut agent, 2, false, None, false);
    assert!(agent.idle());
    assert_eq!(agent.backend.creates, 1);
}

#[test]
fn stuck_retirement_keeps_owner_and_prevents_replacement_even_after_deadline() {
    let mut agent = agent();
    agent.backend.retire_on_kill = false;
    let first = definition();
    tick(&mut agent, 0, true, Some(first.clone()), false);
    let second = Arc::new(ApplicationDefinition {
        arguments: vec!["replacement".into()],
        ..(*first).clone()
    });
    tick(&mut agent, 1, true, Some(second.clone()), false);
    tick(&mut agent, 11, true, Some(second.clone()), false);
    tick(&mut agent, 31, true, Some(second.clone()), false);
    tick(&mut agent, 100, true, Some(second.clone()), false);
    assert_eq!(agent.status.phase, ApplicationPhase::RetirementUnverified);
    assert_eq!(agent.backend.creates, 1);
    assert_eq!(agent.backend.kills, 1);
    agent.backend.retired = true;
    tick(&mut agent, 101, true, Some(second.clone()), false);
    tick(&mut agent, 102, true, Some(second), false);
    assert_eq!(agent.backend.creates, 2);
}

#[test]
fn resume_failure_still_owns_suspended_tree_and_no_retry_precedes_retirement_proof() {
    let mut agent = agent();
    agent.backend.fail_resume = true;
    agent.backend.retire_on_kill = false;
    let definition = definition();
    tick(&mut agent, 0, true, Some(definition.clone()), false);
    assert_eq!(agent.status.phase, ApplicationPhase::Stopping);
    tick(&mut agent, 1, true, Some(definition.clone()), false);
    tick(&mut agent, 100, true, Some(definition.clone()), false);
    assert_eq!(agent.status.phase, ApplicationPhase::RetirementUnverified);
    assert_eq!(agent.backend.creates, 1);
    agent.backend.retired = true;
    tick(&mut agent, 101, true, Some(definition), false);
    assert_eq!(agent.status.phase, ApplicationPhase::Backoff);
}

#[test]
fn configuration_replacement_waits_for_outgoing_or_final_use_hold_release() {
    let mut agent = agent();
    let first = definition();
    tick(&mut agent, 0, true, Some(first.clone()), false);
    let launch = agent.status.launch;
    let next = Arc::new(ApplicationDefinition {
        arguments: vec!["replacement".into()],
        ..(*first).clone()
    });
    for now in [1, 100] {
        agent.tick(
            now,
            Desired {
                demanded: true,
                definition: Some(next.clone()),
                retain_existing: true,
            },
            false,
            || true,
            |_| {},
        );
    }
    assert_eq!(agent.status.launch, launch);
    assert_eq!(agent.backend.closes, 0);
    tick(&mut agent, 101, true, Some(next.clone()), false);
    tick(&mut agent, 111, true, Some(next.clone()), false);
    tick(&mut agent, 112, true, Some(next.clone()), false);
    tick(&mut agent, 113, true, Some(next), false);
    assert_eq!(agent.backend.creates, 2);
    assert_ne!(agent.status.launch, launch);
}

#[test]
fn explicit_disable_and_restore_rearms_a_failed_non_restarting_definition() {
    let mut agent = agent();
    let definition = Arc::new(ApplicationDefinition {
        restart: ApplicationRestart {
            enabled: false,
            ..Default::default()
        },
        ..(*definition()).clone()
    });
    tick(&mut agent, 0, true, Some(definition.clone()), false);
    agent.backend.running = false;
    agent.backend.retired = true;
    tick(&mut agent, 1, true, Some(definition.clone()), false);
    assert_eq!(agent.status.phase, ApplicationPhase::Failed);
    tick(&mut agent, 100, true, Some(definition.clone()), false);
    assert_eq!(agent.backend.creates, 1);
    tick(&mut agent, 101, true, None, false);
    assert_eq!(agent.status.phase, ApplicationPhase::Unavailable);
    tick(&mut agent, 102, true, Some(definition), false);
    assert_eq!(agent.backend.creates, 2);
}
