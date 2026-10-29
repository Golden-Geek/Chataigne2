//! Process-local DXGI observations. Shared-system capacity is not a measured segment budget.

use serde::Serialize;
use thiserror::Error;
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, DXGI_MEMORY_SEGMENT_GROUP,
    DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO, IDXGIAdapter1,
    IDXGIAdapter3, IDXGIFactory1,
};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::core::Interface;

use super::AdapterId;

/// One successful process-local segment query. A zero budget remains an observed zero;
/// failure to query is represented by the enclosing snapshot's absent segment instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct MemorySegmentSnapshot {
    /// Process usage in this segment, including observable SDK/driver allocations.
    pub current_usage_bytes: u64,
    /// Current OS-assigned process budget, distinct from adapter memory capacity.
    pub budget_bytes: u64,
    /// Maximum reservation currently available to this process.
    pub available_for_reservation_bytes: u64,
    /// Reservation currently held by this process; this API never changes it.
    pub current_reservation_bytes: u64,
}

/// Bounded metadata copied from the exact selected adapter, without creating a rendering
/// device. DXGI usage belongs to `process_id`, so query inside the renderer process when
/// measuring its allocations; querying from a collector measures the collector instead.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AdapterMemorySnapshot {
    /// Exact adapter LUID supplied by the caller; no adapter fallback occurs.
    pub adapter: AdapterId,
    /// Process whose DXGI segment usage and reservations were queried.
    pub process_id: u32,
    /// QPC observed after copying both segment results.
    pub observed_qpc: i64,
    /// QPC ticks per second for correlation with the run's sample window.
    pub qpc_frequency: i64,
    /// Adapter-declared dedicated video capacity, not current usage or budget.
    pub dedicated_video_bytes: u64,
    /// Adapter-declared shared-system capacity, never substituted for a DXGI budget.
    pub shared_system_capacity_bytes: u64,
    /// Node-zero LOCAL segment observation; absent when unavailable.
    pub local: Option<MemorySegmentSnapshot>,
    /// LOCAL query/interface failure, when the corresponding observation is absent.
    pub local_hresult: Option<i32>,
    /// Node-zero NON_LOCAL segment observation; absent when unavailable.
    pub nonlocal: Option<MemorySegmentSnapshot>,
    /// NON_LOCAL query/interface failure, when the corresponding observation is absent.
    pub nonlocal_hresult: Option<i32>,
}

impl AdapterMemorySnapshot {
    /// Caps the caller's workload allocation ceiling at half the measured nonzero LOCAL
    /// budget. Missing/zero counters leave admission unqualified, without a capacity or
    /// NON_LOCAL fallback. Integer division rounds down and cannot overflow.
    ///
    /// This is an admission limit, not an allocation or reservation. Budgets can change;
    /// callers must retain subsequent observations when assessing a complete run.
    pub fn admission_limit(&self, workload_ceiling: u64) -> Result<u64, MemoryAdmissionError> {
        let budget = self.local.ok_or(MemoryAdmissionError::MissingLocalBudget)?.budget_bytes;
        if budget == 0 {
            return Err(MemoryAdmissionError::ZeroLocalBudget);
        }
        Ok(workload_ceiling.min(budget / 2))
    }
}

/// An unavailable admission policy is explicit; unknown memory never becomes zero usage or
/// an implicit permission to allocate against shared-system capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAdmissionError {
    /// No successful LOCAL segment query is available.
    #[error("GPU admission is unqualified because the DXGI LOCAL budget is unavailable")]
    MissingLocalBudget,
    /// LOCAL was queried successfully but provided no nonzero budget.
    #[error("GPU admission is unqualified because the DXGI LOCAL budget is zero")]
    ZeroLocalBudget,
}

/// Exact-adapter or clock query failure. Partial segment-query failures instead remain
/// nullable fields in a successful `AdapterMemorySnapshot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemoryQueryError {
    /// The supplied LUID is absent from the current DXGI inventory.
    #[error("selected DXGI adapter was not found")]
    AdapterNotFound,
    /// A software adapter cannot qualify the native hardware-sharing route.
    #[error("selected DXGI adapter is software")]
    SoftwareAdapter,
    /// DXGI or performance-counter API failed before a complete observation was available.
    #[error("adapter memory observation failed with HRESULT {hresult}")]
    Native {
        /// Actual failing API result, without retaining a COM interface or native payload.
        hresult: i32,
    },
    /// QPC supplied a nonpositive coordinate or frequency.
    #[error("adapter memory observation returned an invalid QPC coordinate or frequency")]
    InvalidClock,
}

/// Queries node-zero LOCAL/NON_LOCAL usage on the exact hardware LUID, without creating a
/// D3D device/context or changing memory reservations. Call on a reporting/preparation
/// worker outside engine ticks and UI callbacks; native API execution stays on the caller.
pub fn query_adapter_memory(adapter: AdapterId) -> Result<AdapterMemorySnapshot, MemoryQueryError> {
    // SAFETY: windows-rs owns the factory and every enumerated adapter interface.
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }.map_err(native_error)?;
    let selected = select(&factory, adapter)?;
    // SAFETY: selected owns its copied descriptor and remains live through both queries.
    let descriptor = unsafe { selected.GetDesc1() }.map_err(native_error)?;
    if descriptor.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
        return Err(MemoryQueryError::SoftwareAdapter);
    }
    let (local, local_hresult, nonlocal, nonlocal_hresult) = match selected.cast::<IDXGIAdapter3>() {
        Ok(memory) => {
            let (local, local_hresult) = segment(&memory, DXGI_MEMORY_SEGMENT_GROUP_LOCAL);
            let (nonlocal, nonlocal_hresult) = segment(&memory, DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL);
            (local, local_hresult, nonlocal, nonlocal_hresult)
        }
        Err(error) => (None, Some(error.code().0), None, Some(error.code().0)),
    };
    let mut coordinate = 0_i64;
    let mut frequency = 0_i64;
    // SAFETY: performance counters write initialized stack values and create no native owner.
    unsafe {
        QueryPerformanceFrequency(&mut frequency).map_err(native_error)?;
        QueryPerformanceCounter(&mut coordinate).map_err(native_error)?;
    }
    if coordinate <= 0 || frequency <= 0 {
        return Err(MemoryQueryError::InvalidClock);
    }
    Ok(AdapterMemorySnapshot {
        adapter,
        process_id: std::process::id(),
        observed_qpc: coordinate,
        qpc_frequency: frequency,
        dedicated_video_bytes: descriptor.DedicatedVideoMemory as u64,
        shared_system_capacity_bytes: descriptor.SharedSystemMemory as u64,
        local,
        local_hresult,
        nonlocal,
        nonlocal_hresult,
    })
}

fn select(factory: &IDXGIFactory1, id: AdapterId) -> Result<IDXGIAdapter1, MemoryQueryError> {
    for index in 0.. {
        // SAFETY: enumeration uses a retained factory and owned interface results.
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => return Err(MemoryQueryError::AdapterNotFound),
            Err(error) => return Err(native_error(error)),
        };
        // SAFETY: descriptor data is copied while the owned adapter interface is retained.
        let descriptor = unsafe { adapter.GetDesc1() }.map_err(native_error)?;
        if descriptor.AdapterLuid.LowPart == id.low && descriptor.AdapterLuid.HighPart == id.high {
            return Ok(adapter);
        }
    }
    Err(MemoryQueryError::AdapterNotFound)
}

fn segment(
    adapter: &IDXGIAdapter3,
    segment: DXGI_MEMORY_SEGMENT_GROUP,
) -> (Option<MemorySegmentSnapshot>, Option<i32>) {
    let mut result = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
    // SAFETY: node zero of this retained adapter writes a correctly sized stack structure.
    match unsafe { adapter.QueryVideoMemoryInfo(0, segment, &mut result) } {
        Ok(()) => (
            Some(MemorySegmentSnapshot {
                current_usage_bytes: result.CurrentUsage,
                budget_bytes: result.Budget,
                available_for_reservation_bytes: result.AvailableForReservation,
                current_reservation_bytes: result.CurrentReservation,
            }),
            None,
        ),
        Err(error) => (None, Some(error.code().0)),
    }
}

fn native_error(error: windows::core::Error) -> MemoryQueryError {
    MemoryQueryError::Native {
        hresult: error.code().0,
    }
}

#[cfg(test)]
mod tests;
