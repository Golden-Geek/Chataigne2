//! Tracks native registrations owned by this process's renderer outputs.

use std::collections::{BTreeMap, btree_map::Entry};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct SenderIdentity {
    pub name: String,
    pub shared_handle: u32,
}

struct Registry {
    senders: BTreeMap<SenderIdentity, usize>,
    changing: usize,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    senders: BTreeMap::new(),
    changing: 0,
});
static REVISION: AtomicU64 = AtomicU64::new(0);

fn registry() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(|error| error.into_inner())
}

/// Read before querying the SDK. The matched registration must span the native observation.
pub(super) fn revision() -> u64 {
    REVISION.load(Ordering::Acquire)
}

fn stable_registry(observed_at: u64) -> Option<MutexGuard<'static, Registry>> {
    let registry = registry();
    (registry.changing == 0 && revision() == observed_at).then_some(registry)
}

pub(super) fn internal_names(observed_at: u64, observed: &[SenderIdentity]) -> Option<Vec<String>> {
    let registry = stable_registry(observed_at)?;
    Some(
        observed
            .iter()
            .filter(|sender| registry.senders.contains_key(sender))
            .map(|sender| sender.name.clone())
            .collect(),
    )
}

pub(super) fn is_external(observed_at: u64, name: &str, shared_handle: u32) -> bool {
    stable_registry(observed_at).is_some_and(|registry| {
        !registry
            .senders
            .keys()
            .any(|sender| sender.shared_handle == shared_handle && sender.name == name)
    })
}

/// First send can expose SDK metadata before it returns. Keep overlapping observations
/// provisional without holding the provenance lock across a native SDK call.
pub(super) struct Change;

impl Change {
    fn begin() -> Self {
        let mut registry = registry();
        registry.changing += 1;
        REVISION.fetch_add(1, Ordering::Release);
        Self
    }
}

impl Drop for Change {
    fn drop(&mut self) {
        let mut registry = registry();
        registry.changing -= 1;
        REVISION.fetch_add(1, Ordering::Release);
    }
}

/// The worker retaining the SDK output through its last-use fence also owns its provenance.
#[derive(Default)]
pub(super) struct Publication {
    sender: Option<SenderIdentity>,
}

impl Publication {
    pub fn before_first_send(&self) -> Option<Change> {
        self.sender.is_none().then(Change::begin)
    }

    pub fn complete_first_send(&mut self, _change: Change, sender: Option<SenderIdentity>) {
        if let Some(sender) = sender {
            *registry().senders.entry(sender.clone()).or_default() += 1;
            self.sender = Some(sender);
        }
    }

    pub fn retire(&mut self, destroy: impl FnOnce()) {
        let _change = Change::begin();
        destroy();
        self.unregister();
    }

    fn unregister(&mut self) {
        if let Some(sender) = self.sender.take() {
            let mut registry = registry();
            if let Entry::Occupied(mut entry) = registry.senders.entry(sender) {
                if *entry.get() == 1 {
                    entry.remove();
                } else {
                    *entry.get_mut() -= 1;
                }
            }
        }
    }
}

impl Drop for Publication {
    fn drop(&mut self) {
        if self.sender.is_some() {
            let _change = Change::begin();
            self.unregister();
        }
    }
}

#[cfg(test)]
mod tests;
