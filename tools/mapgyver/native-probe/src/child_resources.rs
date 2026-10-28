//! Sparse snapshots and bounded nonblocking completion markers on an owned GPU worker.

use crate::{ProbeResult, d3d::Gpu};
use mapgyver_native_probe::{
    allocation::Snapshot,
    resource_log::{MAX_SAMPLES, Report, Sample},
};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use windows::Win32::Graphics::Direct3D11::*;

pub struct Recorder {
    start: Instant,
    next: Duration,
    queries: Vec<ID3D11Query>,
    pending: VecDeque<(usize, u64)>,
    samples: Vec<Sample>,
    markers: u32,
    skipped: u32,
}

impl Recorder {
    pub fn new(gpu: &Gpu) -> ProbeResult<Self> {
        let mut queries = Vec::with_capacity(4);
        for _ in 0..4 {
            let mut query = None;
            // SAFETY: event queries stay owned until their submitted serial completes.
            unsafe {
                gpu.device.CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: D3D11_QUERY_EVENT,
                        MiscFlags: 0,
                    },
                    Some(&mut query),
                )?
            };
            queries.push(query.ok_or("child resource query absent")?);
        }
        Ok(Self {
            start: Instant::now(),
            next: Duration::from_secs(1),
            queries,
            pending: VecDeque::with_capacity(4),
            samples: Vec::new(),
            markers: 0,
            skipped: 0,
        })
    }

    pub fn tick(&mut self, gpu: &Gpu) -> ProbeResult<()> {
        while let Some(&(slot, serial)) = self.pending.front() {
            if !crate::query_data::event(&gpu.context, &self.queries[slot])? {
                break;
            }
            gpu.allocations.complete(serial);
            self.pending.pop_front();
        }
        if self.start.elapsed() >= self.next {
            self.record(gpu, None)?;
            self.next = self.start.elapsed() + Duration::from_secs(1);
            if let Some(slot) = (0..4).find(|s| !self.pending.iter().any(|p| p.0 == *s)) {
                // SAFETY: submit only a free event slot, after all previously issued GPU work.
                unsafe {
                    gpu.context.End(&self.queries[slot]);
                    gpu.context.Flush();
                }
                self.pending.push_back((slot, gpu.allocations.submitted()));
                self.markers += 1;
            } else {
                self.skipped += 1;
            }
        }
        Ok(())
    }

    pub fn record(&mut self, gpu: &Gpu, source_phase: Option<u32>) -> ProbeResult<()> {
        if self.samples.len() == MAX_SAMPLES {
            return Err("child resource sample capacity exceeded".into());
        }
        self.samples.push(Sample {
            elapsed_seconds: self.start.elapsed().as_secs_f64(),
            source_phase,
            allocations: gpu.allocations.snapshot(),
            gpu_local_nonlocal: gpu.memory()?,
        });
        Ok(())
    }

    pub fn finish(self, gpu: &Gpu, before_resource_drop: Snapshot) -> ProbeResult<Report> {
        let after_resource_drop = gpu.allocations.snapshot();
        gpu.retire()?;
        let after_fence = gpu.allocations.snapshot();
        Ok(Report {
            before_resource_drop,
            after_resource_drop,
            after_fence,
            operations: gpu.allocations.operations(),
            completion_markers: self.markers,
            skipped_completion_markers: self.skipped,
            sample_count: self.samples.len(),
            samples: self.samples,
        })
    }
}
