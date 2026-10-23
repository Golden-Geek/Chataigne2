use libloading::Library;
use std::{env, mem::MaybeUninit, path::PathBuf, ptr};
const LEAPSDK_LIB_PATH: &str = "LEAPSDK_LIB_PATH";

type CreateConnectionFn =
    unsafe extern "C" fn(*const leap_sys::LEAP_CONNECTION_CONFIG, *mut leap_sys::LEAP_CONNECTION) -> leap_sys::eLeapRS;
type OpenConnectionFn = unsafe extern "C" fn(leap_sys::LEAP_CONNECTION) -> leap_sys::eLeapRS;
type PollConnectionFn =
    unsafe extern "C" fn(leap_sys::LEAP_CONNECTION, u32, *mut leap_sys::LEAP_CONNECTION_MESSAGE) -> leap_sys::eLeapRS;
type GetDeviceListFn =
    unsafe extern "C" fn(leap_sys::LEAP_CONNECTION, *mut leap_sys::LEAP_DEVICE_REF, *mut u32) -> leap_sys::eLeapRS;
type CloseConnectionFn = unsafe extern "C" fn(leap_sys::LEAP_CONNECTION);
type DestroyConnectionFn = unsafe extern "C" fn(leap_sys::LEAP_CONNECTION);

struct LeapApi {
    _library: Library,
    create_connection: CreateConnectionFn,
    open_connection: OpenConnectionFn,
    poll_connection: PollConnectionFn,
    get_device_list: GetDeviceListFn,
    close_connection: CloseConnectionFn,
    destroy_connection: DestroyConnectionFn,
}

pub(super) struct Connection {
    api: LeapApi,
    handle: leap_sys::LEAP_CONNECTION,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LeapCallError {
    Timeout,
    NotConnected,
    HandshakeIncomplete,
    NotAvailable,
    InsufficientBuffer,
    Status(leap_sys::eLeapRS),
}

impl LeapApi {
    fn load() -> Result<Self, String> {
        let mut last_error = None;

        for candidate in ultraleap_library_candidates() {
            let library = match unsafe { Library::new(&candidate) } {
                Ok(library) => library,
                Err(error) => {
                    last_error = Some(format!("{} ({error})", candidate.display()));
                    continue;
                }
            };

            match unsafe { Self::from_library(library) } {
                Ok(api) => return Ok(api),
                Err(error) => {
                    last_error = Some(format!("{} ({error})", candidate.display()));
                }
            }
        }

        let mut message = format!(
            "Ultraleap runtime library was not found or could not be loaded. Install the Ultraleap Tracking Software or set {LEAPSDK_LIB_PATH}."
        );
        if let Some(error) = last_error {
            message.push_str(" Last error: ");
            message.push_str(&error);
        }
        Err(message)
    }

    unsafe fn from_library(library: Library) -> Result<Self, String> {
        Ok(Self {
            create_connection: load_symbol(&library, b"LeapCreateConnection\0", "LeapCreateConnection")?,
            open_connection: load_symbol(&library, b"LeapOpenConnection\0", "LeapOpenConnection")?,
            poll_connection: load_symbol(&library, b"LeapPollConnection\0", "LeapPollConnection")?,
            get_device_list: load_symbol(&library, b"LeapGetDeviceList\0", "LeapGetDeviceList")?,
            close_connection: load_symbol(&library, b"LeapCloseConnection\0", "LeapCloseConnection")?,
            destroy_connection: load_symbol(&library, b"LeapDestroyConnection\0", "LeapDestroyConnection")?,
            _library: library,
        })
    }
}

impl Connection {
    pub(super) fn create() -> Result<Self, String> {
        if cfg!(test) {
            return Err("Native Ultraleap endpoints are disabled in test builds".into());
        }
        let api = LeapApi::load()?;
        let mut handle = ptr::null_mut();

        check_leap_call("create an Ultraleap connection", unsafe {
            (api.create_connection)(ptr::null(), &mut handle)
        })?;

        if handle.is_null() {
            return Err("Failed to create an Ultraleap connection: LeapC returned a null handle.".to_string());
        }

        let connection = Self { api, handle };
        check_leap_call("open the Ultraleap connection", unsafe {
            (connection.api.open_connection)(connection.handle)
        })?;

        Ok(connection)
    }

    pub(super) fn poll(&mut self, timeout_ms: u32) -> Result<leap_sys::LEAP_CONNECTION_MESSAGE, LeapCallError> {
        let mut message = MaybeUninit::<leap_sys::LEAP_CONNECTION_MESSAGE>::zeroed();
        let status = unsafe { (self.api.poll_connection)(self.handle, timeout_ms, message.as_mut_ptr()) };

        if status == leap_sys::_eLeapRS_eLeapRS_Success {
            return Ok(unsafe { message.assume_init() });
        }

        Err(LeapCallError::from_status(status))
    }

    pub(super) fn connected_devices(&mut self) -> Result<usize, LeapCallError> {
        let mut device_count = 0_u32;
        let status = unsafe { (self.api.get_device_list)(self.handle, ptr::null_mut(), &mut device_count) };

        match status {
            leap_sys::_eLeapRS_eLeapRS_Success if device_count <= 64 => Ok(device_count as usize),
            leap_sys::_eLeapRS_eLeapRS_Success => Err(LeapCallError::InsufficientBuffer),
            leap_sys::_eLeapRS_eLeapRS_InsufficientBuffer => {
                if device_count == 0 {
                    return Ok(0);
                }

                if device_count > 64 {
                    return Err(LeapCallError::InsufficientBuffer);
                }
                let mut devices = vec![
                    leap_sys::LEAP_DEVICE_REF {
                        handle: ptr::null_mut(),
                        id: 0,
                    };
                    device_count as usize
                ];

                let status =
                    unsafe { (self.api.get_device_list)(self.handle, devices.as_mut_ptr(), &mut device_count) };

                if status == leap_sys::_eLeapRS_eLeapRS_Success {
                    if device_count as usize > devices.len() {
                        return Err(LeapCallError::InsufficientBuffer);
                    }
                    Ok(device_count as usize)
                } else {
                    Err(LeapCallError::from_status(status))
                }
            }
            _ => Err(LeapCallError::from_status(status)),
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }

        unsafe {
            (self.api.close_connection)(self.handle);
            (self.api.destroy_connection)(self.handle);
        }
    }
}

impl LeapCallError {
    fn from_status(status: leap_sys::eLeapRS) -> Self {
        match status {
            leap_sys::_eLeapRS_eLeapRS_Timeout => Self::Timeout,
            leap_sys::_eLeapRS_eLeapRS_NotConnected => Self::NotConnected,
            leap_sys::_eLeapRS_eLeapRS_HandshakeIncomplete => Self::HandshakeIncomplete,
            leap_sys::_eLeapRS_eLeapRS_NotAvailable => Self::NotAvailable,
            leap_sys::_eLeapRS_eLeapRS_InsufficientBuffer => Self::InsufficientBuffer,
            _ => Self::Status(status),
        }
    }

    pub(super) fn describe(self) -> String {
        match self {
            Self::Timeout => "timed out".to_string(),
            Self::NotConnected => "the Ultraleap Tracking Service is not connected".to_string(),
            Self::HandshakeIncomplete => "the Ultraleap Tracking Service handshake is still in progress".to_string(),
            Self::NotAvailable => "the Ultraleap Tracking Service is not available".to_string(),
            Self::InsufficientBuffer => "the supplied buffer was too small".to_string(),
            Self::Status(status) => format!("LeapC returned error code {status}"),
        }
    }

    pub(super) fn is_service_unavailable(self) -> bool {
        matches!(
            self,
            Self::NotConnected | Self::HandshakeIncomplete | Self::NotAvailable
        )
    }
}

unsafe fn load_symbol<T: Copy>(library: &Library, name: &[u8], label: &str) -> Result<T, String> {
    library
        .get::<T>(name)
        .map(|symbol| *symbol)
        .map_err(|error| format!("failed to resolve {label}: {error}"))
}

fn check_leap_call(action: &str, status: leap_sys::eLeapRS) -> Result<(), String> {
    if status == leap_sys::_eLeapRS_eLeapRS_Success {
        return Ok(());
    }

    Err(format!(
        "Failed to {action}: {}",
        LeapCallError::from_status(status).describe()
    ))
}

fn ultraleap_library_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(value) = env::var_os(LEAPSDK_LIB_PATH) {
        if value.len() > 4096 {
            return candidates;
        }
        let path = PathBuf::from(value);
        if path.is_dir() {
            push_ultraleap_candidate(&mut candidates, path.join(ultraleap_library_name()));
        } else {
            push_ultraleap_candidate(&mut candidates, path);
        }
    }

    #[cfg(windows)]
    {
        push_ultraleap_candidate(
            &mut candidates,
            PathBuf::from(r"C:\Program Files\Ultraleap\LeapSDK\lib\x64").join("LeapC.dll"),
        );
    }

    #[cfg(target_os = "macos")]
    {
        push_ultraleap_candidate(
            &mut candidates,
            PathBuf::from(r"/Applications/Ultraleap Hand Tracking Service.app/Contents/LeapSDK/lib")
                .join("libLeapC.dylib"),
        );
        push_ultraleap_candidate(
            &mut candidates,
            PathBuf::from(r"/Applications/Ultraleap Hand Tracking.app/Contents/LeapSDK/lib").join("libLeapC.dylib"),
        );
    }

    #[cfg(target_os = "linux")]
    {
        push_ultraleap_candidate(
            &mut candidates,
            PathBuf::from("/usr/lib/ultraleap-hand-tracking-service").join("libLeapC.so"),
        );
        push_ultraleap_candidate(
            &mut candidates,
            PathBuf::from("/usr/share/doc/ultraleap-hand-tracking-service").join("libLeapC.so"),
        );
    }

    push_ultraleap_candidate(&mut candidates, PathBuf::from(ultraleap_library_name()));
    candidates
}

fn push_ultraleap_candidate(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

#[cfg(windows)]
fn ultraleap_library_name() -> &'static str {
    "LeapC.dll"
}

#[cfg(target_os = "macos")]
fn ultraleap_library_name() -> &'static str {
    "libLeapC.dylib"
}

#[cfg(target_os = "linux")]
fn ultraleap_library_name() -> &'static str {
    "libLeapC.so"
}
