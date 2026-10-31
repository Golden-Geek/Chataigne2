//! Worker-local deadline timing; no process/global timer-resolution change or polling.
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::sync::Arc;
use std::time::Instant;

use windows::Win32::Foundation::{HANDLE, WAIT_FAILED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateEventW, CreateWaitableTimerExW, INFINITE, ResetEvent, SetEvent,
    SetWaitableTimerEx, TIMER_ALL_ACCESS, WaitForMultipleObjects,
};

pub(super) struct Signal(OwnedHandle);

impl Signal {
    fn new() -> io::Result<Self> {
        // SAFETY: new unnamed manual-reset event, exclusively owned by this worker's signal.
        let handle = unsafe { CreateEventW(None, true, false, None) }.map_err(io::Error::other)?;
        Ok(Self(unsafe { OwnedHandle::from_raw_handle(handle.0) }))
    }

    pub(super) fn notify(&self) {
        // SAFETY: a temporary Arc keeps this event alive through the infallible wake callback.
        // Failure would mean an invalid handle despite that ownership invariant.
        let result = unsafe { SetEvent(self.handle()) };
        debug_assert!(result.is_ok(), "retained worker wake event must remain valid");
    }

    fn handle(&self) -> HANDLE {
        HANDLE(self.0.as_raw_handle())
    }
}

pub(super) struct DeadlineWaiter {
    signal: Arc<Signal>,
    timer: OwnedHandle,
}

impl DeadlineWaiter {
    pub(super) fn new() -> io::Result<Self> {
        let signal = Arc::new(Signal::new()?);
        // SAFETY: unnamed synchronization timer with one worker controlling its deadlines.
        let handle =
            unsafe { CreateWaitableTimerExW(None, None, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS.0) }
                .map_err(io::Error::other)?;
        Ok(Self {
            signal,
            timer: unsafe { OwnedHandle::from_raw_handle(handle.0) },
        })
    }

    pub(super) fn signal(&self) -> &Arc<Signal> {
        &self.signal
    }

    /// Caller holds the wake-generation mutex while resetting, so a later notification
    /// either changes that generation or leaves this event signaled before the kernel wait.
    pub(super) fn prepare(&self, deadline: Instant) -> io::Result<()> {
        let ticks = deadline
            .saturating_duration_since(Instant::now())
            .as_nanos()
            .div_ceil(100);
        let due = -(ticks.max(1).min(i64::MAX as u128) as i64);
        // SAFETY: both handles stay owned; the negative due time is relative in 100 ns units.
        unsafe {
            ResetEvent(self.signal.handle()).map_err(io::Error::other)?;
            SetWaitableTimerEx(HANDLE(self.timer.as_raw_handle()), &due, 0, None, None, None, 0)
                .map_err(io::Error::other)?;
        }
        Ok(())
    }

    pub(super) fn wait(&self) -> io::Result<()> {
        // SAFETY: this context's sole worker retains both distinct handles throughout waiting.
        let result = unsafe {
            WaitForMultipleObjects(
                &[self.signal.handle(), HANDLE(self.timer.as_raw_handle())],
                false,
                INFINITE,
            )
        };
        if result == WAIT_OBJECT_0 || result.0 == WAIT_OBJECT_0.0 + 1 {
            Ok(())
        } else if result == WAIT_FAILED {
            Err(io::Error::last_os_error())
        } else {
            Err(io::Error::other("unexpected precise worker wait result"))
        }
    }
}
