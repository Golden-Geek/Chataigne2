use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::JoinHandle;
#[cfg(not(windows))]
use std::time::Duration;
#[cfg(not(windows))]
use std::{sync::mpsc::Sender, thread};

use device_query::{DeviceQuery, DeviceState};
use rdev::{display_size, simulate, Button as RdevButton, EventType, SimulateError};

use crate::app::module::common::mouse::{
    MouseButtonAction, MouseButtonKind, MouseMoveCoordinate, MouseMoveRequest, MouseMoveUnits,
    MouseScrollRequest,
};

#[cfg(not(windows))]
const GLOBAL_MOUSE_VARIANT: &str = "system|System Mouse";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredMouseDevice {
    pub index: usize,
    pub variant_id: String,
    pub label: String,
    pub details: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MouseInputConfig {
    pub capture_os_input: bool,
    pub selection: String,
}

impl Default for MouseInputConfig {
    fn default() -> Self {
        Self {
            capture_os_input: false,
            selection: "auto".to_string(),
        }
    }
}

#[cfg(not(windows))]
#[derive(Clone, Debug, Default, PartialEq)]
struct MouseObservedState {
    x: i32,
    y: i32,
    left: bool,
    middle: bool,
    right: bool,
}

#[cfg(not(windows))]
impl MouseObservedState {
    fn from_device_state(device_state: &DeviceState) -> Self {
        let mouse_state = device_state.get_mouse();
        Self {
            x: mouse_state.coords.0,
            y: mouse_state.coords.1,
            left: mouse_state.button_pressed.get(1).copied().unwrap_or(false),
            middle: mouse_state.button_pressed.get(2).copied().unwrap_or(false),
            right: mouse_state.button_pressed.get(3).copied().unwrap_or(false),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MouseInputEvent {
    Moved { x: i32, y: i32, dx: i32, dy: i32 },
    ButtonChanged { button: MouseButtonKind, pressed: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MouseRuntimeEvent {
    DevicesChanged(Vec<DiscoveredMouseDevice>),
    Input {
        device: String,
        event: MouseInputEvent,
    },
}

#[cfg(not(windows))]
fn diff_mouse_states(previous: &MouseObservedState, next: &MouseObservedState) -> Vec<MouseInputEvent> {
    let mut events = Vec::new();

    if previous.x != next.x || previous.y != next.y {
        events.push(MouseInputEvent::Moved {
            x: next.x,
            y: next.y,
            dx: next.x - previous.x,
            dy: next.y - previous.y,
        });
    }

    if previous.left != next.left {
        events.push(MouseInputEvent::ButtonChanged {
            button: MouseButtonKind::Left,
            pressed: next.left,
        });
    }
    if previous.middle != next.middle {
        events.push(MouseInputEvent::ButtonChanged {
            button: MouseButtonKind::Middle,
            pressed: next.middle,
        });
    }
    if previous.right != next.right {
        events.push(MouseInputEvent::ButtonChanged {
            button: MouseButtonKind::Right,
            pressed: next.right,
        });
    }

    events
}

pub(crate) struct MouseInputRuntime {
    inner: MouseInputRuntimeInner,
}

impl MouseInputRuntime {
    pub(crate) fn create(config: MouseInputConfig) -> Result<Self, String> {
        #[cfg(windows)]
        {
            WindowsMouseInputRuntime::create(config).map(|runtime| Self {
                inner: MouseInputRuntimeInner::Windows(runtime),
            })
        }

        #[cfg(not(windows))]
        {
            let runtime = GlobalMouseInputRuntime::create(config)?;
            Ok(Self {
                inner: MouseInputRuntimeInner::Global(runtime),
            })
        }
    }

    pub(crate) fn poll_events(&mut self) -> Result<Vec<MouseRuntimeEvent>, String> {
        match &mut self.inner {
            #[cfg(windows)]
            MouseInputRuntimeInner::Windows(runtime) => runtime.poll_events(),
            #[cfg(not(windows))]
            MouseInputRuntimeInner::Global(runtime) => runtime.poll_events(),
        }
    }
}

enum MouseInputRuntimeInner {
    #[cfg(windows)]
    Windows(WindowsMouseInputRuntime),
    #[cfg(not(windows))]
    Global(GlobalMouseInputRuntime),
}

#[cfg(not(windows))]
#[derive(Clone)]
struct GlobalWorkerReady {
    devices: Vec<DiscoveredMouseDevice>,
}

#[cfg(not(windows))]
#[derive(Clone, Debug)]
enum GlobalWorkerEvent {
    Input {
        device: String,
        event: MouseInputEvent,
    },
    Error(String),
}

#[cfg(not(windows))]
struct GlobalMouseInputRuntime {
    event_rx: Receiver<GlobalWorkerEvent>,
    pending_events: Vec<MouseRuntimeEvent>,
    shutdown_tx: Sender<()>,
    worker: Option<JoinHandle<()>>,
}

#[cfg(not(windows))]
impl GlobalMouseInputRuntime {
    fn create(config: MouseInputConfig) -> Result<Self, String> {
        if config.capture_os_input {
            return Err("blocking OS mouse input is only supported on Windows".to_string());
        }

        let (event_tx, event_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let worker = spawn_global_worker(event_tx, ready_tx, shutdown_rx)?;
        let ready = ready_rx
            .recv()
            .map_err(|_| "mouse global-input worker exited before becoming ready".to_string())??;

        Ok(Self {
            event_rx,
            pending_events: vec![MouseRuntimeEvent::DevicesChanged(ready.devices)],
            shutdown_tx,
            worker: Some(worker),
        })
    }

    fn poll_events(&mut self) -> Result<Vec<MouseRuntimeEvent>, String> {
        let mut events = std::mem::take(&mut self.pending_events);
        loop {
            match self.event_rx.try_recv() {
                Ok(GlobalWorkerEvent::Input { device, event }) => {
                    events.push(MouseRuntimeEvent::Input { device, event });
                }
                Ok(GlobalWorkerEvent::Error(error)) => return Err(error),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err("mouse global-input worker stopped unexpectedly".to_string())
                }
            }
        }

        Ok(events)
    }
}

#[cfg(not(windows))]
impl Drop for GlobalMouseInputRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(not(windows))]
fn spawn_global_worker(
    event_tx: Sender<GlobalWorkerEvent>,
    ready_tx: mpsc::SyncSender<Result<GlobalWorkerReady, String>>,
    shutdown_rx: Receiver<()>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("mouse-global-input".to_string())
        .spawn(move || {
            if let Err(error) = global_worker_main(event_tx.clone(), ready_tx, shutdown_rx) {
                let _ = event_tx.send(GlobalWorkerEvent::Error(error));
            }
        })
        .map_err(|error| format!("failed to spawn the mouse global-input worker: {error}"))
}

#[cfg(not(windows))]
fn global_worker_main(
    event_tx: Sender<GlobalWorkerEvent>,
    ready_tx: mpsc::SyncSender<Result<GlobalWorkerReady, String>>,
    shutdown_rx: Receiver<()>,
) -> Result<(), String> {
    let Some(device_state) = DeviceState::checked_new() else {
        let error = "failed to access the local mouse input backend".to_string();
        let _ = ready_tx.send(Err(error.clone()));
        return Err(error);
    };

    if ready_tx
        .send(Ok(GlobalWorkerReady {
            devices: vec![global_mouse_device()],
        }))
        .is_err()
    {
        return Ok(());
    }

    let mut last_state = None;
    loop {
        match shutdown_rx.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {}
        }

        let next_state = MouseObservedState::from_device_state(&device_state);
        if let Some(previous_state) = last_state.as_ref() {
            for event in diff_mouse_states(previous_state, &next_state) {
                if event_tx
                    .send(GlobalWorkerEvent::Input {
                        device: GLOBAL_MOUSE_VARIANT.to_string(),
                        event,
                    })
                    .is_err()
                {
                    return Ok(());
                }
            }
        } else if event_tx
            .send(GlobalWorkerEvent::Input {
                device: GLOBAL_MOUSE_VARIANT.to_string(),
                event: MouseInputEvent::Moved {
                    x: next_state.x,
                    y: next_state.y,
                    dx: 0,
                    dy: 0,
                },
            })
            .is_err()
        {
            return Ok(());
        }
        last_state = Some(next_state);

        thread::sleep(Duration::from_millis(8));
    }

    Ok(())
}

#[cfg(not(windows))]
fn global_mouse_device() -> DiscoveredMouseDevice {
    DiscoveredMouseDevice {
        index: 0,
        variant_id: GLOBAL_MOUSE_VARIANT.to_string(),
        label: "System Mouse".to_string(),
        details: "Global system mouse input".to_string(),
    }
}

#[cfg(windows)]
mod windows_raw_input {
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::mem::{size_of, MaybeUninit};
    use std::ptr::{null, null_mut};
    use std::sync::OnceLock;
    use std::sync::mpsc::{self, SyncSender};
    use std::thread::{self, JoinHandle};
    use std::time::Instant;

    use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::{
        GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList,
        RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
        RAWINPUTDEVICELIST, RAWINPUTHEADER, RIDI_DEVICENAME, RID_INPUT,
        RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIM_TYPEMOUSE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow,
        DispatchMessageW, GetCursorPos, GetMessageW, GetSystemMetrics,
        PostThreadMessageW, RegisterClassW, SetWindowsHookExW, TranslateMessage,
        UnhookWindowsHookEx, HHOOK, HC_ACTION, HWND_MESSAGE, MSG,
        MSLLHOOKSTRUCT, SM_CXVIRTUALSCREEN, SM_CXSCREEN, SM_CYVIRTUALSCREEN,
        SM_CYSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WH_MOUSE_LL,
        WM_INPUT, WM_INPUT_DEVICE_CHANGE, WM_QUIT, WNDCLASSW,
    };

    use super::{DiscoveredMouseDevice, MouseButtonKind, MouseInputConfig, MouseInputEvent};

    const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
    const HID_USAGE_GENERIC_MOUSE: u16 = 0x02;
    const AUTO_MOUSE_SELECTION: &str = "auto";
    const NO_MOUSE_SELECTION: &str = "none";
    const CAPTURE_ACTIVITY_WINDOW_MS: u64 = 250;
    const LOW_LEVEL_MOUSE_INJECTED_FLAG: u32 = 0x00000001;
    const MOUSE_MOVE_ABSOLUTE: u16 = 0x0001;
    const MOUSE_VIRTUAL_DESKTOP: u16 = 0x0002;
    const RAW_INPUT_COORDINATE_MAX: i64 = 65_535;
    const RI_MOUSE_LEFT_BUTTON_DOWN: u16 = 0x0001;
    const RI_MOUSE_LEFT_BUTTON_UP: u16 = 0x0002;
    const RI_MOUSE_RIGHT_BUTTON_DOWN: u16 = 0x0004;
    const RI_MOUSE_RIGHT_BUTTON_UP: u16 = 0x0008;
    const RI_MOUSE_MIDDLE_BUTTON_DOWN: u16 = 0x0010;
    const RI_MOUSE_MIDDLE_BUTTON_UP: u16 = 0x0020;

    thread_local! {
        static CAPTURE_ACTIVE_UNTIL_MS: Cell<u64> = const { Cell::new(0) };
    }

    static CAPTURE_CLOCK_BASE: OnceLock<Instant> = OnceLock::new();

    #[derive(Clone)]
    pub(super) struct WorkerReady {
        pub thread_id: u32,
        pub devices: Vec<DiscoveredMouseDevice>,
    }

    #[derive(Clone, Debug)]
    pub(super) enum WorkerEvent {
        DevicesChanged(Vec<DiscoveredMouseDevice>),
        Input {
            device: String,
            event: MouseInputEvent,
        },
        Error(String),
    }

    #[derive(Clone)]
    struct KnownMouseDevice {
        handle: HANDLE,
        public: DiscoveredMouseDevice,
    }

    pub(super) fn spawn_worker(
        event_tx: mpsc::Sender<WorkerEvent>,
        ready_tx: SyncSender<Result<WorkerReady, String>>,
        config: MouseInputConfig,
    ) -> Result<JoinHandle<()>, String> {
        thread::Builder::new()
            .name("mouse-raw-input".to_string())
            .spawn(move || {
                if let Err(error) = worker_main(event_tx.clone(), ready_tx, config) {
                    let _ = event_tx.send(WorkerEvent::Error(error));
                }
            })
            .map_err(|error| format!("failed to spawn the mouse raw-input worker: {error}"))
    }

    pub(super) fn stop_worker(thread_id: u32) {
        unsafe {
            PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
        }
    }

    fn worker_main(
        event_tx: mpsc::Sender<WorkerEvent>,
        ready_tx: SyncSender<Result<WorkerReady, String>>,
        config: MouseInputConfig,
    ) -> Result<(), String> {
        let thread_id = unsafe { GetCurrentThreadId() };
        let class_name = wide_null("Chataigne2MouseRawInputWindow");
        let instance = unsafe { GetModuleHandleW(null()) };

        let window_class = WNDCLASSW {
            lpfnWndProc: Some(DefWindowProcW),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            ..unsafe { std::mem::zeroed() }
        };
        unsafe {
            RegisterClassW(&window_class);
        }

        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                null_mut(),
                instance,
                null(),
            )
        };
        if hwnd.is_null() {
            let error = "failed to create the hidden mouse raw-input window".to_string();
            let _ = ready_tx.send(Err(error.clone()));
            return Err(error);
        }

        if let Err(error) = register_mouse_input(hwnd) {
            let _ = ready_tx.send(Err(error.clone()));
            unsafe {
                DestroyWindow(hwnd);
            }
            return Err(error);
        }

        let capture_hook = if config.capture_os_input {
            match install_mouse_capture_hook(instance) {
                Ok(hook) => Some(hook),
                Err(error) => {
                    let _ = ready_tx.send(Err(error.clone()));
                    unsafe {
                        DestroyWindow(hwnd);
                    }
                    return Err(error);
                }
            }
        } else {
            None
        };

        clear_capture_window();

        let mut known_devices = enumerate_mice()?;
        let mut device_lookup = build_device_lookup(known_devices.as_slice());
        let mut last_pointer_position = current_cursor_position().unwrap_or((0, 0));

        let _ = ready_tx.send(Ok(WorkerReady {
            thread_id,
            devices: public_devices(known_devices.as_slice()),
        }));

        let mut message = MaybeUninit::<MSG>::zeroed();
        loop {
            let status = unsafe { GetMessageW(message.as_mut_ptr(), null_mut(), 0, 0) };
            if status == -1 {
                teardown_worker(hwnd, capture_hook);
                return Err("mouse raw-input message pump failed".to_string());
            }
            if status == 0 {
                break;
            }

            let message = unsafe { message.assume_init() };
            match message.message {
                WM_INPUT => {
                    let input_events = read_mouse_input(
                        message.lParam,
                        &device_lookup,
                        &mut last_pointer_position,
                    )?;
                    for (device, event) in input_events {
                        if config.capture_os_input
                            && capture_target_matches_selection(
                                config.selection.as_str(),
                                known_devices.as_slice(),
                                device.as_str(),
                            )
                        {
                            arm_capture_window();
                        }
                        let _ = event_tx.send(WorkerEvent::Input { device, event });
                    }
                }
                WM_INPUT_DEVICE_CHANGE => {
                    known_devices = enumerate_mice()?;
                    device_lookup = build_device_lookup(known_devices.as_slice());
                    if !selected_capture_device_available(
                        config.selection.as_str(),
                        known_devices.as_slice(),
                    ) {
                        clear_capture_window();
                    }
                    let _ = event_tx.send(WorkerEvent::DevicesChanged(public_devices(
                        known_devices.as_slice(),
                    )));
                }
                _ => unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                },
            }
        }

        teardown_worker(hwnd, capture_hook);
        Ok(())
    }

    fn teardown_worker(hwnd: HWND, capture_hook: Option<HHOOK>) {
        clear_capture_window();
        if let Some(hook) = capture_hook {
            unsafe {
                UnhookWindowsHookEx(hook);
            }
        }
        unsafe {
            DestroyWindow(hwnd);
        }
    }

    fn register_mouse_input(hwnd: HWND) -> Result<(), String> {
        let device = RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_MOUSE,
            dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
            hwndTarget: hwnd,
        };
        let success = unsafe {
            RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as u32)
        };
        if success == 0 {
            return Err("failed to register the mouse raw-input device sink".to_string());
        }

        Ok(())
    }

    fn install_mouse_capture_hook(instance: *mut core::ffi::c_void) -> Result<HHOOK, String> {
        let hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_capture_proc), instance, 0) };
        if hook.is_null() {
            return Err("failed to install the low-level mouse capture hook".to_string());
        }

        Ok(hook)
    }

    pub(super) fn should_swallow_low_level_mouse_event(
        flags: u32,
        capture_window_active: bool,
    ) -> bool {
        capture_window_active && flags & LOW_LEVEL_MOUSE_INJECTED_FLAG == 0
    }

    unsafe extern "system" fn low_level_mouse_capture_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code == HC_ACTION as i32 && lparam != 0 {
            let hook = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
            if should_swallow_low_level_mouse_event(hook.flags, capture_window_active()) {
                return 1;
            }
        }

        unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
    }

    fn capture_window_active() -> bool {
        let now = capture_now_millis();
        CAPTURE_ACTIVE_UNTIL_MS.with(|deadline| deadline.get() > now)
    }

    fn arm_capture_window() {
        let deadline = capture_now_millis().saturating_add(CAPTURE_ACTIVITY_WINDOW_MS);
        CAPTURE_ACTIVE_UNTIL_MS.with(|value| value.set(deadline));
    }

    fn clear_capture_window() {
        CAPTURE_ACTIVE_UNTIL_MS.with(|value| value.set(0));
    }

    fn capture_now_millis() -> u64 {
        CAPTURE_CLOCK_BASE
            .get_or_init(Instant::now)
            .elapsed()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64
    }

    fn capture_target_matches_selection(
        selection: &str,
        devices: &[KnownMouseDevice],
        device: &str,
    ) -> bool {
        match selection.trim() {
            AUTO_MOUSE_SELECTION => devices
                .first()
                .is_some_and(|selected| selected.public.variant_id == device),
            NO_MOUSE_SELECTION | "" => false,
            selected => {
                selected == device
                    || legacy_mouse_variant_identity(selected)
                        .is_some_and(|identity| identity == device)
            }
        }
    }

    #[cfg(test)]
    pub(super) fn capture_target_matches_public_selection(
        selection: &str,
        devices: &[DiscoveredMouseDevice],
        device: &str,
    ) -> bool {
        match selection.trim() {
            AUTO_MOUSE_SELECTION => devices
                .first()
                .is_some_and(|selected| selected.variant_id == device),
            NO_MOUSE_SELECTION | "" => false,
            selected => {
                selected == device
                    || legacy_mouse_variant_identity(selected)
                        .is_some_and(|identity| identity == device)
            }
        }
    }

    fn selected_capture_device_available(selection: &str, devices: &[KnownMouseDevice]) -> bool {
        match selection.trim() {
            AUTO_MOUSE_SELECTION => !devices.is_empty(),
            NO_MOUSE_SELECTION | "" => false,
            selected => devices.iter().any(|device| {
                capture_target_matches_selection(selected, devices, device.public.variant_id.as_str())
            }),
        }
    }

    fn legacy_mouse_variant_identity(selection: &str) -> Option<&str> {
        let (identity, label) = selection.split_once('|')?;
        if identity.trim().is_empty() || label.trim().is_empty() {
            return None;
        }
        Some(identity)
    }

    fn enumerate_mice() -> Result<Vec<KnownMouseDevice>, String> {
        let mut device_count = 0u32;
        let query_status = unsafe {
            GetRawInputDeviceList(
                null_mut(),
                &mut device_count,
                size_of::<RAWINPUTDEVICELIST>() as u32,
            )
        };
        if query_status == u32::MAX {
            return Err("failed to enumerate raw-input devices".to_string());
        }
        if device_count == 0 {
            return Ok(Vec::new());
        }

        let mut entries = vec![
            RAWINPUTDEVICELIST {
                hDevice: null_mut(),
                dwType: 0,
            };
            device_count as usize
        ];
        let populated = unsafe {
            GetRawInputDeviceList(
                entries.as_mut_ptr(),
                &mut device_count,
                size_of::<RAWINPUTDEVICELIST>() as u32,
            )
        };
        if populated == u32::MAX {
            return Err("failed to read the raw-input device list".to_string());
        }
        entries.truncate(populated as usize);

        let mut mice = Vec::new();
        for entry in entries {
            if entry.dwType != RIM_TYPEMOUSE {
                continue;
            }

            let path = raw_input_device_name(entry.hDevice).unwrap_or_default();
            let label = describe_mouse_label(path.as_str(), entry.hDevice);
            let details = describe_mouse_details(path.as_str());
            mice.push(KnownMouseDevice {
                handle: entry.hDevice,
                public: DiscoveredMouseDevice {
                    index: mice.len(),
                    variant_id: mouse_variant_id(path.as_str(), entry.hDevice),
                    label,
                    details,
                },
            });
        }

        Ok(mice)
    }

    fn build_device_lookup(devices: &[KnownMouseDevice]) -> HashMap<HANDLE, DiscoveredMouseDevice> {
        devices
            .iter()
            .map(|device| (device.handle, device.public.clone()))
            .collect()
    }

    fn public_devices(devices: &[KnownMouseDevice]) -> Vec<DiscoveredMouseDevice> {
        devices.iter().map(|device| device.public.clone()).collect()
    }

    fn raw_input_device_name(handle: HANDLE) -> Result<String, String> {
        let mut length = 0u32;
        let status = unsafe {
            GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, null_mut(), &mut length)
        };
        if status == u32::MAX {
            return Err("failed to query the raw-input mouse name length".to_string());
        }
        if length == 0 {
            return Ok(String::new());
        }

        let mut buffer = vec![0u16; length as usize];
        let status = unsafe {
            GetRawInputDeviceInfoW(
                handle,
                RIDI_DEVICENAME,
                buffer.as_mut_ptr().cast(),
                &mut length,
            )
        };
        if status == u32::MAX {
            return Err("failed to read the raw-input mouse name".to_string());
        }

        if let Some(last) = buffer.last() {
            if *last == 0 {
                buffer.pop();
            }
        }
        Ok(String::from_utf16_lossy(buffer.as_slice()))
    }

    fn read_mouse_input(
        raw_input_lparam: LPARAM,
        device_lookup: &HashMap<HANDLE, DiscoveredMouseDevice>,
        last_pointer_position: &mut (i32, i32),
    ) -> Result<Vec<(String, MouseInputEvent)>, String> {
        let mut size = 0u32;
        let status = unsafe {
            GetRawInputData(
                raw_input_lparam as HRAWINPUT,
                RID_INPUT,
                null_mut(),
                &mut size,
                size_of::<RAWINPUTHEADER>() as u32,
            )
        };
        if status == u32::MAX || size == 0 {
            return Err("failed to measure the raw mouse input packet".to_string());
        }

        let mut buffer = vec![0u8; size as usize];
        let status = unsafe {
            GetRawInputData(
                raw_input_lparam as HRAWINPUT,
                RID_INPUT,
                buffer.as_mut_ptr().cast(),
                &mut size,
                size_of::<RAWINPUTHEADER>() as u32,
            )
        };
        if status == u32::MAX {
            return Err("failed to read the raw mouse input packet".to_string());
        }

        let raw = unsafe { &*(buffer.as_ptr().cast::<RAWINPUT>()) };
        if raw.header.dwType != RIM_TYPEMOUSE {
            return Ok(Vec::new());
        }

        let device = device_lookup
            .get(&raw.header.hDevice)
            .map(|device| device.variant_id.clone())
            .unwrap_or_else(|| fallback_mouse_variant_id(raw.header.hDevice));
        let mut events = Vec::new();
        let mouse = unsafe { raw.data.mouse };
        if let Some((x, y, dx, dy)) = next_mouse_position_from_raw_input(
            mouse.usFlags,
            mouse.lLastX,
            mouse.lLastY,
            *last_pointer_position,
        ) {
            events.push((
                device.clone(),
                MouseInputEvent::Moved {
                    x,
                    y,
                    dx,
                    dy,
                },
            ));
            *last_pointer_position = (x, y);
        }

        let button_flags = unsafe { mouse.Anonymous.Anonymous.usButtonFlags };
        push_button_event(
            &mut events,
            device.as_str(),
            button_flags,
            RI_MOUSE_LEFT_BUTTON_DOWN,
            RI_MOUSE_LEFT_BUTTON_UP,
            MouseButtonKind::Left,
        );
        push_button_event(
            &mut events,
            device.as_str(),
            button_flags,
            RI_MOUSE_MIDDLE_BUTTON_DOWN,
            RI_MOUSE_MIDDLE_BUTTON_UP,
            MouseButtonKind::Middle,
        );
        push_button_event(
            &mut events,
            device.as_str(),
            button_flags,
            RI_MOUSE_RIGHT_BUTTON_DOWN,
            RI_MOUSE_RIGHT_BUTTON_UP,
            MouseButtonKind::Right,
        );

        Ok(events)
    }

    pub(super) fn next_mouse_position_from_raw_input(
        movement_flags: u16,
        raw_x: i32,
        raw_y: i32,
        last_position: (i32, i32),
    ) -> Option<(i32, i32, i32, i32)> {
        if movement_flags & MOUSE_MOVE_ABSOLUTE != 0 {
            let (left, top, width, height) = desktop_bounds(movement_flags);
            let x = normalize_absolute_axis(raw_x, left, width);
            let y = normalize_absolute_axis(raw_y, top, height);
            let dx = x - last_position.0;
            let dy = y - last_position.1;
            return (dx != 0 || dy != 0).then_some((x, y, dx, dy));
        }

        (raw_x != 0 || raw_y != 0).then_some((
            last_position.0 + raw_x,
            last_position.1 + raw_y,
            raw_x,
            raw_y,
        ))
    }

    fn desktop_bounds(movement_flags: u16) -> (i32, i32, i32, i32) {
        if movement_flags & MOUSE_VIRTUAL_DESKTOP != 0 {
            let left = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
            let top = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
            let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) }.max(1);
            let height = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) }.max(1);
            return (left, top, width, height);
        }

        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(1);
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(1);
        (0, 0, width, height)
    }

    fn normalize_absolute_axis(raw_value: i32, origin: i32, extent: i32) -> i32 {
        origin + ((i64::from(raw_value) * i64::from(extent.max(1))) / RAW_INPUT_COORDINATE_MAX) as i32
    }

    fn push_button_event(
        events: &mut Vec<(String, MouseInputEvent)>,
        device: &str,
        button_flags: u16,
        down_flag: u16,
        up_flag: u16,
        button: MouseButtonKind,
    ) {
        if button_flags & down_flag != 0 {
            events.push((
                device.to_string(),
                MouseInputEvent::ButtonChanged {
                    button,
                    pressed: true,
                },
            ));
        }
        if button_flags & up_flag != 0 {
            events.push((
                device.to_string(),
                MouseInputEvent::ButtonChanged {
                    button,
                    pressed: false,
                },
            ));
        }
    }

    fn current_cursor_position() -> Option<(i32, i32)> {
        let mut point = POINT { x: 0, y: 0 };
        let success = unsafe { GetCursorPos(&mut point) };
        (success != 0).then_some((point.x, point.y))
    }

    fn mouse_variant_id(path: &str, handle: HANDLE) -> String {
        mouse_identity(path, handle)
    }

    fn fallback_mouse_variant_id(handle: HANDLE) -> String {
        mouse_identity("", handle)
    }

    fn mouse_identity(path: &str, handle: HANDLE) -> String {
        if !path.trim().is_empty() {
            return path.to_string();
        }

        format!("raw:{:016x}", handle as usize)
    }

    fn describe_mouse_label(path: &str, handle: HANDLE) -> String {
        let upper = path.to_ascii_uppercase();
        if upper.contains("RDP_MOU") {
            return "Remote Desktop Mouse".to_string();
        }
        if let Some((vendor, product)) = parse_vid_pid(upper.as_str()) {
            return format!("Mouse VID {vendor} PID {product}");
        }
        if let Some(identity) = stable_mouse_identity_label(path) {
            return format!("Mouse {identity}");
        }

        format!("Mouse {:04X}", (handle as usize) & 0xffff)
    }

    fn describe_mouse_details(path: &str) -> String {
        if path.trim().is_empty() {
            "Raw Input mouse".to_string()
        } else {
            path.to_string()
        }
    }

    fn parse_vid_pid(path: &str) -> Option<(String, String)> {
        let vendor = extract_four_hex_after(path, "VID_")
            .or_else(|| extract_four_hex_after(path, "VID&"))?;
        let product = extract_four_hex_after(path, "PID_")
            .or_else(|| extract_four_hex_after(path, "PID&"))?;
        Some((vendor.to_string(), product.to_string()))
    }

    fn extract_four_hex_after<'a>(value: &'a str, marker: &str) -> Option<&'a str> {
        let start = value.find(marker)? + marker.len();
        value.get(start..start + 4)
    }

    fn stable_mouse_identity_label(path: &str) -> Option<String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return None;
        }

        let body = trimmed
            .strip_prefix(r"\\?\")
            .or_else(|| trimmed.strip_prefix(r"\??\"))
            .unwrap_or(trimmed);
        let token = body
            .split('#')
            .nth(1)
            .or_else(|| body.split('#').next())?
            .split('&')
            .find(|segment| !segment.is_empty() && !segment.eq_ignore_ascii_case("COL01"))?;
        Some(token.to_string())
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}

#[cfg(windows)]
use self::windows_raw_input::{spawn_worker, stop_worker, WorkerEvent};

#[cfg(windows)]
struct WindowsMouseInputRuntime {
    event_rx: Receiver<WorkerEvent>,
    pending_events: Vec<MouseRuntimeEvent>,
    worker: Option<JoinHandle<()>>,
    thread_id: u32,
}

#[cfg(windows)]
impl WindowsMouseInputRuntime {
    fn create(config: MouseInputConfig) -> Result<Self, String> {
        let (event_tx, event_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let worker = spawn_worker(event_tx, ready_tx, config)?;
        let ready = ready_rx
            .recv()
            .map_err(|_| "mouse raw-input worker exited before becoming ready".to_string())??;

        Ok(Self {
            event_rx,
            pending_events: vec![MouseRuntimeEvent::DevicesChanged(ready.devices)],
            worker: Some(worker),
            thread_id: ready.thread_id,
        })
    }

    fn poll_events(&mut self) -> Result<Vec<MouseRuntimeEvent>, String> {
        let mut events = std::mem::take(&mut self.pending_events);
        loop {
            match self.event_rx.try_recv() {
                Ok(WorkerEvent::DevicesChanged(devices)) => {
                    events.push(MouseRuntimeEvent::DevicesChanged(devices));
                }
                Ok(WorkerEvent::Input { device, event }) => {
                    events.push(MouseRuntimeEvent::Input { device, event });
                }
                Ok(WorkerEvent::Error(error)) => return Err(error),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err("mouse raw-input worker stopped unexpectedly".to_string())
                }
            }
        }

        Ok(events)
    }
}

#[cfg(windows)]
impl Drop for WindowsMouseInputRuntime {
    fn drop(&mut self) {
        stop_worker(self.thread_id);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(all(test, windows))]
pub(crate) fn next_mouse_position_from_raw_input(
    movement_flags: u16,
    raw_x: i32,
    raw_y: i32,
    last_position: (i32, i32),
) -> Option<(i32, i32, i32, i32)> {
    windows_raw_input::next_mouse_position_from_raw_input(
        movement_flags,
        raw_x,
        raw_y,
        last_position,
    )
}

#[cfg(all(test, windows))]
pub(crate) fn should_swallow_captured_mouse_input(flags: u32, capture_window_active: bool) -> bool {
    windows_raw_input::should_swallow_low_level_mouse_event(flags, capture_window_active)
}

#[cfg(all(test, windows))]
pub(crate) fn capture_target_matches_selection(
    selection: &str,
    devices: &[DiscoveredMouseDevice],
    device: &str,
) -> bool {
    windows_raw_input::capture_target_matches_public_selection(selection, devices, device)
}

pub(crate) struct MouseOutputController;

impl MouseOutputController {
    pub(crate) fn create() -> Result<Self, String> {
        Ok(Self)
    }

    pub(crate) fn execute_move(&mut self, request: &MouseMoveRequest) -> Result<String, String> {
        let (x, y) = self.resolve_move(request)?;

        simulate_event(&EventType::MouseMove {
            x: f64::from(x),
            y: f64::from(y),
        })
        .map_err(|error| format!("failed to move the mouse: {error}"))?;

        let summary = match (request.coordinate, request.units) {
            (MouseMoveCoordinate::Absolute, MouseMoveUnits::Pixels) => {
                format!("Moved mouse to ({x}, {y}) pixels")
            }
            (MouseMoveCoordinate::Absolute, MouseMoveUnits::Normalized) => {
                format!("Moved mouse to normalized ({:.3}, {:.3})", request.x, request.y)
            }
            (MouseMoveCoordinate::Relative, MouseMoveUnits::Pixels) => {
                format!("Moved mouse by ({x}, {y}) pixels")
            }
            (MouseMoveCoordinate::Relative, MouseMoveUnits::Normalized) => {
                return Err("normalized mouse movement only supports absolute coordinates".to_string())
            }
        };

        Ok(summary)
    }

    pub(crate) fn execute_button(
        &mut self,
        button: MouseButtonKind,
        action: MouseButtonAction,
    ) -> Result<String, String> {
        let button_name = button.label().to_ascii_lowercase();
        let button = button_to_rdev(button)?;
        match action {
            MouseButtonAction::Click => {
                simulate_event(&EventType::ButtonPress(button))
                    .map_err(|error| format!("failed to press the mouse button: {error}"))?;
                simulate_event(&EventType::ButtonRelease(button))
                    .map_err(|error| format!("failed to release the mouse button: {error}"))?;
            }
            MouseButtonAction::Press => simulate_event(&EventType::ButtonPress(button))
                .map_err(|error| format!("failed to press the mouse button: {error}"))?,
            MouseButtonAction::Release => simulate_event(&EventType::ButtonRelease(button))
                .map_err(|error| format!("failed to release the mouse button: {error}"))?,
        }

        Ok(format!(
            "{} {} mouse button",
            action.label(),
            button_name
        ))
    }

    pub(crate) fn execute_scroll(&mut self, request: &MouseScrollRequest) -> Result<String, String> {
        simulate_event(&EventType::Wheel {
            delta_x: i64::from(request.horizontal),
            delta_y: -i64::from(request.vertical),
        })
        .map_err(|error| format!("failed to scroll the mouse: {error}"))?;

        Ok(format!(
            "Scrolled mouse vertical={} horizontal={}",
            request.vertical, request.horizontal
        ))
    }

    fn resolve_move(&self, request: &MouseMoveRequest) -> Result<(i32, i32), String> {
        match (request.coordinate, request.units) {
            (MouseMoveCoordinate::Absolute, MouseMoveUnits::Pixels) => Ok((
                round_f64_to_i32(request.x, "mouse x")?,
                round_f64_to_i32(request.y, "mouse y")?,
            )),
            (MouseMoveCoordinate::Relative, MouseMoveUnits::Pixels) => {
                let Some(device_state) = DeviceState::checked_new() else {
                    return Err(
                        "relative mouse movement requires a readable current mouse position"
                            .to_string(),
                    );
                };
                let current = device_state.get_mouse();
                Ok((
                    current.coords.0 + round_f64_to_i32(request.x, "mouse dx")?,
                    current.coords.1 + round_f64_to_i32(request.y, "mouse dy")?,
                ))
            }
            (MouseMoveCoordinate::Absolute, MouseMoveUnits::Normalized) => {
                let (width, height) = display_size()
                    .map_err(|error| format!("failed to query the main display size: {error:?}"))?;
                if width == 0 || height == 0 {
                    return Err("the main display reported a non-positive size".to_string());
                }

                let x = clamp_unit(request.x) * (width.saturating_sub(1) as f64);
                let y = clamp_unit(request.y) * (height.saturating_sub(1) as f64);
                Ok((x.round() as i32, y.round() as i32))
            }
            (MouseMoveCoordinate::Relative, MouseMoveUnits::Normalized) => Err(
                "normalized mouse movement only supports absolute coordinates on the main display"
                    .to_string(),
            ),
        }
    }
}

fn clamp_unit(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    value.clamp(0.0, 1.0)
}

fn round_f64_to_i32(value: f64, name: &str) -> Result<i32, String> {
    if !value.is_finite() {
        return Err(format!("{name} must be a finite number"));
    }

    let rounded = value.round();
    if rounded < f64::from(i32::MIN) || rounded > f64::from(i32::MAX) {
        return Err(format!("{name} is out of range for a 32-bit mouse coordinate"));
    }

    Ok(rounded as i32)
}

fn button_to_rdev(button: MouseButtonKind) -> Result<RdevButton, String> {
    match button {
        MouseButtonKind::Left => Ok(RdevButton::Left),
        MouseButtonKind::Middle => Ok(RdevButton::Middle),
        MouseButtonKind::Right => Ok(RdevButton::Right),
    }
}

fn simulate_event(event: &EventType) -> Result<(), SimulateError> {
    simulate(event)
}

trait MouseButtonActionLabel {
    fn label(self) -> &'static str;
}

impl MouseButtonActionLabel for MouseButtonAction {
    fn label(self) -> &'static str {
        match self {
            MouseButtonAction::Click => "Clicked",
            MouseButtonAction::Press => "Pressed",
            MouseButtonAction::Release => "Released",
        }
    }
}
