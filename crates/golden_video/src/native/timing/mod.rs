//! Opt-in bounded worker telemetry. Query results describe their own context, never a
//! cross-context critical path or scanout. Diagnostic pressure cannot affect frame admission.

mod clock;
mod cpu;
mod event;
mod gpu;
mod scheduler;
mod sink;

pub(crate) use clock::Clock;
pub(crate) use cpu::CpuScope;
pub use event::*;
pub(crate) use gpu::GpuTiming;
pub(crate) use scheduler::ScheduleClockSource;
pub(crate) use sink::Recorder;

#[cfg(test)]
mod tests;
