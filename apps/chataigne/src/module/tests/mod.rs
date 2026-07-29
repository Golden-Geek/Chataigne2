use std::sync::{Mutex, MutexGuard};

mod multiplex;
mod runtime_scaling;

static PERFORMANCE_TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock_performance_test() -> MutexGuard<'static, ()> {
    PERFORMANCE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
