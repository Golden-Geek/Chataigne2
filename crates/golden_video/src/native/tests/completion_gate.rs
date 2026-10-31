//! Owned observation delay after real D3D11 query results; never a synthetic GPU completion.
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::runtime::{CompletionToken, DeviceGeneration};

#[derive(Clone, Copy, Debug)]
pub(in crate::native) struct Removal {
    pub device: DeviceGeneration,
    pub reason: i32,
    pub observed_at: Instant,
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::native) struct Snapshot {
    pub held: bool,
    pub observed: Option<CompletionToken>,
    pub delivered: Option<CompletionToken>,
    pub withheld_polls: u64,
    pub removal: Option<Removal>,
}

#[derive(Clone, Default)]
pub(in crate::native) struct CompletionGate(Arc<Mutex<Snapshot>>);

impl CompletionGate {
    pub fn hold(&self) {
        self.0.lock().unwrap().held = true;
    }
    pub fn release(&self) {
        self.0.lock().unwrap().held = false;
    }
    pub fn snapshot(&self) -> Snapshot {
        *self.0.lock().unwrap()
    }
    /// Observation only: called with the real removed reason before reset destroys the device.
    pub fn removed(&self, device: DeviceGeneration, reason: i32) {
        self.0.lock().unwrap().removal.get_or_insert(Removal {
            device,
            reason,
            observed_at: Instant::now(),
        });
    }
    pub fn observe(&self, observed: CompletionToken) -> CompletionToken {
        let mut state = self.0.lock().unwrap();
        state.observed = Some(observed);
        if state.held {
            state.withheld_polls = state.withheld_polls.saturating_add(1);
            state
                .delivered
                .filter(|token| token.device == observed.device)
                .unwrap_or(CompletionToken {
                    device: observed.device,
                    serial: 0,
                })
        } else {
            state.delivered = Some(observed);
            observed
        }
    }
}
