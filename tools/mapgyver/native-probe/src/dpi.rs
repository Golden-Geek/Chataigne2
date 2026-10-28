//! Physical-coordinate observations share one scoped thread DPI authority.

use crate::ProbeResult;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};

pub(crate) struct PhysicalDpiScope(DPI_AWARENESS_CONTEXT);

impl PhysicalDpiScope {
    pub(crate) fn enter() -> ProbeResult<Self> {
        // SAFETY: changes only this calling thread; the original context is restored on drop.
        let previous = unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous.0.is_null() {
            return Err(windows::core::Error::from_win32().into());
        }
        Ok(Self(previous))
    }
}

impl Drop for PhysicalDpiScope {
    fn drop(&mut self) {
        // SAFETY: the matching entry call returned this context on this thread.
        unsafe { SetThreadDpiAwarenessContext(self.0) };
    }
}
