use super::{ProcessSnapshot, ProcessSpec, ProcessWindow, RootProcessState};
use ::windows::{
    Win32::{
        Foundation::{HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WAIT_TIMEOUT, WPARAM},
        System::{JobObjects::*, Threading::*},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowThreadProcessId, PostMessageW, SW_HIDE, SW_SHOWNORMAL, WM_CLOSE,
        },
    },
    core::{BOOL, PCWSTR, PWSTR},
};
use std::{
    collections::HashMap,
    io,
    mem::size_of,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::{Arc, Mutex, MutexGuard, mpsc},
    thread::JoinHandle,
};

mod launch;
mod observer;

#[cfg(test)]
mod tests;
fn raw(handle: &OwnedHandle) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}
fn os_error(error: ::windows::core::Error) -> io::Error {
    io::Error::from_raw_os_error(error.code().0 & 0xffff)
}
const MAX_OBSERVED_PROCESSES: usize = 4096;
pub(super) fn validate(spec: &ProcessSpec) -> io::Result<()> {
    launch::Prepared::new(spec).map(|_| ())
}

pub(super) struct Process {
    // The observer retains the job through its last event. Drop explicitly terminates it first.
    job: Arc<OwnedHandle>,
    root: OwnedHandle,
    thread: Option<OwnedHandle>,
    id: u32,
    observed: Arc<Mutex<Observed>>,
    observer: Option<JoinHandle<()>>,
}
#[derive(Default)]
struct Observed {
    handles: HashMap<u32, OwnedHandle>,
    completed: u64,
    error: Option<String>,
    success: bool,
}
impl Process {
    pub fn create(spec: &ProcessSpec) -> io::Result<Self> {
        validate(spec)?;
        let spec = spec.clone();
        let (send, receive) = mpsc::sync_channel(1);
        // Windows requires creation, WaitForDebugEvent and ContinueDebugEvent on the same thread.
        let observer = std::thread::Builder::new()
            .name("owned-process-events".into())
            .spawn(move || match Self::create_native(&spec) {
                Err(error) => {
                    let _ = send.send(Err(error));
                }
                Ok(process) => {
                    let job = process.job.clone();
                    let observed = process.observed.clone();
                    let id = process.id;
                    if send.send(Ok(process)).is_err() {
                        return;
                    }
                    let result = observer::run(&job, &observed, id);
                    if let Ok(mut state) = observed.lock() {
                        state.success = result.is_ok();
                        state.error = result.as_ref().err().map(ToString::to_string);
                    }
                    if result.is_err() {
                        // Fail closed, including descendants that broke the debugging chain.
                        let _ = unsafe { TerminateJobObject(raw(&job), 1) };
                    }
                }
            })?;
        let mut process = receive
            .recv()
            .map_err(|_| io::Error::other("process observer stopped during creation"))??;
        process.observer = Some(observer);
        Ok(process)
    }
    fn create_native(spec: &ProcessSpec) -> io::Result<Self> {
        let mut prepared = launch::Prepared::new(spec)?;
        // SAFETY: anonymous non-inheritable job; OwnedHandle assumes the successful returned handle.
        let job = unsafe { OwnedHandle::from_raw_handle(CreateJobObjectW(None, PCWSTR::null()).map_err(os_error)?.0) };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                raw(&job),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(os_error)?;
        }
        // Assign at creation, while suspended, eliminating an unowned CreateProcess/Assign gap.
        let attributes = launch::Attributes::new(raw(&job))?;
        let mut startup = STARTUPINFOEXW::default();
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESHOWWINDOW;
        startup.StartupInfo.wShowWindow = match spec.window {
            ProcessWindow::Hidden => SW_HIDE.0 as u16,
            ProcessWindow::Normal => SW_SHOWNORMAL.0 as u16,
        };
        startup.lpAttributeList = attributes.list();
        let mut information = PROCESS_INFORMATION::default();
        let mut flags = CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT | DEBUG_PROCESS;
        if spec.window == ProcessWindow::Hidden {
            flags |= CREATE_NO_WINDOW;
        }
        // SAFETY: mutable command buffer, UTF-16 buffers and attribute values remain alive for the call.
        // No handles (including the private job) are inheritable. No shell or elevation is requested.
        unsafe {
            CreateProcessW(
                PCWSTR(prepared.executable.as_ptr()),
                Some(PWSTR(prepared.command.as_mut_ptr())),
                None,
                None,
                false,
                flags,
                prepared.environment.as_ref().map(|value| value.as_ptr().cast()),
                prepared
                    .directory
                    .as_ref()
                    .map_or(PCWSTR::null(), |value| PCWSTR(value.as_ptr())),
                &startup.StartupInfo,
                &mut information,
            )
            .map_err(os_error)?;
        }
        let process = Self {
            job: Arc::new(job),
            root: unsafe { OwnedHandle::from_raw_handle(information.hProcess.0) },
            thread: Some(unsafe { OwnedHandle::from_raw_handle(information.hThread.0) }),
            id: information.dwProcessId,
            observed: Arc::new(Mutex::new(Observed::default())),
            observer: None,
        };
        let mut assigned = BOOL::default();
        let membership = unsafe { IsProcessInJob(raw(&process.root), Some(raw(&process.job)), &mut assigned) };
        if membership.is_err() || !assigned.as_bool() {
            // Primary thread is still suspended; reject without ever allowing unowned execution.
            let _ = unsafe { TerminateProcess(raw(&process.root), 1) };
            return Err(membership
                .err()
                .map(os_error)
                .unwrap_or_else(|| io::Error::other("created process did not enter the owned job")));
        }
        Ok(process)
    }
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn resume(&mut self) -> io::Result<()> {
        drop(self.observation()?);
        let thread = self
            .thread
            .take()
            .ok_or_else(|| io::Error::other("primary thread was already resumed"))?;
        let previous = unsafe { ResumeThread(raw(&thread)) };
        if previous == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        if previous != 1 {
            return Err(io::Error::other("primary suspend count changed before resume"));
        }
        Ok(())
    }
    pub fn snapshot(&self) -> io::Result<ProcessSnapshot> {
        // Serialize accounting with birth registration, before the observer permits execution.
        let mut observed = self.observation()?;
        observed.reap()?;
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        unsafe {
            QueryInformationJobObject(
                Some(raw(&self.job)),
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                None,
            )
            .map_err(os_error)?;
        }
        let root = match unsafe { WaitForSingleObject(raw(&self.root), 0) } {
            WAIT_TIMEOUT => RootProcessState::Running,
            WAIT_OBJECT_0 => {
                let mut code = 0;
                unsafe {
                    GetExitCodeProcess(raw(&self.root), &mut code).map_err(os_error)?;
                }
                RootProcessState::Exited(code)
            }
            _ => return Err(io::Error::last_os_error()),
        };
        let known = 1 + observed.completed + observed.handles.len() as u64;
        let total = u64::from(accounting.TotalProcesses);
        if known > total {
            return Err(io::Error::other(
                "job process accounting diverged from observed lifetimes",
            ));
        }
        Ok(ProcessSnapshot {
            root_id: self.id,
            root,
            active_processes: accounting.ActiveProcesses,
            unobserved_processes: (total - known) as u32,
            pending_observed_processes: observed.handles.len() as u32,
            observation_complete: observed.success && self.observer.as_ref().is_some_and(JoinHandle::is_finished),
        })
    }
    pub fn terminate(&self, exit_code: u32) -> io::Result<()> {
        unsafe { TerminateJobObject(raw(&self.job), exit_code).map_err(os_error) }
    }
    pub fn request_close(&self) -> io::Result<u32> {
        // Keep identity-pinning handles borrowed through enumeration. A stale PID alone must
        // never authorize a close request to an unrelated replacement process.
        let observed = self.observation()?;
        let ids = std::iter::once(self.id as usize)
            .chain(observed.handles.keys().map(|id| *id as usize))
            .collect::<Vec<_>>();
        let mut close = CloseWindows {
            ids: &ids,
            sent: 0,
            error: None,
        };
        unsafe {
            EnumWindows(
                Some(close_window),
                LPARAM((&mut close as *mut CloseWindows<'_>) as isize),
            )
            .map_err(os_error)?;
        }
        if let Some(error) = close.error {
            return Err(error);
        }
        Ok(close.sent)
    }
    fn observation(&self) -> io::Result<MutexGuard<'_, Observed>> {
        let state = self
            .observed
            .lock()
            .map_err(|_| io::Error::other("process observer state poisoned"))?;
        if let Some(error) = &state.error {
            return Err(io::Error::other(format!("process observer failed: {error}")));
        }
        Ok(state)
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        // Never join on the caller. Killing the job wakes the event thread, including before resume.
        let _ = unsafe { TerminateJobObject(raw(&self.job), 1) };
    }
}
impl Observed {
    fn reap(&mut self) -> io::Result<()> {
        let mut error = None;
        self.handles
            .retain(|_, handle| match unsafe { WaitForSingleObject(raw(handle), 0) } {
                WAIT_OBJECT_0 => {
                    self.completed += 1;
                    false
                }
                WAIT_TIMEOUT => true,
                _ => {
                    error = Some(io::Error::last_os_error());
                    true
                }
            });
        error.map_or(Ok(()), Err)
    }
}
struct CloseWindows<'a> {
    ids: &'a [usize],
    sent: u32,
    error: Option<io::Error>,
}
unsafe extern "system" fn close_window(window: HWND, parameter: LPARAM) -> BOOL {
    // SAFETY: EnumWindows invokes synchronously while this context and its job ID list are alive.
    let close = unsafe { &mut *(parameter.0 as *mut CloseWindows<'_>) };
    let mut id = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut id));
    }
    if close.ids.contains(&(id as usize)) {
        match unsafe { PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0)) } {
            Ok(()) => close.sent += 1,
            Err(error) => close.error = Some(os_error(error)),
        }
    }
    BOOL(1)
}
