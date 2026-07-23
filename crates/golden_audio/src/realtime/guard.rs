use std::cell::Cell;

thread_local! {
    static REALTIME_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Marks the current lexical scope as callback-owned in debug and test builds.
///
/// The guard is deliberately allocation-free. Control-thread APIs call
/// [`assert_not_realtime`] so accidental use from a callback fails close to the boundary.
#[derive(Debug)]
#[must_use]
pub struct RealtimeScope {
    active: bool,
}

impl RealtimeScope {
    #[inline]
    pub fn enter() -> Self {
        if cfg!(debug_assertions) {
            REALTIME_DEPTH.with(|depth| depth.set(depth.get().saturating_add(1)));
        }
        Self {
            active: cfg!(debug_assertions),
        }
    }
}

impl Drop for RealtimeScope {
    #[inline]
    fn drop(&mut self) {
        if self.active {
            REALTIME_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
        }
    }
}

#[inline]
#[must_use]
pub fn is_realtime_thread() -> bool {
    cfg!(debug_assertions) && REALTIME_DEPTH.with(|depth| depth.get() != 0)
}

#[inline]
pub fn assert_not_realtime(operation: &'static str) {
    assert!(
        !is_realtime_thread(),
        "{operation} is forbidden from a realtime callback"
    );
}
