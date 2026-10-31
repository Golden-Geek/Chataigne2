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
    cell::RefCell,
    collections::{HashMap, HashSet},
    io,
    mem::{offset_of, size_of},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
};

mod launch;

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
    // First drop closes the sole job handle, requesting termination even if the root already exited.
    job: OwnedHandle,
    root: OwnedHandle,
    thread: Option<OwnedHandle>,
    id: u32,
    observed: RefCell<Observed>,
}
#[derive(Default)]
struct Observed {
    handles: HashMap<u32, OwnedHandle>,
    completed: u64,
}
impl Process {
    pub fn create(spec: &ProcessSpec) -> io::Result<Self> {
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
        let mut flags = CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT;
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
            job,
            root: unsafe { OwnedHandle::from_raw_handle(information.hProcess.0) },
            thread: Some(unsafe { OwnedHandle::from_raw_handle(information.hThread.0) }),
            id: information.dwProcessId,
            observed: RefCell::new(Observed::default()),
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
        // Capture creation-time job members before user code can exit the root and its helpers.
        self.observe_descendants()?;
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
        self.observe_descendants()?;
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
        let observed = self.observed.borrow();
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
        })
    }
    pub fn terminate(&self, exit_code: u32) -> io::Result<()> {
        // Preserve actual handle completion evidence before termination can remove the job IDs.
        // A failed observation must not prevent the kill; the accounting gap then remains unverified.
        let _ = self.snapshot();
        unsafe { TerminateJobObject(raw(&self.job), exit_code).map_err(os_error) }
    }
    pub fn request_close(&self) -> io::Result<u32> {
        self.observe_descendants()?;
        // Keep identity-pinning handles borrowed through enumeration. A stale PID alone must
        // never authorize a close request to an unrelated replacement process.
        let observed = self.observed.borrow();
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
    fn observe_descendants(&self) -> io::Result<()> {
        let ids = self.process_ids()?;
        let mut observed = self.observed.borrow_mut();
        let mut completed = HashSet::new();
        observed.handles.retain(|id, handle| {
            if unsafe { WaitForSingleObject(raw(handle), 0) } == WAIT_OBJECT_0 {
                completed.insert(*id);
                false
            } else {
                true
            }
        });
        observed.completed += completed.len() as u64;
        for id in ids {
            let id = id as u32;
            if id == self.id
                || observed.handles.contains_key(&id)
                || completed.contains(&id)
                || observed.handles.len() >= MAX_OBSERVED_PROCESSES
            {
                continue;
            }
            // Open and verify membership while the handle pins identity; never adopt a reused PID.
            if let Ok(handle) = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_ACCESS_RIGHTS(0x0010_0000),
                    false,
                    id,
                )
            } {
                let handle = unsafe { OwnedHandle::from_raw_handle(handle.0) };
                let mut member = BOOL::default();
                if unsafe { IsProcessInJob(raw(&handle), Some(raw(&self.job)), &mut member) }.is_ok()
                    && member.as_bool()
                {
                    observed.handles.insert(id, handle);
                }
            }
        }
        Ok(())
    }
    fn process_ids(&self) -> io::Result<Vec<usize>> {
        let bytes =
            offset_of!(JOBOBJECT_BASIC_PROCESS_ID_LIST, ProcessIdList) + MAX_OBSERVED_PROCESSES * size_of::<usize>();
        let mut buffer = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        unsafe {
            QueryInformationJobObject(
                Some(raw(&self.job)),
                JobObjectBasicProcessIdList,
                buffer.as_mut_ptr().cast(),
                bytes as u32,
                None,
            )
            .map_err(os_error)?;
        }
        // SAFETY: usize-aligned allocation includes header and 4096 IDs; validate reported length first.
        let list = unsafe { &*buffer.as_ptr().cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>() };
        let count = list.NumberOfProcessIdsInList as usize;
        if count > MAX_OBSERVED_PROCESSES {
            return Err(io::Error::other("job close observation exceeds 4096 processes"));
        }
        let ids = unsafe {
            std::slice::from_raw_parts(
                buffer
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset_of!(JOBOBJECT_BASIC_PROCESS_ID_LIST, ProcessIdList))
                    .cast::<usize>(),
                count,
            )
        };
        Ok(ids.to_vec())
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
