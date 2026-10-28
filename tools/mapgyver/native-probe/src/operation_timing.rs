//! Bounded CPU operation attribution. Wall time includes scheduling/preemption inside a call.

use crate::statistics::{Distribution, Series};
use serde::Serialize;
use std::time::Instant;

pub const OPERATION_COUNT: usize = 19;
pub const OPERATION_NAMES: [&str; OPERATION_COUNT] = [
    "resources",
    "gpu_admission",
    "query_begin",
    "layout",
    "source_poll_a",
    "source_poll_b",
    "compose",
    "send_spout",
    "observer_control",
    "output_target",
    "output_draw",
    "output_present",
    "editor_target",
    "editor_draw",
    "editor_present",
    "status",
    "query_end",
    "gpu_signal",
    "gpu_flush",
];
pub const OPERATION_THRESHOLD_NS: u64 = 4_000_000;
pub const FRAME_PERIOD_NS: u64 = 16_666_667;
pub const OUTLIER_CAPACITY: usize = 128;

#[derive(Clone, Copy)]
pub enum Operation {
    Resources,
    GpuAdmission,
    QueryBegin,
    Layout,
    PollSourceA,
    PollSourceB,
    Compose,
    SendSpout,
    ObserverControl,
    OutputTarget,
    OutputDraw,
    OutputPresent,
    EditorTarget,
    EditorDraw,
    EditorPresent,
    Status,
    QueryEnd,
    GpuSignal,
    GpuFlush,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Stamp {
    pub frame: u64,
    pub qpc: u64,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Span {
    pub offset_ns: u64,
    pub elapsed_ns: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Frame {
    pub stamp: Stamp,
    pub scheduled_delay_ns: u64,
    pub elapsed_ns: u64,
    pub gpu_query_skipped: bool,
    pub gpu_admission_busy: bool,
    pub spans: [Option<Span>; OPERATION_COUNT],
}

impl Frame {
    fn new(stamp: Stamp, scheduled_delay_ns: u64) -> Self {
        Self {
            stamp,
            scheduled_delay_ns,
            elapsed_ns: 0,
            gpu_query_skipped: false,
            gpu_admission_busy: false,
            spans: [None; OPERATION_COUNT],
        }
    }

    fn insert(&mut self, operation: Operation, span: Span) {
        let entry = &mut self.spans[operation as usize];
        assert!(entry.is_none(), "an operation runs at most once per frame");
        *entry = Some(span);
    }

    fn outlier(&self) -> bool {
        self.elapsed_ns >= FRAME_PERIOD_NS
            || self.scheduled_delay_ns >= FRAME_PERIOD_NS
            || self.gpu_query_skipped
            || self.gpu_admission_busy
            || self
                .spans
                .iter()
                .flatten()
                .any(|span| span.elapsed_ns >= OPERATION_THRESHOLD_NS)
    }
}

pub struct FrameTimer {
    started: Instant,
    frame: Frame,
}

pub enum GpuWork {
    Submitted { profiling_skipped: bool },
    Coalesced,
}

impl FrameTimer {
    pub fn new(started: Instant, scheduled: Instant, stamp: Stamp) -> Self {
        Self {
            started,
            frame: Frame::new(stamp, started.saturating_duration_since(scheduled).as_nanos() as u64),
        }
    }

    pub fn measure<T>(&mut self, operation: Operation, run: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let result = run();
        let end = Instant::now();
        self.frame.insert(
            operation,
            Span {
                offset_ns: start.duration_since(self.started).as_nanos() as u64,
                elapsed_ns: end.duration_since(start).as_nanos() as u64,
            },
        );
        result
    }

    pub fn finish(mut self, work: GpuWork) -> Frame {
        self.frame.elapsed_ns = self.started.elapsed().as_nanos() as u64;
        match work {
            GpuWork::Submitted { profiling_skipped } => self.frame.gpu_query_skipped = profiling_skipped,
            GpuWork::Coalesced => self.frame.gpu_admission_busy = true,
        }
        self.frame
    }
}

#[derive(Serialize)]
pub struct Outliers<T> {
    pub total: u64,
    pub omitted: u64,
    pub events: Vec<T>,
    pub maximum: Option<T>,
    maximum_score: u64,
}

impl<T: Clone> Default for Outliers<T> {
    fn default() -> Self {
        Self {
            total: 0,
            omitted: 0,
            events: Vec::with_capacity(OUTLIER_CAPACITY),
            maximum: None,
            maximum_score: 0,
        }
    }
}

impl<T: Clone> Outliers<T> {
    fn push(&mut self, event: T, score: u64) {
        self.total += 1;
        if self.maximum.is_none() || score > self.maximum_score {
            self.maximum_score = score;
            self.maximum = Some(event.clone());
        }
        if self.events.len() < OUTLIER_CAPACITY {
            self.events.push(event);
        } else {
            self.omitted += 1;
        }
    }
}

#[derive(Default, Serialize)]
pub struct Window {
    pub frames: u64,
    pub outliers: Outliers<Frame>,
}

#[derive(Serialize)]
pub struct Report {
    pub qpc_frequency: u64,
    pub operation_names: [&'static str; OPERATION_COUNT],
    pub operation_threshold_ns: u64,
    pub frame_period_ns: u64,
    pub outlier_capacity_per_window: usize,
    pub warmup: Window,
    pub sampled: Window,
    pub sampled_by_operation: [Option<Distribution>; OPERATION_COUNT],
    pub truncated_operation_samples: u32,
}

pub struct Recorder {
    frequency: u64,
    warmup: Window,
    sampled: Window,
    operations: [Series; OPERATION_COUNT],
}

impl Recorder {
    pub fn new(frequency: u64) -> Self {
        assert!(frequency > 0);
        Self {
            frequency,
            warmup: Window::default(),
            sampled: Window::default(),
            operations: std::array::from_fn(|_| Series::default()),
        }
    }

    pub fn record(&mut self, frame: Frame, collecting: bool) {
        if collecting {
            for (series, span) in self.operations.iter_mut().zip(&frame.spans) {
                if let Some(span) = span {
                    series.push(span.elapsed_ns as f64 / 1_000_000.0);
                }
            }
        }
        let window = if collecting {
            &mut self.sampled
        } else {
            &mut self.warmup
        };
        window.frames += 1;
        if frame.outlier() {
            let score = frame.elapsed_ns.max(frame.scheduled_delay_ns);
            window.outliers.push(frame, score);
        }
    }

    pub fn finish(self) -> Report {
        let truncated_operation_samples = self.operations.iter().map(|series| series.truncated).sum();
        Report {
            qpc_frequency: self.frequency,
            operation_names: OPERATION_NAMES,
            operation_threshold_ns: OPERATION_THRESHOLD_NS,
            frame_period_ns: FRAME_PERIOD_NS,
            outlier_capacity_per_window: OUTLIER_CAPACITY,
            warmup: self.warmup,
            sampled: self.sampled,
            sampled_by_operation: self.operations.map(|mut series| series.distribution()),
            truncated_operation_samples,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct GpuEvent {
    pub stamp: Stamp,
    pub observed_completed_qpc: u64,
    pub start_ticks: u64,
    pub end_ticks: u64,
    pub frequency: u64,
}

impl Outliers<GpuEvent> {
    pub fn record_gpu(&mut self, event: GpuEvent) {
        assert!(event.frequency > 0 && event.end_ticks >= event.start_ticks);
        let ns =
            ((u128::from(event.end_ticks - event.start_ticks) * 1_000_000_000) / u128::from(event.frequency)) as u64;
        if ns >= FRAME_PERIOD_NS {
            self.push(event, ns);
        }
    }
}

#[cfg(test)]
mod tests;
