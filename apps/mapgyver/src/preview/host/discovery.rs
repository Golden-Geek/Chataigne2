//! One coalesced refresh flag and a bounded metadata cache; SDK work belongs to the worker.

use crate::media::protocol::SenderDiscoverySnapshot;
use golden_io::WorkerNotifier;
use std::sync::{Arc, Mutex};

pub(super) const MAX_NAMES: usize = 64;
// Leave room for the snapshot envelope within the host's 64 KiB control reply limit.
const MAX_NAMES_JSON_BYTES: usize = 48 * 1024;

#[derive(Default)]
struct State {
    pending: bool,
    snapshot: SenderDiscoverySnapshot,
}

#[derive(Clone)]
pub(super) struct Discovery {
    state: Arc<Mutex<State>>,
    notifier: WorkerNotifier,
}

impl Discovery {
    pub fn new(notifier: WorkerNotifier) -> Self {
        Self {
            state: Arc::default(),
            notifier,
        }
    }

    pub fn request(&self, refresh: bool) -> Result<SenderDiscoverySnapshot, String> {
        let mut state = self.state.lock().map_err(|_| "sender catalog lock poisoned")?;
        let wake = refresh && !state.snapshot.refreshing;
        if wake {
            state
                .snapshot
                .revision
                .checked_add(2)
                .ok_or("sender catalog revision exhausted")?;
            state.snapshot.revision += 1;
            state.snapshot.refreshing = true;
            state.snapshot.error = None;
            state.pending = true;
        }
        let snapshot = state.snapshot.clone();
        drop(state);
        if wake {
            self.notifier.notify();
        }
        Ok(snapshot)
    }

    pub fn take(&self) -> Option<u32> {
        let mut state = self.state.lock().ok()?;
        if !std::mem::take(&mut state.pending) {
            return None;
        }
        Some(state.snapshot.revision)
    }

    pub fn complete(
        &self,
        revision: u32,
        result: Result<(Vec<String>, bool), String>,
    ) -> Option<SenderDiscoverySnapshot> {
        let mut state = self.state.lock().ok()?;
        if !state.snapshot.refreshing || state.snapshot.revision != revision {
            return None;
        }
        state.snapshot.revision += 1;
        state.snapshot.refreshing = false;
        match result {
            Ok((names, mut truncated)) => {
                let mut admitted = Vec::with_capacity(names.len().min(MAX_NAMES));
                let mut bytes = 0;
                for name in names {
                    let encoded = serde_json::to_string(&name).expect("sender string").len() + 1;
                    if admitted.len() == MAX_NAMES || bytes + encoded > MAX_NAMES_JSON_BYTES {
                        truncated = true;
                        break;
                    }
                    bytes += encoded;
                    admitted.push(name);
                }
                state.snapshot.names = admitted;
                state.snapshot.truncated = truncated;
                state.snapshot.error = None;
            }
            Err(error) => state.snapshot.error = Some(error),
        }
        Some(state.snapshot.clone())
    }
}

#[cfg(test)]
mod tests;
