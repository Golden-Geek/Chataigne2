use crate::applications::protocol::*;
use golden_io::owned_process::{OwnedProcess, ProcessSnapshot, ProcessSpec, RootProcessState, SuspendedProcess};
use std::sync::Arc;

pub(super) struct ResumeFailure<P> {
    pub message: String,
    pub process: Box<P>,
}
pub(super) trait Backend {
    type Suspended;
    type Process;
    fn create(&mut self, spec: &ProcessSpec) -> Result<Self::Suspended, String>;
    fn resume(&mut self, process: Self::Suspended) -> Result<Self::Process, ResumeFailure<Self::Process>>;
    fn abandon(&mut self, process: Self::Suspended) -> Self::Process;
    fn id(&self, process: &Self::Process) -> u32;
    fn snapshot(&mut self, process: &Self::Process) -> Result<ProcessSnapshot, String>;
    fn close(&mut self, process: &Self::Process) -> Result<(), String>;
    fn terminate(&mut self, process: &Self::Process) -> Result<(), String>;
}
pub(super) struct Native;
pub(super) enum NativeProcess {
    Suspended(SuspendedProcess),
    Running(OwnedProcess),
}
impl Backend for Native {
    type Suspended = SuspendedProcess;
    type Process = NativeProcess;
    fn create(&mut self, spec: &ProcessSpec) -> Result<Self::Suspended, String> {
        SuspendedProcess::create(spec).map_err(|e| e.to_string())
    }
    fn resume(&mut self, process: Self::Suspended) -> Result<Self::Process, ResumeFailure<Self::Process>> {
        process.resume().map(NativeProcess::Running).map_err(|error| {
            let (error, process) = error.into_parts();
            ResumeFailure {
                message: error.to_string(),
                process: Box::new(NativeProcess::Suspended(process)),
            }
        })
    }
    fn abandon(&mut self, process: Self::Suspended) -> Self::Process {
        NativeProcess::Suspended(process)
    }
    fn id(&self, process: &Self::Process) -> u32 {
        match process {
            NativeProcess::Suspended(process) => process.id(),
            NativeProcess::Running(process) => process.id(),
        }
    }
    fn snapshot(&mut self, process: &Self::Process) -> Result<ProcessSnapshot, String> {
        match process {
            NativeProcess::Suspended(process) => process.snapshot(),
            NativeProcess::Running(process) => process.snapshot(),
        }
        .map_err(|e| e.to_string())
    }
    fn close(&mut self, process: &Self::Process) -> Result<(), String> {
        match process {
            NativeProcess::Suspended(_) => Ok(()),
            NativeProcess::Running(process) => process.request_close().map(|_| ()).map_err(|e| e.to_string()),
        }
    }
    fn terminate(&mut self, process: &Self::Process) -> Result<(), String> {
        match process {
            NativeProcess::Suspended(process) => process.terminate(1),
            NativeProcess::Running(process) => process.terminate(1),
        }
        .map_err(|e| e.to_string())
    }
}

struct Stop {
    force: u64,
    deadline: u64,
    forced: bool,
    failure: bool,
}

pub(super) struct Desired {
    pub demanded: bool,
    pub definition: Option<Arc<ApplicationDefinition>>,
    pub retain_existing: bool,
}

pub(super) struct Agent<B: Backend> {
    pub status: ApplicationRuntimeStatus,
    pub backend: B,
    process: Option<B::Process>,
    definition: Option<Arc<ApplicationDefinition>>,
    stop: Option<Stop>,
    ready_at: Option<u64>,
    failed: bool,
}
impl<B: Backend> Agent<B> {
    pub fn new(application: ApplicationId, backend: B) -> Self {
        Self {
            status: ApplicationRuntimeStatus {
                application,
                launch: None,
                phase: ApplicationPhase::Idle,
                process_id: None,
                exit_code: None,
                failures: 0,
                next_action_ms: None,
                message: None,
            },
            backend,
            process: None,
            definition: None,
            stop: None,
            ready_at: None,
            failed: false,
        }
    }
    pub fn idle(&self) -> bool {
        self.process.is_none() && self.status.phase == ApplicationPhase::Idle
    }
    pub fn readiness_rule(&self) -> Option<&ApplicationReadiness> {
        self.definition.as_ref().map(|definition| &definition.readiness)
    }
    fn message(&mut self, message: impl AsRef<str>) {
        self.status.message = Some(message.as_ref().chars().take(512).collect());
    }
    fn failure(&mut self, now: u64, message: &str) {
        if self
            .ready_at
            .take()
            .is_some_and(|ready| now.saturating_sub(ready) >= 10_000)
        {
            self.status.failures = 0;
        }
        self.message(message);
        let definition = self.definition.as_ref().unwrap();
        let delay = u64::from(definition.restart.initial_ms)
            .saturating_mul(1u64 << self.status.failures.min(31))
            .min(u64::from(definition.restart.maximum_ms));
        self.status.failures = self.status.failures.saturating_add(1);
        self.failed = !definition.restart.enabled;
        self.status.phase = if self.failed {
            ApplicationPhase::Failed
        } else {
            ApplicationPhase::Backoff
        };
        self.status.next_action_ms = (!self.failed).then_some(now.saturating_add(delay));
    }
    fn begin_stop(&mut self, now: u64, failure: bool, message: &str) {
        if self.stop.is_some() {
            return;
        }
        self.message(message);
        let definition = self.definition.as_ref().unwrap();
        let force = now.saturating_add(u64::from(definition.stop.graceful_ms));
        let deadline = force.saturating_add(u64::from(definition.stop.retirement_ms));
        self.stop = Some(Stop {
            force,
            deadline,
            forced: false,
            failure,
        });
        self.status.phase = ApplicationPhase::Stopping;
        self.status.next_action_ms = Some(force);
        if let Some(process) = &self.process
            && let Err(error) = self.backend.close(process)
        {
            self.message(error);
        }
    }
    /// One background service turn. A target with no definition retains an existing owner only.
    /// The admission closure is checked again after suspended creation, before any target executes.
    pub fn tick(
        &mut self,
        now: u64,
        desired: Desired,
        ready: bool,
        admitted: impl Fn() -> bool,
        publish: impl Fn(&ApplicationRuntimeStatus),
    ) {
        let Desired {
            demanded,
            definition: target,
            retain_existing,
        } = desired;
        let changed = target
            .as_ref()
            .is_some_and(|target| self.definition.as_ref() != Some(target));
        if self.process.is_some() && (!demanded || (changed && !retain_existing)) {
            self.begin_stop(
                now,
                false,
                if demanded {
                    "configuration replaced"
                } else {
                    "demand released"
                },
            );
        }
        if let Some(process) = &self.process {
            match self.backend.snapshot(process) {
                Ok(snapshot) => {
                    if let RootProcessState::Exited(code) = snapshot.root {
                        self.status.exit_code = Some(code);
                        if self.stop.is_none() {
                            self.begin_stop(now, true, "owned root exited");
                        }
                    }
                    if snapshot.tree_exited() {
                        self.process.take();
                        self.status.process_id = None;
                        let failure = self.stop.take().is_some_and(|stop| stop.failure) && demanded && !changed;
                        if failure {
                            let message = self.status.message.clone().unwrap_or_default();
                            self.failure(now, &message);
                        } else {
                            self.status.phase = ApplicationPhase::Idle;
                            self.status.next_action_ms = None;
                            self.failed = false;
                        }
                        return;
                    }
                }
                Err(error) => self.begin_stop(now, true, &error),
            }
            if self.stop.is_none() && self.status.phase == ApplicationPhase::WaitingForSender {
                if ready {
                    self.status.phase = ApplicationPhase::Ready;
                    self.status.next_action_ms = None;
                    self.ready_at = Some(now);
                } else if self.status.next_action_ms.is_some_and(|deadline| now >= deadline) {
                    self.begin_stop(now, true, "sender readiness timed out");
                }
            }
            if let Some(stop) = &mut self.stop {
                if now >= stop.force && !stop.forced {
                    stop.forced = true;
                    self.status.next_action_ms = Some(stop.deadline);
                    if let Err(error) = self.backend.terminate(self.process.as_ref().unwrap()) {
                        self.status.message = Some(error.chars().take(512).collect());
                    }
                }
                if now >= stop.deadline {
                    self.status.phase = ApplicationPhase::RetirementUnverified;
                    self.status.next_action_ms = None;
                    // Keep ownership and the worker's global lifetime slot until proof arrives.
                }
            }
            return;
        }
        if !demanded {
            self.definition = None;
            self.failed = false;
            self.status.phase = ApplicationPhase::Idle;
            self.status.next_action_ms = None;
            self.status.failures = 0;
            return;
        }
        let Some(target) = target else {
            self.definition = None;
            self.failed = false;
            self.status.failures = 0;
            self.status.phase = ApplicationPhase::Unavailable;
            self.status.next_action_ms = None;
            return;
        };
        if changed {
            self.definition = Some(target.clone());
            self.failed = false;
            self.status.failures = 0;
            self.status.next_action_ms = None;
        }
        if self.failed || self.status.next_action_ms.is_some_and(|deadline| now < deadline) || !admitted() {
            return;
        }
        self.status.launch = Some(uuid::Uuid::new_v4());
        self.status.phase = ApplicationPhase::Starting;
        self.status.message = None;
        self.status.exit_code = None;
        publish(&self.status);
        let launched: Result<Option<B::Process>, String> = (|| {
            let spec = target.process_spec()?.ok_or("application has no executable")?;
            let suspended = self.backend.create(&spec)?;
            if !admitted() {
                self.process = Some(self.backend.abandon(suspended));
                self.begin_stop(now, false, "launch cancelled before resume");
                self.stop.as_mut().unwrap().force = now;
                return Ok(None);
            }
            match self.backend.resume(suspended) {
                Ok(process) => Ok(Some(process)),
                Err(error) => {
                    self.process = Some(*error.process);
                    self.begin_stop(now, true, &error.message);
                    self.stop.as_mut().unwrap().force = now;
                    Ok(None)
                }
            }
        })();
        match launched {
            Ok(None) => {}
            Ok(Some(process)) => {
                self.status.process_id = Some(self.backend.id(&process));
                self.process = Some(process);
                if !admitted() {
                    self.begin_stop(now, false, "launch cancelled after resume");
                    return;
                }
                match target.readiness {
                    ApplicationReadiness::ProcessStarted => {
                        self.status.phase = ApplicationPhase::Ready;
                        self.ready_at = Some(now);
                        self.status.next_action_ms = None;
                    }
                    ApplicationReadiness::Spout { timeout_ms, .. } => {
                        self.status.phase = ApplicationPhase::WaitingForSender;
                        self.status.next_action_ms = Some(now.saturating_add(u64::from(timeout_ms)));
                    }
                }
            }
            Err(error) => self.failure(now, &error),
        }
    }
}
