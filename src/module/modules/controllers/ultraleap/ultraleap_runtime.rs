#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::env;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::mem::MaybeUninit;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::ptr;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::thread::{self, JoinHandle};

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use libloading::Library;

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
const ULTRALEAP_WORKER_POLL_TIMEOUT_MS: u32 = 25;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
const ULTRALEAP_BURST_POLL_LIMIT: usize = 64;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
const LEAPSDK_LIB_PATH: &str = "LEAPSDK_LIB_PATH";
const MILLIMETERS_PER_METER: f64 = 1000.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UltraleapVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl UltraleapVec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn distance(self, other: Self) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        let dz = other.z - self.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UltraleapHandSnapshot {
    pub active: bool,
    pub grab_strength: f64,
    pub pinch_strength: f64,
    pub pinch_distance: f64,
    pub thumb_extended: bool,
    pub index_extended: bool,
    pub middle_extended: bool,
    pub ring_extended: bool,
    pub pinky_extended: bool,
    pub palm_position: UltraleapVec3,
    pub palm_stabilized_position: UltraleapVec3,
    pub palm_velocity: UltraleapVec3,
    pub palm_direction: UltraleapVec3,
    pub palm_normal: UltraleapVec3,
}

impl Default for UltraleapHandSnapshot {
    fn default() -> Self {
        Self {
            active: false,
            grab_strength: 0.0,
            pinch_strength: 0.0,
            pinch_distance: 0.0,
            thumb_extended: false,
            index_extended: false,
            middle_extended: false,
            ring_extended: false,
            pinky_extended: false,
            palm_position: UltraleapVec3::ZERO,
            palm_stabilized_position: UltraleapVec3::ZERO,
            palm_velocity: UltraleapVec3::ZERO,
            palm_direction: UltraleapVec3::ZERO,
            palm_normal: UltraleapVec3::ZERO,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UltraleapFrameSnapshot {
    pub hand_count: usize,
    pub left: UltraleapHandSnapshot,
    pub right: UltraleapHandSnapshot,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UltraleapRuntimePoll {
    pub service_connected: bool,
    pub connected_devices: usize,
    pub frame: Option<UltraleapFrameSnapshot>,
    pub last_event: Option<String>,
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
#[derive(Clone, Debug, PartialEq)]
enum UltraleapWorkerEvent {
    Poll(UltraleapRuntimePoll),
    Error(String),
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
pub struct UltraleapRuntime {
    event_rx: Receiver<UltraleapWorkerEvent>,
    shutdown_tx: Sender<()>,
    worker: Option<JoinHandle<()>>,
    service_connected: bool,
    connected_devices: usize,
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub struct UltraleapRuntime;

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
impl UltraleapRuntime {
    pub fn create() -> Result<Self, String> {
        let (event_tx, event_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let worker = spawn_worker(event_tx, ready_tx, shutdown_rx)?;

        ready_rx
            .recv()
            .map_err(|_| "Ultraleap worker stopped before becoming ready".to_string())??;

        Ok(Self {
            event_rx,
            shutdown_tx,
            worker: Some(worker),
            service_connected: false,
            connected_devices: 0,
        })
    }

    pub fn poll(&mut self) -> Result<UltraleapRuntimePoll, String> {
        let mut latest = UltraleapRuntimePoll {
            service_connected: self.service_connected,
            connected_devices: self.connected_devices,
            frame: None,
            last_event: None,
        };
        let mut received_poll = false;

        loop {
            match self.event_rx.try_recv() {
                Ok(UltraleapWorkerEvent::Poll(poll)) => {
                    self.service_connected = poll.service_connected;
                    self.connected_devices = poll.connected_devices;
                    latest = poll;
                    received_poll = true;
                }
                Ok(UltraleapWorkerEvent::Error(error)) => return Err(error),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err("Ultraleap worker stopped unexpectedly".to_string());
                }
            }
        }

        if received_poll {
            return Ok(latest);
        }

        Ok(UltraleapRuntimePoll {
            service_connected: self.service_connected,
            connected_devices: self.connected_devices,
            frame: None,
            last_event: None,
        })
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
impl Drop for UltraleapRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
impl UltraleapRuntime {
    pub fn create() -> Result<Self, String> {
        Err("Ultraleap not supported on this OS.".to_string())
    }

    pub fn poll(&mut self) -> Result<UltraleapRuntimePoll, String> {
        Ok(UltraleapRuntimePoll::default())
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
type CreateConnectionFn = unsafe extern "C" fn(
    *const leap_sys::LEAP_CONNECTION_CONFIG,
    *mut leap_sys::LEAP_CONNECTION,
) -> leap_sys::eLeapRS;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
type OpenConnectionFn = unsafe extern "C" fn(leap_sys::LEAP_CONNECTION) -> leap_sys::eLeapRS;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
type PollConnectionFn = unsafe extern "C" fn(
    leap_sys::LEAP_CONNECTION,
    u32,
    *mut leap_sys::LEAP_CONNECTION_MESSAGE,
) -> leap_sys::eLeapRS;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
type GetDeviceListFn = unsafe extern "C" fn(
    leap_sys::LEAP_CONNECTION,
    *mut leap_sys::LEAP_DEVICE_REF,
    *mut u32,
) -> leap_sys::eLeapRS;
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
type CloseConnectionFn = unsafe extern "C" fn(leap_sys::LEAP_CONNECTION);
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
type DestroyConnectionFn = unsafe extern "C" fn(leap_sys::LEAP_CONNECTION);

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
struct LeapApi {
    _library: Library,
    create_connection: CreateConnectionFn,
    open_connection: OpenConnectionFn,
    poll_connection: PollConnectionFn,
    get_device_list: GetDeviceListFn,
    close_connection: CloseConnectionFn,
    destroy_connection: DestroyConnectionFn,
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
struct LeapConnection {
    api: LeapApi,
    handle: leap_sys::LEAP_CONNECTION,
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LeapCallError {
    Timeout,
    NotConnected,
    HandshakeIncomplete,
    NotAvailable,
    InsufficientBuffer,
    Status(leap_sys::eLeapRS),
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UltraleapHandSide {
    Left,
    Right,
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
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

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
impl LeapConnection {
    fn create() -> Result<Self, String> {
        let api = LeapApi::load()?;
        let mut handle = ptr::null_mut();

        check_leap_call(
            "create an Ultraleap connection",
            unsafe { (api.create_connection)(ptr::null(), &mut handle) },
        )?;

        if handle.is_null() {
            return Err("Failed to create an Ultraleap connection: LeapC returned a null handle."
                .to_string());
        }

        let connection = Self { api, handle };
        check_leap_call(
            "open the Ultraleap connection",
            unsafe { (connection.api.open_connection)(connection.handle) },
        )?;

        Ok(connection)
    }

    fn poll(&mut self, timeout_ms: u32) -> Result<leap_sys::LEAP_CONNECTION_MESSAGE, LeapCallError> {
        let mut message = MaybeUninit::<leap_sys::LEAP_CONNECTION_MESSAGE>::zeroed();
        let status = unsafe {
            (self.api.poll_connection)(self.handle, timeout_ms, message.as_mut_ptr())
        };

        if status == leap_sys::_eLeapRS_eLeapRS_Success {
            return Ok(unsafe { message.assume_init() });
        }

        Err(LeapCallError::from_status(status))
    }

    fn connected_devices(&mut self) -> Result<usize, LeapCallError> {
        let mut device_count = 0_u32;
        let status = unsafe {
            (self.api.get_device_list)(self.handle, ptr::null_mut(), &mut device_count)
        };

        match status {
            leap_sys::_eLeapRS_eLeapRS_Success => Ok(device_count as usize),
            leap_sys::_eLeapRS_eLeapRS_InsufficientBuffer => {
                if device_count == 0 {
                    return Ok(0);
                }

                let mut devices = vec![
                    leap_sys::LEAP_DEVICE_REF {
                        handle: ptr::null_mut(),
                        id: 0,
                    };
                    device_count as usize
                ];

                let status = unsafe {
                    (self.api.get_device_list)(self.handle, devices.as_mut_ptr(), &mut device_count)
                };

                if status == leap_sys::_eLeapRS_eLeapRS_Success {
                    Ok(device_count as usize)
                } else {
                    Err(LeapCallError::from_status(status))
                }
            }
            _ => Err(LeapCallError::from_status(status)),
        }
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
impl Drop for LeapConnection {
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

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
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

    fn describe(self) -> String {
        match self {
            Self::Timeout => "timed out".to_string(),
            Self::NotConnected => "the Ultraleap Tracking Service is not connected".to_string(),
            Self::HandshakeIncomplete => {
                "the Ultraleap Tracking Service handshake is still in progress".to_string()
            }
            Self::NotAvailable => "the Ultraleap Tracking Service is not available".to_string(),
            Self::InsufficientBuffer => "the supplied buffer was too small".to_string(),
            Self::Status(status) => format!("LeapC returned error code {status}"),
        }
    }

    fn is_service_unavailable(self) -> bool {
        matches!(
            self,
            Self::NotConnected | Self::HandshakeIncomplete | Self::NotAvailable
        )
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
unsafe fn load_symbol<T: Copy>(library: &Library, name: &[u8], label: &str) -> Result<T, String> {
    library
        .get::<T>(name)
        .map(|symbol| *symbol)
        .map_err(|error| format!("failed to resolve {label}: {error}"))
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn check_leap_call(action: &str, status: leap_sys::eLeapRS) -> Result<(), String> {
    if status == leap_sys::_eLeapRS_eLeapRS_Success {
        return Ok(());
    }

    Err(format!(
        "Failed to {action}: {}",
        LeapCallError::from_status(status).describe()
    ))
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn ultraleap_library_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(value) = env::var_os(LEAPSDK_LIB_PATH) {
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
            PathBuf::from(
                r"/Applications/Ultraleap Hand Tracking Service.app/Contents/LeapSDK/lib",
            )
            .join("libLeapC.dylib"),
        );
        push_ultraleap_candidate(
            &mut candidates,
            PathBuf::from(r"/Applications/Ultraleap Hand Tracking.app/Contents/LeapSDK/lib")
                .join("libLeapC.dylib"),
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

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
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

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn spawn_worker(
    event_tx: Sender<UltraleapWorkerEvent>,
    ready_tx: mpsc::SyncSender<Result<(), String>>,
    shutdown_rx: Receiver<()>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("ultraleap-input".to_string())
        .spawn(move || worker_main(event_tx, ready_tx, shutdown_rx))
        .map_err(|error| format!("Failed to start Ultraleap worker thread: {error}"))
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn worker_main(
    event_tx: Sender<UltraleapWorkerEvent>,
    ready_tx: mpsc::SyncSender<Result<(), String>>,
    shutdown_rx: Receiver<()>,
) {
    let mut connection = match LeapConnection::create() {
        Ok(connection) => connection,
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return;
        }
    };

    if ready_tx.send(Ok(())).is_err() {
        return;
    }

    let mut service_connected = false;
    let mut connected_devices = 0usize;

    loop {
        match shutdown_rx.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        let cycle = match collect_runtime_poll(&mut connection, service_connected, connected_devices) {
            Ok(cycle) => cycle,
            Err(error) => {
                let _ = event_tx.send(UltraleapWorkerEvent::Error(error));
                break;
            }
        };

        service_connected = cycle.service_connected;
        connected_devices = cycle.connected_devices;

        if let Some(poll) = cycle.poll {
            if event_tx.send(UltraleapWorkerEvent::Poll(poll)).is_err() {
                break;
            }
        }
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
struct WorkerCycle {
    service_connected: bool,
    connected_devices: usize,
    poll: Option<UltraleapRuntimePoll>,
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn collect_runtime_poll(
    connection: &mut LeapConnection,
    previous_service_connected: bool,
    previous_connected_devices: usize,
) -> Result<WorkerCycle, String> {
    let mut latest_frame = None;
    let mut last_event = None;
    let mut service_connected = previous_service_connected;
    let mut connected_devices = previous_connected_devices;
    let mut device_list_dirty = false;
    let mut poll_dirty = false;
    let mut first_poll_received = false;

    match connection.poll(ULTRALEAP_WORKER_POLL_TIMEOUT_MS) {
        Ok(message) => {
            first_poll_received = true;
            handle_event(
                &message,
                &mut service_connected,
                &mut device_list_dirty,
                &mut latest_frame,
                &mut last_event,
                &mut poll_dirty,
            );
        }
        Err(LeapCallError::Timeout) => {}
        Err(error) if error.is_service_unavailable() => {
            return Ok(disconnected_worker_cycle(
                previous_service_connected,
                previous_connected_devices,
            ));
        }
        Err(error) => {
            return Err(format!(
                "Ultraleap runtime poll failed: {}",
                error.describe()
            ));
        }
    }

    if first_poll_received {
        for _ in 1..ULTRALEAP_BURST_POLL_LIMIT {
            match connection.poll(0) {
                Ok(message) => handle_event(
                    &message,
                    &mut service_connected,
                    &mut device_list_dirty,
                    &mut latest_frame,
                    &mut last_event,
                    &mut poll_dirty,
                ),
                Err(LeapCallError::Timeout) => break,
                Err(error) if error.is_service_unavailable() => {
                    service_connected = false;
                    connected_devices = 0;
                    latest_frame = None;
                    poll_dirty = true;
                    last_event = Some("Ultraleap Tracking Service unavailable".to_string());
                    break;
                }
                Err(error) => {
                    return Err(format!(
                        "Ultraleap runtime poll failed: {}",
                        error.describe()
                    ));
                }
            }
        }
    }

    if !service_connected {
        connected_devices = 0;
    } else if device_list_dirty || connected_devices == 0 {
        connected_devices = refresh_connected_devices(connection)?;
    }

    if service_connected != previous_service_connected || connected_devices != previous_connected_devices {
        poll_dirty = true;
    }

    if poll_dirty || latest_frame.is_some() || last_event.is_some() {
        return Ok(WorkerCycle {
            service_connected,
            connected_devices,
            poll: Some(UltraleapRuntimePoll {
                service_connected,
                connected_devices,
                frame: latest_frame,
                last_event,
            }),
        });
    }

    Ok(WorkerCycle {
        service_connected,
        connected_devices,
        poll: None,
    })
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn disconnected_worker_cycle(
    previous_service_connected: bool,
    previous_connected_devices: usize,
) -> WorkerCycle {
    let poll = if previous_service_connected || previous_connected_devices != 0 {
        Some(UltraleapRuntimePoll {
            service_connected: false,
            connected_devices: 0,
            frame: None,
            last_event: Some("Ultraleap Tracking Service unavailable".to_string()),
        })
    } else {
        None
    };

    WorkerCycle {
        service_connected: false,
        connected_devices: 0,
        poll,
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn handle_event(
    message: &leap_sys::LEAP_CONNECTION_MESSAGE,
    service_connected: &mut bool,
    device_list_dirty: &mut bool,
    latest_frame: &mut Option<UltraleapFrameSnapshot>,
    last_event: &mut Option<String>,
    poll_dirty: &mut bool,
) {
    match message_event_type(message) {
        leap_sys::_eLeapEventType_eLeapEventType_None => {}
        leap_sys::_eLeapEventType_eLeapEventType_Connection => {
            *service_connected = true;
            *device_list_dirty = true;
            *poll_dirty = true;
            *last_event = Some("Connected to Ultraleap Tracking Service".to_string());
        }
        leap_sys::_eLeapEventType_eLeapEventType_ConnectionLost => {
            *service_connected = false;
            *device_list_dirty = true;
            *poll_dirty = true;
            *latest_frame = None;
            *last_event = Some("Lost Ultraleap Tracking Service connection".to_string());
        }
        leap_sys::_eLeapEventType_eLeapEventType_Device => {
            *service_connected = true;
            *device_list_dirty = true;
            *poll_dirty = true;
            *last_event = Some("Ultraleap device connected".to_string());
        }
        leap_sys::_eLeapEventType_eLeapEventType_DeviceLost => {
            *device_list_dirty = true;
            *poll_dirty = true;
            *latest_frame = None;
            *last_event = Some("Ultraleap device disconnected".to_string());
        }
        leap_sys::_eLeapEventType_eLeapEventType_DeviceFailure => {
            *device_list_dirty = true;
            *poll_dirty = true;
            *last_event = Some("Ultraleap device failure".to_string());
        }
        leap_sys::_eLeapEventType_eLeapEventType_DeviceStatusChange => {
            *device_list_dirty = true;
            *poll_dirty = true;
            *last_event = Some("Ultraleap device status changed".to_string());
        }
        leap_sys::_eLeapEventType_eLeapEventType_Tracking => {
            *service_connected = true;
            *latest_frame = tracking_event_from_message(message).map(snapshot_from_tracking);
        }
        _ => {}
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn refresh_connected_devices(connection: &mut LeapConnection) -> Result<usize, String> {
    match connection.connected_devices() {
        Ok(device_count) => Ok(device_count),
        Err(error) if error.is_service_unavailable() => Ok(0),
        Err(error) => Err(format!(
            "Failed to enumerate Ultraleap devices: {}",
            error.describe()
        )),
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn message_event_type(message: &leap_sys::LEAP_CONNECTION_MESSAGE) -> leap_sys::eLeapEventType {
    unsafe { ptr::read_unaligned(ptr::addr_of!(message.type_)) }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn tracking_event_from_message(
    message: &leap_sys::LEAP_CONNECTION_MESSAGE,
) -> Option<*const leap_sys::LEAP_TRACKING_EVENT> {
    let event = unsafe { ptr::read_unaligned(ptr::addr_of!(message.__bindgen_anon_1.tracking_event)) };
    if event.is_null() {
        None
    } else {
        Some(event)
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn snapshot_from_tracking(event: *const leap_sys::LEAP_TRACKING_EVENT) -> UltraleapFrameSnapshot {
    let mut snapshot = UltraleapFrameSnapshot::default();
    if event.is_null() {
        return snapshot;
    }

    let event = unsafe { ptr::read_unaligned(event) };
    let hand_count = unsafe { ptr::read_unaligned(ptr::addr_of!(event.nHands)) as usize };
    let hands = unsafe { ptr::read_unaligned(ptr::addr_of!(event.pHands)) as *const leap_sys::LEAP_HAND };

    snapshot.hand_count = hand_count;

    if hands.is_null() {
        return snapshot;
    }

    for index in 0..hand_count {
        let hand = unsafe { hands.add(index) };
        match hand_side(hand) {
            Some(UltraleapHandSide::Left) => snapshot.left = snapshot_from_hand(hand),
            Some(UltraleapHandSide::Right) => snapshot.right = snapshot_from_hand(hand),
            None => {}
        }
    }

    snapshot
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn hand_side(hand: *const leap_sys::LEAP_HAND) -> Option<UltraleapHandSide> {
    if hand.is_null() {
        return None;
    }

    let hand = unsafe { ptr::read_unaligned(hand) };
    match unsafe { ptr::read_unaligned(ptr::addr_of!(hand.type_)) } {
        leap_sys::_eLeapHandType_eLeapHandType_Left => Some(UltraleapHandSide::Left),
        leap_sys::_eLeapHandType_eLeapHandType_Right => Some(UltraleapHandSide::Right),
        _ => None,
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn snapshot_from_hand(hand: *const leap_sys::LEAP_HAND) -> UltraleapHandSnapshot {
    let hand = unsafe { ptr::read_unaligned(hand) };
    let palm = unsafe { ptr::read_unaligned(ptr::addr_of!(hand.palm)) };
    let digits = unsafe { ptr::read_unaligned(ptr::addr_of!(hand.__bindgen_anon_1.digits)) };

    UltraleapHandSnapshot {
        active: true,
        grab_strength: unsafe { ptr::read_unaligned(ptr::addr_of!(hand.grab_strength)) as f64 },
        pinch_strength: unsafe { ptr::read_unaligned(ptr::addr_of!(hand.pinch_strength)) as f64 },
        pinch_distance: millimeters_to_meters(
            unsafe { ptr::read_unaligned(ptr::addr_of!(hand.pinch_distance)) as f64 },
        ),
        thumb_extended: digit_is_extended(&digits, 0),
        index_extended: digit_is_extended(&digits, 1),
        middle_extended: digit_is_extended(&digits, 2),
        ring_extended: digit_is_extended(&digits, 3),
        pinky_extended: digit_is_extended(&digits, 4),
        palm_position: leap_vec3_meters(ptr::addr_of!(palm.position)),
        palm_stabilized_position: leap_vec3_meters(ptr::addr_of!(palm.stabilized_position)),
        palm_velocity: leap_vec3_meters(ptr::addr_of!(palm.velocity)),
        palm_direction: leap_vec3(ptr::addr_of!(palm.direction)),
        palm_normal: leap_vec3(ptr::addr_of!(palm.normal)),
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn digit_is_extended(digits: &[leap_sys::LEAP_DIGIT; 5], index: usize) -> bool {
    let digit = digits.as_ptr().wrapping_add(index);
    unsafe { ptr::read_unaligned(ptr::addr_of!((*digit).is_extended)) != 0 }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn leap_vec3(vector: *const leap_sys::LEAP_VECTOR) -> UltraleapVec3 {
    if vector.is_null() {
        return UltraleapVec3::ZERO;
    }

    let vector = unsafe { ptr::read_unaligned(vector) };
    let [x, y, z] = unsafe { ptr::read_unaligned(ptr::addr_of!(vector.__bindgen_anon_1.v)) };
    UltraleapVec3::new(x as f64, y as f64, z as f64)
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn leap_vec3_meters(vector: *const leap_sys::LEAP_VECTOR) -> UltraleapVec3 {
    let vector = leap_vec3(vector);
    UltraleapVec3::new(
        millimeters_to_meters(vector.x),
        millimeters_to_meters(vector.y),
        millimeters_to_meters(vector.z),
    )
}

fn millimeters_to_meters(value: f64) -> f64 {
    value / MILLIMETERS_PER_METER
}
