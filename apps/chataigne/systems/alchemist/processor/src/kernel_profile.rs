#[cfg(feature = "kernel-profiling")]
use std::{cell::Cell, time::Instant};

/// Cumulative compiled-graph work on the current thread, excluding processor lane setup.
#[cfg(feature = "kernel-profiling")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessorKernelProfile {
    pub elapsed_ns: u64,
    pub evaluations: u64,
}

#[cfg(feature = "kernel-profiling")]
thread_local! {
    static PROFILE: Cell<ProcessorKernelProfile> = Cell::new(ProcessorKernelProfile::default());
}

#[cfg(feature = "kernel-profiling")]
#[must_use]
pub fn processor_kernel_profile_snapshot() -> ProcessorKernelProfile {
    PROFILE.with(Cell::get)
}

#[inline]
pub(crate) fn profile_kernel<T>(evaluate: impl FnOnce() -> T) -> T {
    #[cfg(feature = "kernel-profiling")]
    let started = Instant::now();
    let result = evaluate();
    #[cfg(feature = "kernel-profiling")]
    PROFILE.with(|profile| {
        let previous = profile.get();
        profile.set(ProcessorKernelProfile {
            elapsed_ns: previous.elapsed_ns.saturating_add(started.elapsed().as_nanos() as u64),
            evaluations: previous.evaluations.saturating_add(1),
        });
    });
    result
}
