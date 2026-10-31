use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Default)]
struct Lifetime {
    complete: Arc<AtomicBool>,
}
impl Drop for Lifetime {
    fn drop(&mut self) {
        self.complete.store(true, Ordering::Release);
    }
}

/// Payload-free retention through queued input, preparation, live use and resource retirement.
/// Clones keep the same lifetime; observers never prolong it. No app/native object enters Golden.
#[derive(Clone, Debug, Default)]
pub struct PlanLifetime(Arc<Lifetime>);
#[derive(Clone, Debug)]
/// Read-only final-release observation; it never keeps a plan alive.
pub struct PlanLifetimeObserver(Arc<AtomicBool>);
impl PlanLifetime {
    /// Observe release without retaining input, compiled plans or backend resources.
    pub fn observer(&self) -> PlanLifetimeObserver {
        PlanLifetimeObserver(self.0.complete.clone())
    }
    pub(crate) const fn retained_heap_bytes(&self) -> usize {
        // Conservative charge for the lifetime and completion Arc allocations, with alignment.
        size_of::<[usize; 8]>()
    }
}
impl PlanLifetimeObserver {
    /// True only after all input/compiled/recovery/retirement owners released their token.
    pub fn is_complete(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
