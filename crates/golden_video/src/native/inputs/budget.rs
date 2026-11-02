//! Capacity follows actual pool ownership, including cancelled workers still retiring.
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone)]
pub(super) struct Budget(Arc<State>);
struct State {
    limit: u64,
    used: AtomicU64,
}
pub(super) struct Lease {
    budget: Budget,
    bytes: u64,
}
impl Budget {
    pub fn new(limit: u64) -> Self {
        Self(Arc::new(State {
            limit,
            used: AtomicU64::new(0),
        }))
    }
    pub fn claim(&self, bytes: u64) -> Option<Lease> {
        self.0
            .used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(bytes).filter(|next| *next <= self.0.limit)
            })
            .ok()?;
        Some(Lease {
            budget: self.clone(),
            bytes,
        })
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.budget.0.used.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests;
