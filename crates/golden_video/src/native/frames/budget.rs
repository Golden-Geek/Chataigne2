//! One coherent admission/accounting boundary for all retained file-source GPU storage.
use std::sync::{Arc, Mutex};

use serde::Serialize;

/// Actual source-worker payload submissions, independent of later renderer sampling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SourceUploadTotals {
    /// Actual payload API calls; GPU completion is observed separately.
    pub calls: u64,
    /// Actual bytes passed to those calls; excludes fixed shader/constants setup.
    pub bytes: u64,
    /// An exhausted counter invalidates complete accounting instead of wrapping.
    pub overflowed: bool,
}

/// Lifetime totals for one budget, shared by all producers and retained after retirement.
/// Hardware-frame copies do not submit CPU payloads and leave both classes unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SourceUploadSnapshot {
    /// Tight RGBA payload uploaded by software-decoded source workers.
    pub software: SourceUploadTotals,
    /// Compressed BC payload uploaded by HAP source workers.
    pub hap: SourceUploadTotals,
}

#[cfg(any(feature = "ffmpeg", test))]
#[derive(Clone, Copy)]
pub(super) enum UploadKind {
    Software,
    Hap,
}

#[cfg(any(feature = "ffmpeg", test))]
impl SourceUploadTotals {
    fn record(&mut self, bytes: u64) {
        if self.overflowed {
            return;
        }
        match self.calls.checked_add(1).zip(self.bytes.checked_add(bytes)) {
            Some((calls, bytes)) => {
                self.calls = calls;
                self.bytes = bytes;
            }
            None => self.overflowed = true,
        }
    }
}

/// Logical payload retained by allocations of one source-storage class.
/// Shared image/frame observers retain the same allocation rather than adding another count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct GpuFrameAllocation {
    /// Live allocation groups, not decoded frames or texture-array slices.
    pub allocations: u64,
    /// Admitted logical payload; physical driver overhead remains unmeasured.
    pub bytes: u64,
}

/// A coherent inventory of one shared source budget, including retiring old generations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct GpuFrameBudgetSnapshot {
    /// Stable budget lifetime; clones and renderer recreations retain this identity.
    pub id: uuid::Uuid,
    /// Unchanged admission ceiling shared by every class below.
    pub limit_bytes: u64,
    /// Sum of all four current classes from the same locked observation.
    pub retained_bytes: u64,
    /// Maximum simultaneous admitted payload over this budget's lifetime.
    pub peak_bytes: u64,
    /// FFmpeg's fixed decoder arrays, including aligned NV12/P010 planes and all pool slices.
    pub decoder_pools: GpuFrameAllocation,
    /// Hardware-frame conversion input, constants and three retained RGBA output slots.
    pub hardware_producers: GpuFrameAllocation,
    /// HAP compressed input, constants and three retained RGBA output slots.
    pub hap_producers: GpuFrameAllocation,
    /// Three RGBA slots uploaded by a software-source worker.
    pub software_producers: GpuFrameAllocation,
    /// Opt-in actual upload totals. None means unmeasured, including after retirement.
    pub uploads: Option<SourceUploadSnapshot>,
}

#[cfg(any(feature = "ffmpeg", test))]
#[derive(Clone, Copy)]
pub(crate) enum AllocationKind {
    DecoderPool,
    HardwareProducer,
    HapProducer,
    SoftwareProducer,
}
#[cfg(any(feature = "ffmpeg", test))]
impl GpuFrameBudgetSnapshot {
    fn class(&mut self, kind: AllocationKind) -> &mut GpuFrameAllocation {
        match kind {
            AllocationKind::DecoderPool => &mut self.decoder_pools,
            AllocationKind::HardwareProducer => &mut self.hardware_producers,
            AllocationKind::HapProducer => &mut self.hap_producers,
            AllocationKind::SoftwareProducer => &mut self.software_producers,
        }
    }
}

/// Shared admission for decoder pools, conversion images and retained bridge slots.
/// Renderer-owned targets require separate admission from the same adapter allowance.
#[derive(Clone)]
pub struct GpuFrameBudget(Arc<Mutex<GpuFrameBudgetSnapshot>>);
impl GpuFrameBudget {
    /// A zero limit deliberately rejects every allocation.
    pub fn new(max_bytes: u64) -> Self {
        Self(Arc::new(Mutex::new(GpuFrameBudgetSnapshot {
            id: uuid::Uuid::new_v4(),
            limit_bytes: max_bytes,
            ..Default::default()
        })))
    }
    /// Create a fresh budget with complete source-worker upload totals from its first use.
    /// This opt-in adds one short accounting lock after each actual payload submission.
    /// Ordinary budgets perform no per-frame accounting and report uploads as unmeasured.
    pub fn new_with_upload_accounting(max_bytes: u64) -> Self {
        let budget = Self::new(max_bytes);
        budget.0.lock().unwrap().uploads = Some(SourceUploadSnapshot::default());
        budget
    }
    #[cfg(feature = "ffmpeg")]
    pub(super) fn upload_accounting(&self) -> Option<Self> {
        self.snapshot().uploads.map(|_| self.clone())
    }
    #[cfg(any(feature = "ffmpeg", test))]
    pub(super) fn record_upload(&self, kind: UploadKind, bytes: u64) {
        if let Some(uploads) = &mut self.0.lock().unwrap().uploads {
            match kind {
                UploadKind::Software => &mut uploads.software,
                UploadKind::Hap => &mut uploads.hap,
            }
            .record(bytes);
        }
    }
    /// Known admitted GPU storage still retained by producers, images or in-flight readers.
    pub fn retained_bytes(&self) -> u64 {
        self.snapshot().retained_bytes
    }
    /// Read all categories together without retaining a decoder, image or native device.
    pub fn snapshot(&self) -> GpuFrameBudgetSnapshot {
        *self.0.lock().unwrap()
    }
    #[cfg(feature = "ffmpeg")]
    pub(crate) fn reserve(&self, bytes: u64, kind: AllocationKind) -> Option<Arc<dyn Send + Sync>> {
        self.claim(bytes, kind)
    }
    #[cfg(any(feature = "ffmpeg", test))]
    pub(super) fn claim(&self, bytes: u64, kind: AllocationKind) -> Option<Arc<dyn Send + Sync>> {
        let mut state = self.0.lock().unwrap();
        let total = state.retained_bytes.checked_add(bytes)?;
        if bytes == 0 || total > state.limit_bytes {
            return None;
        }
        let class = state.class(kind);
        let count = class.allocations.checked_add(1)?;
        let retained = class.bytes.checked_add(bytes)?;
        *class = GpuFrameAllocation {
            allocations: count,
            bytes: retained,
        };
        state.retained_bytes = total;
        state.peak_bytes = state.peak_bytes.max(total);
        Some(Arc::new(Lease {
            budget: self.clone(),
            bytes,
            kind,
        }))
    }
}

#[cfg(any(feature = "ffmpeg", test))]
struct Lease {
    budget: GpuFrameBudget,
    bytes: u64,
    kind: AllocationKind,
}
#[cfg(any(feature = "ffmpeg", test))]
impl Drop for Lease {
    fn drop(&mut self) {
        let mut state = self.budget.0.lock().unwrap();
        let class = state.class(self.kind);
        class.allocations -= 1;
        class.bytes -= self.bytes;
        state.retained_bytes -= self.bytes;
    }
}

#[cfg(test)]
mod tests;
