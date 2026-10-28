//! Bounded probe telemetry. Timestamp/event queries are polled only on the render worker.

mod stages;

use std::time::{Duration, Instant};

use serde::Serialize;
use windows::Win32::Graphics::Direct3D11::*;

use crate::{ProbeResult, d3d::Gpu};
use mapgyver_native_probe::gpu_stage_timing::{self, Boundary};
use mapgyver_native_probe::operation_timing::{GpuEvent, Outliers, Stamp};
use mapgyver_native_probe::pipeline_timing::{GpuSpan, GpuTrace};

pub(crate) use mapgyver_native_probe::statistics::{Distribution, Series};

#[derive(Serialize)]
pub struct TimingReport {
    pub cpu_submission_work: Option<Distribution>,
    pub worker_start_intervals: Option<Distribution>,
    pub gpu_execution: Option<Distribution>,
    pub skipped_gpu_queries: u32,
    pub invalid_gpu_queries: u32,
    pub pending_gpu_queries: usize,
    pub truncated_samples: u32,
    pub gpu_outliers: Outliers<GpuEvent>,
    pub gpu_trace: Option<GpuTrace>,
    pub gpu_stages: Option<gpu_stage_timing::Trace>,
}

struct Slot {
    start: ID3D11Query,
    end: ID3D11Query,
    disjoint: ID3D11Query,
    done: ID3D11Query,
    pending: bool,
    collected: bool,
    serial: u64,
    stamp: Stamp,
    stages: Option<stages::Queries>,
}

pub struct Timing {
    slots: Vec<Slot>,
    cursor: usize,
    active: Option<usize>,
    previous_start: Option<Instant>,
    cpu: Series,
    intervals: Series,
    gpu: Series,
    skipped: u32,
    invalid: u32,
    collecting: bool,
    gpu_outliers: Outliers<GpuEvent>,
    gpu_trace: Option<GpuTrace>,
    gpu_stages: Option<gpu_stage_timing::Trace>,
}

fn query(gpu: &Gpu, kind: D3D11_QUERY) -> ProbeResult<ID3D11Query> {
    let mut result = None;
    // SAFETY: stack descriptor/output, query owned through windows-rs.
    unsafe {
        gpu.device.CreateQuery(
            &D3D11_QUERY_DESC {
                Query: kind,
                MiscFlags: 0,
            },
            Some(&mut result),
        )?
    };
    Ok(result.ok_or("D3D11 returned no timing query")?)
}

impl Timing {
    pub fn new(gpu: &Gpu) -> ProbeResult<Self> {
        let slots = (0..4)
            .map(|_| {
                Ok(Slot {
                    start: query(gpu, D3D11_QUERY_TIMESTAMP)?,
                    end: query(gpu, D3D11_QUERY_TIMESTAMP)?,
                    disjoint: query(gpu, D3D11_QUERY_TIMESTAMP_DISJOINT)?,
                    done: query(gpu, D3D11_QUERY_EVENT)?,
                    pending: false,
                    collected: false,
                    serial: 0,
                    stamp: Stamp { frame: 0, qpc: 0 },
                    stages: None,
                })
            })
            .collect::<ProbeResult<Vec<_>>>()?;
        Ok(Self {
            slots,
            cursor: 0,
            active: None,
            previous_start: None,
            cpu: Series::default(),
            intervals: Series::default(),
            gpu: Series::default(),
            skipped: 0,
            invalid: 0,
            collecting: false,
            gpu_outliers: Outliers::default(),
            gpu_trace: None,
            gpu_stages: None,
        })
    }

    /// Capture warmup too: a sampled composite can retain a source published before warmup ends.
    pub fn traced(gpu: &Gpu) -> ProbeResult<Self> {
        let mut timing = Self::new(gpu)?;
        timing.gpu_trace = Some(GpuTrace::default());
        Ok(timing)
    }

    pub fn staged(gpu: &Gpu) -> ProbeResult<Self> {
        let mut timing = Self::traced(gpu)?;
        for slot in &mut timing.slots {
            slot.stages = Some(stages::Queries::new(gpu)?);
        }
        timing.gpu_stages = Some(gpu_stage_timing::Trace::default());
        Ok(timing)
    }

    pub fn begin(&mut self, gpu: &Gpu, start: Instant, collect: bool, stamp: Stamp) -> ProbeResult<bool> {
        if self.active.is_some() {
            return Err("previous GPU timing scope is still active".into());
        }
        self.collecting = collect;
        self.reap(gpu)?;
        if !collect {
            self.previous_start = None;
        } else if let Some(previous) = self.previous_start.replace(start) {
            self.intervals
                .push(start.duration_since(previous).as_secs_f64() * 1000.0);
        }
        let index = self.cursor;
        self.cursor = (self.cursor + 1) % self.slots.len();
        if self.slots[index].pending {
            if let Some(trace) = &mut self.gpu_trace {
                trace.skipped += 1;
            }
            if let Some(trace) = &mut self.gpu_stages {
                trace.skipped += 1;
            }
            if collect {
                self.skipped += 1;
            }
            return Ok(false);
        }
        self.active = Some(index);
        self.slots[index].collected = collect;
        self.slots[index].stamp = stamp;
        if let Some(stages) = &mut self.slots[index].stages {
            stages.reset();
        }
        // SAFETY: only one disjoint scope is active on this immediate context.
        unsafe {
            if collect || self.gpu_trace.is_some() {
                gpu.context.Begin(&self.slots[index].disjoint);
                gpu.context.End(&self.slots[index].start);
            }
        }
        Ok(true)
    }

    pub fn checkpoint(&mut self, gpu: &Gpu, boundary: Boundary) -> ProbeResult<()> {
        if self.gpu_stages.is_some()
            && let Some(index) = self.active
        {
            self.slots[index]
                .stages
                .as_mut()
                .ok_or("GPU stage queries absent")?
                .mark(gpu, boundary)?;
        }
        Ok(())
    }

    /// Close diagnostic markers; the runtime owns submission/flush independently of this ring.
    pub fn end_gpu(&mut self, gpu: &Gpu) {
        if let Some(index) = self.active.take() {
            let slot = &mut self.slots[index];
            // SAFETY: close this frame's timestamp scope; event fences preceding query results.
            unsafe {
                if slot.collected || self.gpu_trace.is_some() {
                    gpu.context.End(&slot.end);
                    gpu.context.End(&slot.disjoint);
                }
                gpu.context.End(&slot.done);
            }
            slot.pending = true;
            slot.serial = gpu.allocations.submitted();
        }
    }

    pub fn end_cpu(&mut self, start: Instant) {
        if self.collecting {
            self.cpu.push(start.elapsed().as_secs_f64() * 1000.0);
        }
    }

    fn reap(&mut self, gpu: &Gpu) -> ProbeResult<()> {
        for slot in &mut self.slots {
            if !slot.pending {
                continue;
            }
            if !crate::query_data::event(&gpu.context, &slot.done)? {
                continue;
            }
            gpu.allocations.complete(slot.serial);
            if !slot.collected && self.gpu_trace.is_none() {
                slot.pending = false;
                continue;
            }
            let Some(start) = crate::query_data::timestamp(&gpu.context, &slot.start)? else {
                continue;
            };
            let Some(end) = crate::query_data::timestamp(&gpu.context, &slot.end)? else {
                continue;
            };
            let Some(clock) = crate::query_data::clock(&gpu.context, &slot.disjoint)? else {
                continue;
            };
            if clock.Disjoint.as_bool() || clock.Frequency == 0 || end < start {
                if slot.collected {
                    self.invalid += 1;
                }
                if let Some(trace) = &mut self.gpu_trace {
                    trace.invalid += 1;
                }
                if let Some(trace) = &mut self.gpu_stages {
                    trace.invalid += 1;
                }
            } else {
                let stage_ticks = if let Some(stages) = &slot.stages {
                    match stages.read(gpu)? {
                        stages::Read::Pending => continue,
                        stages::Read::Incomplete => None,
                        stages::Read::Ready(ticks) => Some(ticks),
                    }
                } else {
                    None
                };
                let event = GpuEvent {
                    stamp: slot.stamp,
                    observed_completed_qpc: crate::clock::now()?,
                    start_ticks: start,
                    end_ticks: end,
                    frequency: clock.Frequency,
                };
                if let Some(trace) = &mut self.gpu_stages {
                    if let Some(ticks) = stage_ticks {
                        let boundaries = [start, ticks[0], ticks[1], ticks[2], ticks[3], ticks[4], end];
                        if trace
                            .record(gpu_stage_timing::Frame {
                                frame: slot.stamp.frame,
                                submitted_qpc: slot.stamp.qpc,
                                observed_completed_qpc: event.observed_completed_qpc,
                                frequency: clock.Frequency,
                                collected: slot.collected,
                                boundaries,
                            })
                            .is_err()
                        {
                            trace.invalid += 1;
                        }
                    } else {
                        trace.invalid += 1;
                    }
                }
                if slot.collected {
                    self.gpu.push((end - start) as f64 / clock.Frequency as f64 * 1000.0);
                    self.gpu_outliers.record_gpu(event.clone());
                }
                if let Some(trace) = &mut self.gpu_trace {
                    trace.record(GpuSpan {
                        frame: slot.stamp.frame,
                        submitted_qpc: slot.stamp.qpc,
                        observed_completed_qpc: event.observed_completed_qpc,
                        start_ticks: start,
                        end_ticks: end,
                        frequency: clock.Frequency,
                        collected: slot.collected,
                    });
                }
            }
            slot.pending = false;
        }
        Ok(())
    }

    pub fn finish(mut self, gpu: &Gpu) -> ProbeResult<TimingReport> {
        // Only shutdown drains briefly; normal rendering never waits for timing queries.
        unsafe {
            gpu.context.Flush();
        }
        let deadline = Instant::now() + Duration::from_millis(100);
        loop {
            self.reap(gpu)?;
            if self.slots.iter().all(|slot| !slot.pending) || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if let Some(trace) = &mut self.gpu_trace {
            trace.pending = self.slots.iter().filter(|slot| slot.pending).count();
        }
        if let Some(trace) = &mut self.gpu_stages {
            trace.pending = self.slots.iter().filter(|slot| slot.pending).count();
        }
        Ok(TimingReport {
            cpu_submission_work: self.cpu.distribution(),
            worker_start_intervals: self.intervals.distribution(),
            gpu_execution: self.gpu.distribution(),
            skipped_gpu_queries: self.skipped,
            invalid_gpu_queries: self.invalid,
            pending_gpu_queries: self.slots.iter().filter(|slot| slot.pending && slot.collected).count(),
            truncated_samples: self.cpu.truncated + self.intervals.truncated + self.gpu.truncated,
            gpu_outliers: self.gpu_outliers,
            gpu_trace: self.gpu_trace,
            gpu_stages: self.gpu_stages,
        })
    }
}
