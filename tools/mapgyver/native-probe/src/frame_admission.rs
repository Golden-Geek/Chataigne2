//! Native completion markers for the render worker, bound to their creating GPU/context.

use crate::{ProbeResult, d3d::Gpu, query_data};
use mapgyver_native_probe::gpu_admission::{Admission, CAPACITY, Report, Reservation};
use serde::Serialize;
use windows::Win32::Graphics::Direct3D11::*;

#[derive(Serialize)]
pub struct FinalReport {
    pub mechanism: &'static str,
    #[serde(flatten)]
    pub admission: Report,
    pub pending_at_final_poll: usize,
}

pub struct FrameAdmission<'gpu> {
    gpu: &'gpu Gpu,
    queries: Vec<ID3D11Query>,
    serials: [u64; CAPACITY],
    timeline: Option<crate::completion::Completion<'gpu>>,
    values: [u64; CAPACITY],
    admission: Admission,
}

/// A reserved batch cannot become pending until its marker has been submitted to the driver.
#[must_use = "flush the signaled batch before pacing or reserving another frame"]
pub struct SignaledBatch<'admission, 'gpu> {
    admission: &'admission mut FrameAdmission<'gpu>,
    reservation: Reservation,
}

impl SignaledBatch<'_, '_> {
    pub fn flush(self) {
        // SAFETY: this is the creating immediate context, after all frame/marker commands.
        // No application wait is added; the driver's Flush call can itself block.
        unsafe { self.admission.gpu.context.Flush() };
        self.admission.admission.submitted(self.reservation);
    }
}

impl<'gpu> FrameAdmission<'gpu> {
    /// Explicit diagnostic backend; event readiness can stall on timestamp-free batches.
    pub fn event_queries(gpu: &'gpu Gpu) -> ProbeResult<Self> {
        let mut queries = Vec::with_capacity(CAPACITY);
        for _ in 0..CAPACITY {
            let mut query = None;
            // SAFETY: an event query for each fixed batch slot on this retained device.
            unsafe {
                gpu.device.CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: D3D11_QUERY_EVENT,
                        MiscFlags: 0,
                    },
                    Some(&mut query),
                )?;
            }
            queries.push(query.ok_or("missing frame admission query")?);
        }
        Ok(Self {
            gpu,
            queries,
            serials: [0; CAPACITY],
            timeline: None,
            values: [0; CAPACITY],
            admission: Admission::default(),
        })
    }

    pub fn new(gpu: &'gpu Gpu) -> ProbeResult<Self> {
        Ok(Self {
            gpu,
            queries: Vec::new(),
            serials: [0; CAPACITY],
            timeline: Some(crate::completion::Completion::new(gpu)?),
            values: [0; CAPACITY],
            admission: Admission::default(),
        })
    }

    pub fn reserve(&mut self, collecting: bool) -> ProbeResult<Option<Reservation>> {
        self.admission.reserve(collecting, |slot| -> ProbeResult<bool> {
            let completed = if let Some(timeline) = &self.timeline {
                timeline.complete(self.values[slot])?
            } else {
                query_data::event(&self.gpu.context, &self.queries[slot])?
            };
            if completed {
                self.gpu.allocations.complete(self.serials[slot]);
            }
            Ok(completed)
        })
    }

    pub fn submit(&mut self, reservation: Reservation) -> ProbeResult<()> {
        self.signal(reservation)?.flush();
        Ok(())
    }

    pub fn signal(&mut self, reservation: Reservation) -> ProbeResult<SignaledBatch<'_, 'gpu>> {
        let slot = reservation.slot();
        self.serials[slot] = self.gpu.allocations.submitted();
        if let Some(timeline) = &mut self.timeline {
            self.values[slot] = timeline.signal()?;
        } else {
            // SAFETY: the reserved event slot is free; its marker follows frame work.
            unsafe {
                self.gpu.context.End(&self.queries[slot]);
            }
        }
        Ok(SignaledBatch {
            admission: self,
            reservation,
        })
    }

    pub fn snapshot(&self) -> Report {
        self.admission.report()
    }

    /// Call only after the creating context's successful retirement fence.
    pub fn finish_after_fence(mut self) -> ProbeResult<FinalReport> {
        let pending_at_final_poll = self.admission.report().pending;
        self.admission.poll(|slot| {
            if let Some(timeline) = &self.timeline {
                timeline.complete(self.values[slot])
            } else {
                query_data::event(&self.gpu.context, &self.queries[slot])
            }
        })?;
        let report = self.admission.report();
        if report.pending != 0 || report.reserved || report.completed != report.submitted {
            return Err("frame batches retained after final context fence".into());
        }
        Ok(FinalReport {
            mechanism: if self.timeline.is_some() {
                "timeline_fence"
            } else {
                "event_query"
            },
            admission: report,
            pending_at_final_poll,
        })
    }
}
