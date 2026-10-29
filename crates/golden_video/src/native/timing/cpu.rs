use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::{GetCurrentThread, GetThreadTimes};

use super::{CpuInterval, Recorder, ThreadCpuDelta};

#[derive(Clone, Copy)]
struct ThreadStamp {
    kernel: u64,
    user: u64,
}

fn thread_stamp() -> Option<ThreadStamp> {
    let (mut created, mut ended, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: the pseudo handle refers to this calling worker and all outputs are stack data.
    unsafe { GetThreadTimes(GetCurrentThread(), &mut created, &mut ended, &mut kernel, &mut user) }.ok()?;
    let ticks = |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    Some(ThreadStamp {
        kernel: ticks(kernel),
        user: ticks(user),
    })
}

pub(crate) struct CpuMeasurement {
    pub(crate) wall: Option<CpuInterval>,
    pub(crate) thread_cpu: Option<ThreadCpuDelta>,
}

/// Inclusive OS-accounted CPU and wall observations; no API-subtracted CPU inference.
pub(crate) struct CpuScope {
    recorder: Recorder,
    begin: Option<u64>,
    thread: Option<ThreadStamp>,
}

impl CpuScope {
    pub(crate) fn new(recorder: &Recorder) -> Self {
        Self {
            recorder: recorder.clone(),
            begin: recorder.stamp(),
            thread: thread_stamp(),
        }
    }
    pub(crate) fn finish(self) -> CpuMeasurement {
        let wall = self.recorder.interval(self.begin);
        let thread_cpu = self.thread.zip(thread_stamp()).and_then(|(begin, end)| {
            Some(ThreadCpuDelta {
                kernel_100ns: end.kernel.checked_sub(begin.kernel)?,
                user_100ns: end.user.checked_sub(begin.user)?,
            })
        });
        CpuMeasurement { wall, thread_cpu }
    }
}
