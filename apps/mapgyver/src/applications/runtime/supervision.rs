//! Host service health, independent of whether any process worker was admitted.
use super::*;

#[derive(Default)]
pub(super) struct State {
    owner: Option<uuid::Uuid>,
    callback_revision: u64,
    pub snapshot: ApplicationSupervisionSnapshot,
}

pub(crate) struct Source(capture::Context);
pub(crate) struct Supervisor(Publisher);

/// Non-owning write token: cancellation/replacement can revoke a blocked callback's publication.
#[derive(Clone)]
pub(crate) struct Publisher {
    context: capture::Context,
    owner: uuid::Uuid,
}
impl Source {
    pub fn capture(engine: &AppEngine) -> Result<Self, String> {
        capture::Context::capture(engine).map(Self)
    }
    pub fn claim(self, host: ApplicationHost) -> Result<Supervisor, String> {
        let context = self.0;
        let mut state = context
            .view
            .supervision
            .lock()
            .expect("application supervision lock poisoned");
        if !context.guard.is_current() {
            return Err("application supervision project is retired".into());
        }
        if state.owner.is_some() {
            return Err("application supervision is already owned".into());
        }
        let owner = uuid::Uuid::new_v4();
        state.owner = Some(owner);
        state.snapshot = ApplicationSupervisionSnapshot {
            host: Some(host),
            demand: ApplicationServiceStatus::Starting,
            callbacks: ApplicationServiceStatus::Starting,
        };
        context.view.origin.get_or_init(Instant::now);
        context.view.changed();
        drop(state);
        Ok(Supervisor(Publisher { context, owner }))
    }
}
impl Supervisor {
    pub fn publisher(&self) -> Publisher {
        self.0.clone()
    }
    pub fn project(&self) -> uuid::Uuid {
        self.0.context.project
    }
    pub fn is_current(&self) -> bool {
        self.0.is_current()
    }
    pub fn validate_project(&self, project: uuid::Uuid) -> Result<(), String> {
        if self.project() == project && self.is_current() {
            Ok(())
        } else {
            Err("application supervision project changed before admission".into())
        }
    }
    #[cfg(all(windows, feature = "native-video"))]
    pub fn observations(&self) -> ApplicationObservations {
        ApplicationObservations::new(
            self.project(),
            self.0.context.guard.clone(),
            self.0.context.view.clone(),
        )
    }
    pub fn callbacks(&self) -> ApplicationCallbacks {
        ApplicationCallbacks::new(
            self.project(),
            self.0.context.guard.clone(),
            self.0.context.view.clone(),
        )
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        let mut state = self
            .0
            .context
            .view
            .supervision
            .lock()
            .expect("application supervision lock poisoned");
        if state.owner == Some(self.0.owner) {
            state.owner = None;
            state.snapshot = ApplicationSupervisionSnapshot::default();
            self.0.context.view.changed();
        }
    }
}
impl Publisher {
    fn is_current(&self) -> bool {
        self.context.guard.is_current()
            && self
                .context
                .view
                .supervision
                .lock()
                .expect("application supervision lock poisoned")
                .owner
                == Some(self.owner)
    }
    pub fn demand(&self, result: Result<(), String>) {
        self.publish(false, result);
    }
    pub fn callbacks(&self, result: Result<(), String>) {
        self.publish(true, result);
    }
    pub fn callback_attempt(&self) -> Option<u64> {
        let state = self
            .context
            .view
            .supervision
            .lock()
            .expect("application supervision lock poisoned");
        (self.context.guard.is_current() && state.owner == Some(self.owner)).then_some(state.callback_revision)
    }
    pub fn callbacks_started(&self, attempt: Option<u64>) {
        // The first delivery may already have failed before the spawning host regains control.
        if let Some(attempt) = attempt {
            self.publish_status(true, ApplicationServiceStatus::Running, Some(attempt));
        }
    }
    fn publish(&self, callbacks: bool, result: Result<(), String>) {
        let next = match result {
            Ok(()) => ApplicationServiceStatus::Running,
            Err(message) => ApplicationServiceStatus::Retrying {
                message: message.chars().take(512).collect(),
            },
        };
        self.publish_status(callbacks, next, None);
    }
    fn publish_status(&self, callbacks: bool, next: ApplicationServiceStatus, expected: Option<u64>) {
        let mut state = self
            .context
            .view
            .supervision
            .lock()
            .expect("application supervision lock poisoned");
        if !self.context.guard.is_current() || state.owner != Some(self.owner) {
            return;
        }
        if callbacks {
            if expected.is_some_and(|revision| revision != state.callback_revision) {
                return;
            }
            state.callback_revision = state
                .callback_revision
                .checked_add(1)
                .expect("callback service revision exhausted");
        }
        let status = if callbacks {
            &mut state.snapshot.callbacks
        } else {
            &mut state.snapshot.demand
        };
        if *status != next {
            *status = next;
            self.context.view.changed();
        }
    }
}
