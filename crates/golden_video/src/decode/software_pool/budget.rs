//! Allocation charges outlive codec pools while any frame still owns their buffers.
use super::super::DecodeError;
use std::sync::{Arc, Mutex};

/// Actual decoded-plane payload inventory, including cached reusable buffers.
/// Excludes codec-private state, allocator overhead, packets, scaler and published RGBA.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct SoftwareFrameAllocation {
    /// Maximum simultaneous allocated plane payload.
    pub limit_bytes: u64,
    /// Currently allocated payload, including buffers idle in a codec pool.
    pub retained_bytes: u64,
    /// Lifetime maximum allocated payload.
    pub peak_bytes: u64,
    /// Number of allocated plane buffers, not frame references.
    pub buffers: u64,
}

/// Read-only lifetime observer; retaining it does not retain any pixel buffer or pool.
#[derive(Clone)]
pub struct SoftwareFrameBudget(Arc<Mutex<SoftwareFrameAllocation>>);

pub(super) struct Lease {
    budget: SoftwareFrameBudget,
    bytes: u64,
}
impl SoftwareFrameBudget {
    pub(super) fn new(bytes: u64) -> Self {
        Self(Arc::new(Mutex::new(SoftwareFrameAllocation {
            limit_bytes: bytes,
            retained_bytes: 0,
            peak_bytes: 0,
            buffers: 0,
        })))
    }
    /// One coherent inventory; a zero value after decoder/frame drop proves payload retirement.
    pub fn snapshot(&self) -> SoftwareFrameAllocation {
        *self.0.lock().unwrap_or_else(|error| error.into_inner())
    }
    pub(in crate::decode) fn tighten(&self, bytes: u64) -> Result<(), DecodeError> {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if bytes == 0 || bytes > state.limit_bytes {
            return Err(DecodeError::InvalidLimits);
        }
        if bytes < state.retained_bytes {
            return Err(DecodeError::LimitExceeded);
        }
        state.limit_bytes = bytes;
        Ok(())
    }
    pub(super) fn reserve(&self, bytes: usize) -> Option<Lease> {
        let bytes = u64::try_from(bytes).ok().filter(|bytes| *bytes > 0)?;
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let retained = state.retained_bytes.checked_add(bytes)?;
        if retained > state.limit_bytes {
            return None;
        }
        state.retained_bytes = retained;
        state.peak_bytes = state.peak_bytes.max(retained);
        state.buffers += 1;
        Some(Lease {
            budget: self.clone(),
            bytes,
        })
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        let mut state = self.budget.0.lock().unwrap_or_else(|error| error.into_inner());
        state.retained_bytes -= self.bytes;
        state.buffers -= 1;
    }
}
