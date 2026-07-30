use std::sync::{Mutex, MutexGuard};

mod multiplex;
mod runtime_scaling;

static PERFORMANCE_TEST_LOCK: Mutex<()> = Mutex::new(());

struct PerformanceTestGuard {
    _performance: MutexGuard<'static, ()>,
    _shared_formula_dir: crate::test_support::ScopedSharedFormulaDir,
}

fn lock_performance_test() -> PerformanceTestGuard {
    let performance = PERFORMANCE_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let shared_formula_dir = crate::test_support::scoped_shared_formula_dir(None);
    crate::app::systems_alchemist_formula::reset_shared_formula_watcher_for_test();
    PerformanceTestGuard {
        _performance: performance,
        _shared_formula_dir: shared_formula_dir,
    }
}
