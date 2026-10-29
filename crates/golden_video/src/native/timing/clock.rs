use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

/// Machine-wide monotonic coordinates shared with independent qualification processes.
#[derive(Clone, Copy)]
pub(crate) struct Clock {
    pub(crate) frequency: Option<u64>,
}

impl Clock {
    pub(crate) fn new() -> Self {
        let mut value = 0;
        // SAFETY: writable stack output; QPC frequency is fixed at system startup.
        let frequency = unsafe { QueryPerformanceFrequency(&mut value) }
            .ok()
            .and_then(|()| u64::try_from(value).ok())
            .filter(|value| *value != 0);
        Self { frequency }
    }

    pub(crate) fn stamp(self) -> Option<u64> {
        let mut value = 0;
        // SAFETY: writable stack output, no native ownership leaves this call.
        unsafe { QueryPerformanceCounter(&mut value) }
            .ok()
            .and_then(|()| u64::try_from(value).ok())
    }
}
