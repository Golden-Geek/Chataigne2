//! Capacity follows actual pool ownership, including cancelled workers still retiring.
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone)]
pub(in crate::native) struct Budget(Arc<State>);
struct State {
    limit: u64,
    used: AtomicU64,
}
pub(in crate::native) struct Lease {
    budget: Budget,
    bytes: u64,
}
impl Budget {
    pub fn used(&self) -> u64 {
        self.0.used.load(Ordering::Acquire)
    }
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

impl Lease {
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[cfg(test)]
mod tests;
