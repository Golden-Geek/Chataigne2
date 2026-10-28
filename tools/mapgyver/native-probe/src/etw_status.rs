//! Read-only statistics for the qualification runner's explicitly named PresentMon session.

use crate::{ProbeResult, clock};
use serde::Serialize;
use windows::Win32::{Foundation::ERROR_SUCCESS, System::Diagnostics::Etw::*};
use windows::core::PCWSTR;

const PREFIX: &str = "MapGyver-M01-Present-";
const NAME_CAPACITY: usize = 1025;

#[repr(C)]
struct Properties {
    header: EVENT_TRACE_PROPERTIES,
    logger_name: [u16; NAME_CAPACITY],
    log_file_name: [u16; NAME_CAPACITY],
}

impl Properties {
    fn new() -> Self {
        let mut result = Self {
            header: EVENT_TRACE_PROPERTIES::default(),
            logger_name: [0; NAME_CAPACITY],
            log_file_name: [0; NAME_CAPACITY],
        };
        result.header.Wnode.BufferSize = std::mem::size_of::<Self>() as u32;
        result.header.Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        result.header.LoggerNameOffset = std::mem::offset_of!(Self, logger_name) as u32;
        result.header.LogFileNameOffset = std::mem::offset_of!(Self, log_file_name) as u32;
        result
    }
}

#[derive(Serialize)]
pub struct Statistics {
    pub returned_session_name: String,
    pub session_guid: String,
    pub events_lost: u32,
    pub log_buffers_lost: u32,
    pub realtime_buffers_lost: u32,
    pub buffer_size_kib: u32,
    pub minimum_buffers: u32,
    pub maximum_buffers: u32,
    pub number_of_buffers: u32,
    pub free_buffers: u32,
    pub buffers_written: u32,
    pub log_file_mode: u32,
}

#[derive(Serialize)]
pub struct Report {
    pub session_name: String,
    pub before_qpc: u64,
    pub after_qpc: u64,
    pub status: u32,
    pub status_message: String,
    pub response_validation_error: Option<String>,
    pub statistics: Option<Statistics>,
    pub scope: &'static str,
}

fn validate_name(name: &str) -> ProbeResult<()> {
    let suffix = name
        .strip_prefix(PREFIX)
        .ok_or("ETW status requires an owned PresentMon session name")?;
    if suffix.len() != 32
        || !suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("ETW status requires exactly 32 lowercase hex digits after the owned prefix".into());
    }
    Ok(())
}

fn statistics(name: &str, status: u32, properties: &Properties) -> Result<Option<Statistics>, String> {
    if status != ERROR_SUCCESS.0 {
        // Unwritten/default output fields are never evidence of zero loss.
        return Ok(None);
    }
    let end = properties
        .logger_name
        .iter()
        .position(|unit| *unit == 0)
        .ok_or("queried ETW session name is not terminated")?;
    let returned_session_name =
        String::from_utf16(&properties.logger_name[..end]).map_err(|_| "queried ETW session name is invalid UTF-16")?;
    if !returned_session_name.eq_ignore_ascii_case(name) {
        return Err("queried ETW session name differs from the requested session".into());
    }
    let header = &properties.header;
    Ok(Some(Statistics {
        returned_session_name,
        session_guid: format!("{:?}", header.Wnode.Guid),
        events_lost: header.EventsLost,
        log_buffers_lost: header.LogBuffersLost,
        realtime_buffers_lost: header.RealTimeBuffersLost,
        buffer_size_kib: header.BufferSize,
        minimum_buffers: header.MinimumBuffers,
        maximum_buffers: header.MaximumBuffers,
        number_of_buffers: header.NumberOfBuffers,
        free_buffers: header.FreeBuffers,
        buffers_written: header.BuffersWritten,
        log_file_mode: header.LogFileMode,
    }))
}

pub fn query(name: &str) -> ProbeResult<Report> {
    validate_name(name)?;
    let mut properties = Properties::new();
    let wide_name: Vec<_> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let before_qpc = clock::now()?;
    // SAFETY: aligned, initialized properties include maximum-length UTF-16 name buffers.
    // The only control code is QUERY; this never changes, flushes or stops any session.
    let status = unsafe {
        ControlTraceW(
            CONTROLTRACE_HANDLE::default(),
            PCWSTR(wide_name.as_ptr()),
            &mut properties.header,
            EVENT_TRACE_CONTROL_QUERY,
        )
    };
    let after_qpc = clock::now()?;
    let (statistics, response_validation_error) = match statistics(name, status.0, &properties) {
        Ok(statistics) => (statistics, None),
        Err(error) => (None, Some(error)),
    };
    Ok(Report {
        session_name: name.to_owned(),
        before_qpc,
        after_qpc,
        status: status.0,
        status_message: windows::core::Error::from_hresult(windows::core::HRESULT::from_win32(status.0)).to_string(),
        response_validation_error,
        statistics,
        scope: "Read-only live ETW session snapshot bounded by QPC; not final shutdown statistics or independent proof of zero loss for the entire capture",
    })
}

#[cfg(test)]
mod tests;
