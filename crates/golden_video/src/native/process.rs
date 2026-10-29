//! Sparse process observations for owned qualification processes; no GPU or window access.

use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use serde::Serialize;
use thiserror::Error;
use windows::Win32::Foundation::{FILETIME, HANDLE};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::ProcessStatus::{
    GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessHandleCount, GetProcessTimes, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};

/// Current process memory counters. Working set and private commit measure different things
/// and must not be added together as one memory allocation total.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProcessMemorySnapshot {
    /// Current process private committed bytes, from `PrivateUsage`.
    pub private_bytes: u64,
    /// Currently resident working-set bytes, including potentially shared pages.
    pub working_set_bytes: u64,
    /// Peak working set across the entire process lifetime, including warmup/setup.
    pub lifetime_peak_working_set_bytes: u64,
}

/// Cumulative OS process execution accounting, including all threads. Differences measure
/// aggregate process CPU time; these counters are not per-frame submission-work timings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ProcessTimeSnapshot {
    /// Process creation FILETIME, in 100 ns units since the Windows FILETIME epoch.
    pub creation_filetime_100ns: u64,
    /// Nonzero process exit FILETIME when reported by the OS; absent otherwise.
    pub exit_filetime_100ns: Option<u64>,
    /// Cumulative kernel execution time, in 100 ns units.
    pub kernel_time_100ns: u64,
    /// Cumulative user execution time, in 100 ns units.
    pub user_time_100ns: u64,
}

/// Nullable, process-specific OS observations. A failed counter stays absent with its actual
/// HRESULT instead of becoming a success-shaped zero. Fixture/WebView observations must
/// remain separate from the renderer's process totals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProcessResourceSnapshot {
    /// Exact process ID requested by the caller.
    pub process_id: u32,
    /// QPC observed after querying the counters below.
    pub observed_qpc: i64,
    /// QPC ticks per second, validated positive.
    pub qpc_frequency: i64,
    /// Private commit and working-set observations, when available.
    pub memory: Option<ProcessMemorySnapshot>,
    /// Memory-query failure when `memory` is absent.
    pub memory_hresult: Option<i32>,
    /// Current OS handle count, when available.
    pub handle_count: Option<u32>,
    /// Handle-count query failure when `handle_count` is absent.
    pub handle_count_hresult: Option<i32>,
    /// Process identity and cumulative CPU accounting, when available.
    pub times: Option<ProcessTimeSnapshot>,
    /// Process-time query failure when `times` is absent.
    pub times_hresult: Option<i32>,
}

/// Failure to open a process or establish the observation clock. Individual counter failures
/// instead remain nullable fields in a successful `ProcessResourceSnapshot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProcessQueryError {
    /// PID zero is not an owned renderer/fixture process.
    #[error("process resource observation requires a nonzero process ID")]
    InvalidProcessId,
    /// Process access or performance-counter API failed.
    #[error("process resource observation failed with HRESULT {hresult}")]
    Native {
        /// Actual failing API result, without retaining a process handle.
        hresult: i32,
    },
    /// QPC supplied a nonpositive coordinate or frequency.
    #[error("process resource observation returned an invalid QPC coordinate or frequency")]
    InvalidClock,
}

/// Copies OS counters for the caller's current process or an explicitly owned fixture PID.
/// This creates no GPU device and inspects no windows. Native handles are opened only for
/// the duration of this call; compare creation FILETIMEs across samples to detect PID reuse.
/// Run sparse observations on a reporting worker outside engine ticks and UI callbacks.
pub fn query_process_resources(process_id: u32) -> Result<ProcessResourceSnapshot, ProcessQueryError> {
    if process_id == 0 {
        return Err(ProcessQueryError::InvalidProcessId);
    }
    let owned = if process_id == std::process::id() {
        None
    } else {
        // SAFETY: read-only access to the explicit PID; the owned handle closes on every exit.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, process_id) }
            .map_err(native_error)?;
        // SAFETY: successful OpenProcess transfers one valid, uniquely owned handle.
        Some(unsafe { OwnedHandle::from_raw_handle(handle.0) })
    };
    let handle = match &owned {
        Some(owned) => HANDLE(owned.as_raw_handle()),
        // SAFETY: borrow the current-process pseudo handle, which must never be closed.
        None => unsafe { GetCurrentProcess() },
    };
    let (memory, memory_hresult) = memory(handle);
    let (handle_count, handle_count_hresult) = handles(handle);
    let (times, times_hresult) = times(handle);
    let mut coordinate = 0_i64;
    let mut frequency = 0_i64;
    // SAFETY: performance-counter output pointers refer to initialized stack values.
    unsafe {
        QueryPerformanceFrequency(&mut frequency).map_err(native_error)?;
        QueryPerformanceCounter(&mut coordinate).map_err(native_error)?;
    }
    if coordinate <= 0 || frequency <= 0 {
        return Err(ProcessQueryError::InvalidClock);
    }
    Ok(ProcessResourceSnapshot {
        process_id,
        observed_qpc: coordinate,
        qpc_frequency: frequency,
        memory,
        memory_hresult,
        handle_count,
        handle_count_hresult,
        times,
        times_hresult,
    })
}

fn memory(handle: HANDLE) -> (Option<ProcessMemorySnapshot>, Option<i32>) {
    let mut counters = PROCESS_MEMORY_COUNTERS_EX::default();
    counters.cb = std::mem::size_of_val(&counters) as u32;
    // SAFETY: the retained process handle and full EX structure match the supplied byte size.
    let result = unsafe {
        GetProcessMemoryInfo(
            handle,
            (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast::<PROCESS_MEMORY_COUNTERS>(),
            counters.cb,
        )
    };
    match result {
        Ok(()) => (
            Some(ProcessMemorySnapshot {
                private_bytes: counters.PrivateUsage as u64,
                working_set_bytes: counters.WorkingSetSize as u64,
                lifetime_peak_working_set_bytes: counters.PeakWorkingSetSize as u64,
            }),
            None,
        ),
        Err(error) => (None, Some(error.code().0)),
    }
}

fn handles(handle: HANDLE) -> (Option<u32>, Option<i32>) {
    let mut count = 0;
    // SAFETY: the retained process handle writes a correctly typed stack output.
    match unsafe { GetProcessHandleCount(handle, &mut count) } {
        Ok(()) => (Some(count), None),
        Err(error) => (None, Some(error.code().0)),
    }
}

fn times(handle: HANDLE) -> (Option<ProcessTimeSnapshot>, Option<i32>) {
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: all four FILETIME outputs are initialized and live through this read-only call.
    match unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) } {
        Ok(()) => (
            Some(ProcessTimeSnapshot {
                creation_filetime_100ns: filetime(creation),
                exit_filetime_100ns: (filetime(exit) != 0).then_some(filetime(exit)),
                kernel_time_100ns: filetime(kernel),
                user_time_100ns: filetime(user),
            }),
            None,
        ),
        Err(error) => (None, Some(error.code().0)),
    }
}

fn filetime(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}

fn native_error(error: windows::core::Error) -> ProcessQueryError {
    ProcessQueryError::Native {
        hresult: error.code().0,
    }
}

#[cfg(test)]
mod tests;
