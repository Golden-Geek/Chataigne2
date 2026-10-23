use super::*;
use golden_core::engine::{EngineTime, NodeWaker};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex, OnceLock, Weak,
};

const MAX_SOURCES: usize = 512;

pub(super) enum Origin {
    Engine(NodeWaker),
    #[cfg(test)]
    Fixture,
}
struct Claim {
    time: EngineTime,
    sequence: u64,
    origin: Origin,
    guards: [RevisionGuard; 2],
    done: AtomicBool,
}
impl Claim {
    fn live(&self) -> bool {
        !self.done.load(Ordering::Acquire)
            && self.guards.iter().all(RevisionGuard::is_current)
            && match &self.origin {
                Origin::Engine(node) => !node.is_closed(),
                #[cfg(test)]
                Origin::Fixture => true,
            }
    }
}
#[derive(Default)]
struct State {
    sequence: u64,
    pending: Vec<Weak<Claim>>,
    active: Option<u64>,
}
pub(super) struct Coordinator {
    state: Mutex<State>,
    changed: Condvar,
}
impl Coordinator {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
        })
    }
    pub(super) fn process() -> Arc<Self> {
        static SHARED: OnceLock<Arc<Coordinator>> = OnceLock::new();
        SHARED.get_or_init(Self::new).clone()
    }
    pub(super) fn capture(
        self: &Arc<Self>,
        time: EngineTime,
        origin: Origin,
        guards: [RevisionGuard; 2],
    ) -> Result<Ticket, String> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        prune(&mut state);
        if state.pending.len() + usize::from(state.active.is_some()) >= MAX_SOURCES {
            return Err("Keyboard process output's 512 ordering slots are occupied".into());
        }
        let sequence = state
            .sequence
            .checked_add(1)
            .ok_or("Keyboard output ordering sequence exhausted")?;
        state.sequence = sequence;
        let claim = Arc::new(Claim {
            time,
            sequence,
            origin,
            guards,
            done: AtomicBool::new(false),
        });
        let at = state.pending.partition_point(|pending| {
            pending
                .upgrade()
                .is_some_and(|pending| (pending.time, pending.sequence) <= (time, sequence))
        });
        state.pending.insert(at, Arc::downgrade(&claim));
        self.changed.notify_all();
        Ok(Ticket {
            claim,
            coordinator: self.clone(),
        })
    }
}
fn prune(state: &mut State) {
    state
        .pending
        .retain(|pending| pending.upgrade().is_some_and(|claim| claim.live()));
}
pub(super) struct Ticket {
    claim: Arc<Claim>,
    coordinator: Arc<Coordinator>,
}
impl Ticket {
    pub(super) fn live(&self) -> bool {
        self.claim.live()
    }
    pub(super) fn finish(&self) {
        if !self.claim.done.swap(true, Ordering::AcqRel) {
            let mut state = self.coordinator.state.lock().unwrap_or_else(|error| error.into_inner());
            prune(&mut state);
            self.coordinator.changed.notify_all();
        }
    }
    pub(super) fn acquire(&self, cancelled: impl Fn() -> bool) -> Option<Permit> {
        let mut state = self.coordinator.state.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            prune(&mut state);
            if cancelled() || !self.live() {
                drop(state);
                self.finish();
                return None;
            }
            if state.active.is_none()
                && state
                    .pending
                    .first()
                    .and_then(Weak::upgrade)
                    .is_some_and(|claim| claim.sequence == self.claim.sequence)
            {
                state.pending.remove(0);
                state.active = Some(self.claim.sequence);
                return Some(Permit {
                    coordinator: self.coordinator.clone(),
                    sequence: self.claim.sequence,
                });
            }
            // Waiting and liveness checks stay on the IO worker. Retirement may
            // invalidate a captured head that has not reached its native mailbox.
            state = self
                .coordinator
                .changed
                .wait_timeout(state, Duration::from_millis(10))
                .unwrap_or_else(|error| error.into_inner())
                .0;
        }
    }
}
impl Drop for Ticket {
    fn drop(&mut self) {
        self.finish();
    }
}
pub(super) struct Permit {
    coordinator: Arc<Coordinator>,
    sequence: u64,
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut state = self.coordinator.state.lock().unwrap_or_else(|error| error.into_inner());
        debug_assert_eq!(state.active, Some(self.sequence));
        state.active = None;
        self.coordinator.changed.notify_all();
    }
}

#[cfg(test)]
mod tests;
