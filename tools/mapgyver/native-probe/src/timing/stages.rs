//! Five optional markers share the frame's disjoint scope and completion event.

use super::query;
use crate::{ProbeResult, d3d::Gpu, query_data};
use mapgyver_native_probe::gpu_stage_timing::{Boundary, INTERMEDIATE_COUNT};
use windows::Win32::Graphics::Direct3D11::{D3D11_QUERY_TIMESTAMP, ID3D11Query};

pub(super) struct Queries {
    queries: [ID3D11Query; INTERMEDIATE_COUNT],
    issued: usize,
}

pub(super) enum Read {
    Incomplete,
    Pending,
    Ready([u64; INTERMEDIATE_COUNT]),
}

impl Queries {
    pub(super) fn new(gpu: &Gpu) -> ProbeResult<Self> {
        Ok(Self {
            queries: [
                query(gpu, D3D11_QUERY_TIMESTAMP)?,
                query(gpu, D3D11_QUERY_TIMESTAMP)?,
                query(gpu, D3D11_QUERY_TIMESTAMP)?,
                query(gpu, D3D11_QUERY_TIMESTAMP)?,
                query(gpu, D3D11_QUERY_TIMESTAMP)?,
            ],
            issued: 0,
        })
    }

    pub(super) fn reset(&mut self) {
        self.issued = 0;
    }

    pub(super) fn mark(&mut self, gpu: &Gpu, boundary: Boundary) -> ProbeResult<()> {
        if self.issued != boundary.index() {
            return Err("GPU stage markers must be issued once in declared order".into());
        }
        // SAFETY: the active frame's creating immediate context/disjoint scope owns this query.
        unsafe {
            gpu.context.End(&self.queries[self.issued]);
        }
        self.issued += 1;
        Ok(())
    }

    pub(super) fn read(&self, gpu: &Gpu) -> ProbeResult<Read> {
        if self.issued != self.queries.len() {
            return Ok(Read::Incomplete);
        }
        let mut ticks = [0; INTERMEDIATE_COUNT];
        for (query, tick) in self.queries.iter().zip(&mut ticks) {
            let Some(value) = query_data::timestamp(&gpu.context, query)? else {
                return Ok(Read::Pending);
            };
            *tick = value;
        }
        Ok(Read::Ready(ticks))
    }
}
