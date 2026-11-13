//! Opt-in bounded worker telemetry. Query results describe their own context, never a
//! cross-context critical path or scanout. Diagnostic pressure cannot affect frame admission.

mod clock;
mod cpu;
mod event;
mod gpu;
mod plan;
mod queue;
mod scheduler;
mod sink;
mod submission;

pub(crate) use clock::Clock;
pub(crate) use cpu::CpuScope;
pub use event::*;
pub(crate) use gpu::GpuTiming;
pub use queue::OutputQueueBound;
pub(crate) use scheduler::ScheduleClockSource;
pub(crate) use sink::Recorder;
pub use submission::SourceSubmissionSink;

#[cfg(test)]
mod tests;
