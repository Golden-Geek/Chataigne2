//! Birth events fence user execution while an owned duplicate pins each process identity.
//! The OS owns event process/thread handles; only image/DLL file handles belong to us.
use super::{MAX_OBSERVED_PROCESSES, Observed, os_error, raw};
use std::{
    collections::HashMap,
    io,
    os::windows::io::{FromRawHandle, OwnedHandle},
    sync::Mutex,
};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED, DUPLICATE_SAME_ACCESS, DuplicateHandle,
            EXCEPTION_BREAKPOINT, HANDLE, NTSTATUS, STATUS_WX86_BREAKPOINT,
        },
        System::{
            Diagnostics::Debug::*,
            JobObjects::IsProcessInJob,
            Threading::{GetCurrentProcess, INFINITE, IsWow64Process},
        },
    },
    core::BOOL,
};

pub(super) fn run(job: &OwnedHandle, observed: &Mutex<Observed>, root: u32) -> io::Result<()> {
    // A process created with DEBUG_PROCESS inherits this debugging chain unless a descendant
    // explicitly starts another one. Independent job accounting detects any resulting gap.
    let mut processes = HashMap::<u32, InitialBreakpoint>::new();
    let mut root_seen = false;
    loop {
        let mut event = DEBUG_EVENT::default();
        unsafe {
            WaitForDebugEventEx(&mut event, INFINITE).map_err(os_error)?;
        }
        let mut status = DBG_CONTINUE;
        match event.dwDebugEventCode {
            CREATE_PROCESS_DEBUG_EVENT => {
                let info = unsafe { event.u.CreateProcessInfo };
                close_file(info.hFile)?;
                let mut member = BOOL::default();
                unsafe {
                    IsProcessInJob(info.hProcess, Some(raw(job)), &mut member).map_err(os_error)?;
                }
                if !member.as_bool() {
                    return Err(io::Error::other("debug creation event is outside the owned job"));
                }
                if processes.len() >= MAX_OBSERVED_PROCESSES || processes.contains_key(&event.dwProcessId) {
                    return Err(io::Error::other(
                        "process observer identity capacity or duplicate birth",
                    ));
                }
                if event.dwProcessId == root {
                    if root_seen {
                        return Err(io::Error::other("duplicate root creation event"));
                    }
                    root_seen = true;
                } else {
                    let handle = duplicate(info.hProcess)?;
                    let mut state = observed
                        .lock()
                        .map_err(|_| io::Error::other("process observer state poisoned"))?;
                    state.reap()?;
                    if state.handles.len() >= MAX_OBSERVED_PROCESSES {
                        return Err(io::Error::other("process observer handle capacity exceeded"));
                    }
                    if state.handles.insert(event.dwProcessId, handle).is_some() {
                        return Err(io::Error::other(
                            "process identity reused while its handle remains pinned",
                        ));
                    }
                }
                let mut wow64 = BOOL::default();
                unsafe {
                    IsWow64Process(info.hProcess, &mut wow64).map_err(os_error)?;
                }
                processes.insert(
                    event.dwProcessId,
                    InitialBreakpoint::new(event.dwThreadId, wow64.as_bool()),
                );
            }
            EXIT_PROCESS_DEBUG_EVENT => {
                if processes.remove(&event.dwProcessId).is_none() {
                    return Err(io::Error::other("process exit without an observed creation"));
                }
                // Continue first; an exit event is not a kernel-completion barrier. Snapshot
                // retains each duplicate until WaitForSingleObject actually signals.
            }
            LOAD_DLL_DEBUG_EVENT => close_file(unsafe { event.u.LoadDll.hFile })?,
            EXCEPTION_DEBUG_EVENT => {
                let info = unsafe { event.u.Exception };
                let initial = processes
                    .get_mut(&event.dwProcessId)
                    .ok_or_else(|| io::Error::other("exception without observed process creation"))?;
                if !initial.consume(
                    event.dwThreadId,
                    info.dwFirstChance != 0,
                    info.ExceptionRecord.ExceptionCode,
                ) {
                    status = DBG_EXCEPTION_NOT_HANDLED;
                }
            }
            RIP_EVENT => return Err(io::Error::other("Windows reported a process debugging failure")),
            _ => {}
        }
        // No application callback, readiness work, logging or synchronous transport on this path.
        unsafe {
            ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status).map_err(os_error)?;
        }
        if root_seen && processes.is_empty() {
            return Ok(());
        }
    }
}

fn duplicate(handle: HANDLE) -> io::Result<OwnedHandle> {
    let mut copy = HANDLE::default();
    unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            handle,
            GetCurrentProcess(),
            &mut copy,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
        .map_err(os_error)?;
        Ok(OwnedHandle::from_raw_handle(copy.0))
    }
}
fn close_file(handle: HANDLE) -> io::Result<()> {
    if !handle.is_invalid() {
        unsafe {
            CloseHandle(handle).map_err(os_error)?;
        }
    }
    Ok(())
}

struct InitialBreakpoint {
    thread: u32,
    native: bool,
    wow64: bool,
}
impl InitialBreakpoint {
    fn new(thread: u32, wow64: bool) -> Self {
        Self {
            thread,
            native: true,
            wow64,
        }
    }
    fn consume(&mut self, thread: u32, first_chance: bool, code: NTSTATUS) -> bool {
        if thread != self.thread || !first_chance {
            return false;
        }
        // WOW64 has a separate initial x86 loader breakpoint. Each loader handshake may
        // be consumed once; later application exceptions still reach Windows handlers.
        let pending = match code {
            EXCEPTION_BREAKPOINT => &mut self.native,
            STATUS_WX86_BREAKPOINT => &mut self.wow64,
            _ => return false,
        };
        std::mem::replace(pending, false)
    }
}

#[cfg(test)]
mod tests;
