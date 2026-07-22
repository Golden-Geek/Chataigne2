use std::ffi::c_void;
use std::io::{Error, Result};
use std::mem::{size_of, zeroed};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::process::{Child, Command};

const CREATE_SUSPENDED: u32 = 0x0000_0004;
const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;
const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x0000_2000;

#[repr(C)]
struct JobObjectBasicLimitInformation {
    per_process_user_time_limit: i64,
    per_job_user_time_limit: i64,
    limit_flags: u32,
    minimum_working_set_size: usize,
    maximum_working_set_size: usize,
    active_process_limit: u32,
    affinity: usize,
    priority_class: u32,
    scheduling_class: u32,
}

#[repr(C)]
struct IoCounters {
    read_operation_count: u64,
    write_operation_count: u64,
    other_operation_count: u64,
    read_transfer_count: u64,
    write_transfer_count: u64,
    other_transfer_count: u64,
}

#[repr(C)]
struct JobObjectExtendedLimitInformation {
    basic_limit_information: JobObjectBasicLimitInformation,
    io_info: IoCounters,
    process_memory_limit: usize,
    job_memory_limit: usize,
    peak_process_memory_used: usize,
    peak_job_memory_used: usize,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "CreateJobObjectW"]
    fn create_job_object(job_attributes: *const c_void, name: *const u16) -> *mut c_void;
    #[link_name = "SetInformationJobObject"]
    fn set_information_job_object(
        job: *mut c_void,
        information_class: i32,
        information: *const c_void,
        information_length: u32,
    ) -> i32;
    #[link_name = "AssignProcessToJobObject"]
    fn assign_process_to_job_object(job: *mut c_void, process: *mut c_void) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
    #[link_name = "NtResumeProcess"]
    fn nt_resume_process(process: *mut c_void) -> i32;
}

pub(crate) struct WindowsProcessJob {
    handle: OwnedHandle,
}

impl WindowsProcessJob {
    pub(crate) fn spawn(command: &mut Command) -> Result<(Child, Self)> {
        let job = Self::new()?;
        command.creation_flags(CREATE_SUSPENDED);
        let mut child = command.spawn()?;

        if let Err(err) = job.assign(&child).and_then(|()| resume_process(&child)) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err);
        }

        Ok((child, job))
    }

    fn new() -> Result<Self> {
        // SAFETY: Null security and name pointers request an unnamed job with default security.
        let raw_handle = unsafe { create_job_object(std::ptr::null(), std::ptr::null()) };
        if raw_handle.is_null() {
            return Err(Error::last_os_error());
        }
        // SAFETY: CreateJobObjectW returned a new owned handle, checked above for null.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw_handle) };

        // SAFETY: The structure is a C layout of integer fields where all-zero is valid. The one
        // required limit flag is set immediately below.
        let mut information: JobObjectExtendedLimitInformation = unsafe { zeroed() };
        information.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: The handle and information pointer are live for the call, and the byte length
        // exactly matches the repr(C) structure required by the selected information class.
        let configured = unsafe {
            set_information_job_object(
                handle.as_raw_handle(),
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                std::ptr::addr_of!(information).cast(),
                size_of::<JobObjectExtendedLimitInformation>() as u32,
            )
        };
        if configured == 0 {
            return Err(Error::last_os_error());
        }

        Ok(Self { handle })
    }

    fn assign(&self, child: &Child) -> Result<()> {
        // SAFETY: Both handles are live. The child is suspended, so it cannot create descendants
        // before assignment to the kill-on-close job.
        let assigned = unsafe { assign_process_to_job_object(self.handle.as_raw_handle(), child.as_raw_handle()) };
        if assigned == 0 {
            return Err(Error::last_os_error());
        }
        Ok(())
    }
}

fn resume_process(child: &Child) -> Result<()> {
    // SAFETY: The child handle is live and was created suspended by this module.
    let status = unsafe { nt_resume_process(child.as_raw_handle()) };
    if status < 0 {
        return Err(Error::other(format!(
            "failed to resume frontend process after job assignment (NTSTATUS 0x{:08x})",
            status as u32
        )));
    }
    Ok(())
}
