//! One machine-wide monotonic authority for producer and independent-consumer stamps.

use crate::ProbeResult;
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

pub fn now() -> ProbeResult<u64> {
    let mut value = 0;
    // SAFETY: writable stack output; QPC is comparable across processes on this machine.
    unsafe { QueryPerformanceCounter(&mut value)? };
    Ok(value.try_into()?)
}

pub fn frequency() -> ProbeResult<u64> {
    let mut value = 0;
    // SAFETY: writable stack output; frequency is fixed at system startup.
    unsafe { QueryPerformanceFrequency(&mut value)? };
    let value = u64::try_from(value)?;
    if value == 0 {
        return Err("zero performance counter frequency".into());
    }
    Ok(value)
}
