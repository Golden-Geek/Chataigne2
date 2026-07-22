use std::hint;
use std::thread;
use std::time::{Duration, Instant};

const MIN_INTERVAL: Duration = Duration::from_nanos(1);
const MAX_CATCH_UP_PERIODS: u32 = 1;
const FINAL_PRECISION_GUARD: Duration = Duration::from_micros(100);
const SPIN_WINDOW: Duration = Duration::from_micros(20);

/// Timing information for one runtime-loop iteration.
pub(super) struct RuntimeTickStart {
    pub(super) started_at: Instant,
    pub(super) elapsed: Duration,
}

/// Paces a runtime thread against monotonic absolute deadlines.
///
/// Relative sleeps accumulate every late wake-up into the next period. Keeping an absolute
/// deadline lets a following short wait recover ordinary scheduler jitter. A long suspension is
/// re-anchored after at most one period of catch-up so the host never creates an unbounded burst.
pub(super) struct RuntimeLoopPacer {
    last_tick_started_at: Instant,
    schedule: DeadlineSchedule,
    waiter: DeadlineWaiter,
}

impl RuntimeLoopPacer {
    pub(super) fn new() -> Self {
        Self::new_at(Instant::now())
    }

    fn new_at(now: Instant) -> Self {
        Self {
            last_tick_started_at: now,
            schedule: DeadlineSchedule::default(),
            waiter: DeadlineWaiter::new(),
        }
    }

    pub(super) fn begin_tick(&mut self) -> RuntimeTickStart {
        let started_at = Instant::now();
        let elapsed = started_at.saturating_duration_since(self.last_tick_started_at);
        self.last_tick_started_at = started_at;
        RuntimeTickStart { started_at, elapsed }
    }

    pub(super) fn wait_for_next_tick(&mut self, tick_started_at: Instant, requested_interval: Duration) {
        let deadline =
            self.schedule
                .next_deadline(tick_started_at, Instant::now(), requested_interval.max(MIN_INTERVAL));
        self.waiter.wait_until(deadline);
    }
}

#[derive(Default)]
pub(super) struct DeadlineSchedule {
    interval: Option<Duration>,
    next_deadline: Option<Instant>,
}

impl DeadlineSchedule {
    /// Advances the absolute cadence, re-anchoring only after reconfiguration or excessive lag.
    pub(super) fn next_deadline(
        &mut self,
        tick_started_at: Instant,
        tick_finished_at: Instant,
        requested_interval: Duration,
    ) -> Instant {
        let interval = requested_interval.max(MIN_INTERVAL);
        let reconfigured = self.interval != Some(interval);
        let mut deadline = if reconfigured {
            tick_started_at.checked_add(interval).unwrap_or(tick_finished_at)
        } else {
            self.next_deadline
                .and_then(|deadline| deadline.checked_add(interval))
                .unwrap_or_else(|| tick_started_at.checked_add(interval).unwrap_or(tick_finished_at))
        };

        let max_catch_up = interval.saturating_mul(MAX_CATCH_UP_PERIODS);
        if tick_finished_at.saturating_duration_since(deadline) >= max_catch_up {
            // Dropping old deadlines avoids a replay storm after suspension while preserving
            // maximum throughput when the tick workload itself exceeds the requested interval.
            deadline = tick_finished_at;
        }

        self.interval = Some(interval);
        self.next_deadline = Some(deadline);
        deadline
    }
}

struct DeadlineWaiter {
    #[cfg(windows)]
    high_resolution_timer: Option<WindowsHighResolutionTimer>,
}

impl DeadlineWaiter {
    fn new() -> Self {
        Self {
            #[cfg(windows)]
            high_resolution_timer: WindowsHighResolutionTimer::new(),
        }
    }

    fn wait_until(&self, deadline: Instant) {
        loop {
            let now = Instant::now();
            let Some(remaining) = deadline.checked_duration_since(now) else {
                return;
            };

            if remaining > FINAL_PRECISION_GUARD {
                let coarse_wait = remaining - FINAL_PRECISION_GUARD;
                if !self.sleep_precisely(coarse_wait) {
                    thread::sleep(coarse_wait);
                }
                continue;
            }

            if remaining > SPIN_WINDOW {
                thread::yield_now();
            } else {
                hint::spin_loop();
            }
        }
    }

    fn sleep_precisely(&self, duration: Duration) -> bool {
        #[cfg(windows)]
        if let Some(timer) = self.high_resolution_timer.as_ref() {
            return timer.wait(duration);
        }

        #[cfg(not(windows))]
        let _ = duration;

        false
    }
}

#[cfg(windows)]
struct WindowsHighResolutionTimer {
    handle: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
impl WindowsHighResolutionTimer {
    fn new() -> Option<Self> {
        use std::ptr;
        use windows_sys::Win32::System::Threading::{
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateWaitableTimerExW, TIMER_ALL_ACCESS,
        };

        // SAFETY: null security attributes and name are explicitly supported. The returned handle
        // is owned by this value and closed in Drop.
        let handle = unsafe {
            CreateWaitableTimerExW(
                ptr::null(),
                ptr::null(),
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS,
            )
        };
        (!handle.is_null()).then_some(Self { handle })
    }

    fn wait(&self, duration: Duration) -> bool {
        use std::ptr;
        use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
        use windows_sys::Win32::System::Threading::{INFINITE, SetWaitableTimerEx, WaitForSingleObject};

        let ticks_100ns = duration.as_nanos().div_ceil(100).clamp(1, i64::MAX as u128) as i64;
        let relative_due_time = -ticks_100ns;

        // SAFETY: the handle remains valid for this call, all optional callback/context pointers
        // are null, and `relative_due_time` points to a live i64 for the duration of the call.
        let armed =
            unsafe { SetWaitableTimerEx(self.handle, &relative_due_time, 0, None, ptr::null(), ptr::null(), 0) };
        if armed == 0 {
            return false;
        }

        // SAFETY: this thread exclusively waits on the owned timer handle. An armed one-shot timer
        // must become signaled; failure is reported to the caller so it can use the std fallback.
        unsafe { WaitForSingleObject(self.handle, INFINITE) == WAIT_OBJECT_0 }
    }
}

#[cfg(windows)]
impl Drop for WindowsHighResolutionTimer {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;

        // SAFETY: the handle was returned by CreateWaitableTimerExW and is closed exactly once.
        let _ = unsafe { CloseHandle(self.handle) };
    }
}
