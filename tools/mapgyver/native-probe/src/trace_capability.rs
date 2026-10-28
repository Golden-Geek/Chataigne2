//! Verify session-creation rights directly without enabling providers, collecting events,
//! writing files, changing privileges, or touching a session we did not successfully create.

use crate::ProbeResult;
use serde::Serialize;
use windows::Win32::{Foundation::ERROR_SUCCESS, System::Diagnostics::Etw::*};
use windows::core::PCWSTR;

const NAME: &str = "MapGyver-M01-Presentation-Preflight";

#[repr(C)]
struct Properties {
    header: EVENT_TRACE_PROPERTIES,
    name: [u16; 128],
}

impl Properties {
    fn new() -> Self {
        let mut result = Self {
            header: EVENT_TRACE_PROPERTIES::default(),
            name: [0; 128],
        };
        result.header.Wnode.BufferSize = std::mem::size_of::<Self>() as u32;
        result.header.Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        result.header.Wnode.ClientContext = 1;
        result.header.BufferSize = 16;
        result.header.MinimumBuffers = 2;
        result.header.MaximumBuffers = 2;
        result.header.LogFileMode = EVENT_TRACE_REAL_TIME_MODE | EVENT_TRACE_NO_PER_PROCESSOR_BUFFERING;
        result.header.LoggerNameOffset = std::mem::offset_of!(Self, name) as u32;
        result
    }
}

struct Session {
    handle: CONTROLTRACE_HANDLE,
    properties: Properties,
    stopped: bool,
}

impl Session {
    fn stop(&mut self) -> u32 {
        // SAFETY: handle came from our successful StartTrace; no name-based stop of an
        // existing session. The complete properties buffer remains owned during the call.
        let status = unsafe {
            ControlTraceW(
                self.handle,
                PCWSTR::null(),
                &mut self.properties.header,
                EVENT_TRACE_CONTROL_STOP,
            )
        };
        self.stopped = status == ERROR_SUCCESS;
        status.0
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.stopped {
            self.stop();
        }
    }
}

#[derive(Serialize)]
pub struct Report {
    session_name: &'static str,
    start_status: u32,
    start_message: String,
    stop_status: Option<u32>,
    providers_enabled: u32,
    trace_file_written: bool,
    display_capture_tested: bool,
}

pub fn probe() -> ProbeResult<Report> {
    let mut properties = Properties::new();
    let mut handle = CONTROLTRACE_HANDLE::default();
    let name: Vec<_> = NAME.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: aligned properties plus room for the session name; no event providers are
    // enabled. A name collision returns evidence and never authorizes replacing a session.
    let status = unsafe { StartTraceW(&mut handle, PCWSTR(name.as_ptr()), &mut properties.header) };
    let stop_status = if status == ERROR_SUCCESS {
        let mut session = Session {
            handle,
            properties,
            stopped: false,
        };
        let stop = session.stop();
        if stop != 0 {
            return Err(format!("owned empty ETW session stop failed: {stop}").into());
        }
        Some(stop)
    } else {
        None
    };
    Ok(Report {
        session_name: NAME,
        start_status: status.0,
        start_message: windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(status.0)).to_string(),
        stop_status,
        providers_enabled: 0,
        trace_file_written: false,
        display_capture_tested: false,
    })
}
