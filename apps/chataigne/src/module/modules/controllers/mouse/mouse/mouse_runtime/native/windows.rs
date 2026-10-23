use super::*;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::mem::{size_of, MaybeUninit};
use std::ptr::{null, null_mut};
use std::sync::mpsc::{self, SyncSender};
use std::sync::OnceLock;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList, RegisterRawInputDevices, HRAWINPUT, RAWINPUT,
    RAWINPUTDEVICE, RAWINPUTDEVICELIST, RAWINPUTHEADER, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDEV_REMOVE,
    RIDI_DEVICENAME, RID_INPUT, RIM_TYPEMOUSE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW,
    GetSystemMetrics, PostThreadMessageW, RegisterClassW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    HC_ACTION, HHOOK, HWND_MESSAGE, MSG, MSLLHOOKSTRUCT, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WH_MOUSE_LL, WM_INPUT, WM_INPUT_DEVICE_CHANGE, WM_QUIT,
    WNDCLASSW,
};

use super::{DiscoveredMouseDevice, MouseButtonKind, MouseInputConfig, MouseInputEvent};

const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
const HID_USAGE_GENERIC_MOUSE: u16 = 0x02;
const WM_CAPTURE_CHANGED: u32 = 0x8001;
#[cfg(test)]
const AUTO_MOUSE_SELECTION: &str = "auto";
#[cfg(test)]
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
    static CAPTURE_BUS: RefCell<Option<broker::Broadcast>> = const { RefCell::new(None) };
    static CAPTURE_ACTIVE_UNTIL_MS: Cell<u64> = const { Cell::new(0) };
}

static CAPTURE_CLOCK_BASE: OnceLock<Instant> = OnceLock::new();

#[derive(Clone)]
pub(super) struct WorkerReady {
    pub thread_id: u32,
}

#[derive(Clone)]
struct KnownMouseDevice {
    handle: HANDLE,
    public: DiscoveredMouseDevice,
}

pub(super) fn spawn_worker(
    event_tx: broker::Broadcast,
    ready_tx: SyncSender<Result<WorkerReady, String>>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("mouse-raw-input".to_string())
        .spawn(move || {
            if let Err(error) = worker_main(event_tx.clone(), ready_tx) {
                event_tx.fail(error);
            }
        })
        .map_err(|error| format!("failed to spawn the mouse raw-input worker: {error}"))
}

pub(super) fn stop_worker(thread_id: u32) {
    unsafe {
        PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
    }
}

fn worker_main(event_tx: broker::Broadcast, ready_tx: SyncSender<Result<WorkerReady, String>>) -> Result<(), String> {
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

    let mut window = Window {
        hwnd,
        capture_hook: None,
        registered: false,
    };
    let _stopped = broker::Stopped(event_tx.clone());
    let result = pump_input(&mut window, event_tx.clone(), ready_tx, thread_id);
    if let Err(error) = &result {
        event_tx.fail(error.clone());
    }
    result
}
fn pump_input(
    window: &mut Window,
    event_tx: broker::Broadcast,
    ready_tx: SyncSender<Result<WorkerReady, String>>,
    thread_id: u32,
) -> Result<(), String> {
    let hwnd = window.hwnd;
    register_mouse_input(hwnd)?;
    window.registered = true;
    event_tx.notify_changes(WorkerNotifier::new(move || unsafe {
        PostThreadMessageW(thread_id, WM_CAPTURE_CHANGED, 0, 0);
    }));
    window.sync_capture_hook(&event_tx)?;
    CAPTURE_BUS.with(|slot| *slot.borrow_mut() = Some(event_tx.clone()));
    clear_capture_window();
    let mut known_devices = enumerate_mice()?;
    let mut device_lookup = build_device_lookup(known_devices.as_slice());
    let mut last_pointer_position = current_cursor_position().unwrap_or((0, 0));

    event_tx.send(MouseRuntimeEvent::DevicesChanged(public_devices(&known_devices)))?;
    ready_tx
        .send(Ok(WorkerReady { thread_id }))
        .map_err(|_| "mouse input owner retired during startup")?;

    let mut message = MaybeUninit::<MSG>::zeroed();
    loop {
        let status = unsafe { GetMessageW(message.as_mut_ptr(), null_mut(), 0, 0) };
        if status == -1 {
            return Err("mouse raw-input message pump failed".to_string());
        }
        if status == 0 {
            break;
        }

        let message = unsafe { message.assume_init() };
        match message.message {
            WM_CAPTURE_CHANGED => window.sync_capture_hook(&event_tx)?,
            WM_INPUT => {
                let input = read_mouse_input(message.lParam, &device_lookup, &mut last_pointer_position);
                unsafe {
                    DefWindowProcW(message.hwnd, message.message, message.wParam, message.lParam);
                }
                for (device, event) in input? {
                    if event_tx.capture_target(&public_devices(&known_devices), &device) {
                        arm_capture_window();
                    }
                    event_tx.send(MouseRuntimeEvent::Input { device, event })?;
                }
            }
            WM_INPUT_DEVICE_CHANGE => {
                known_devices = enumerate_mice()?;
                device_lookup = build_device_lookup(known_devices.as_slice());
                if !event_tx.has_capture_selection(&public_devices(&known_devices)) {
                    clear_capture_window();
                }
                event_tx.send(MouseRuntimeEvent::DevicesChanged(public_devices(&known_devices)))?;
            }
            _ => unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            },
        }
    }

    Ok(())
}

struct Window {
    hwnd: HWND,
    capture_hook: Option<HHOOK>,
    registered: bool,
}
impl Drop for Window {
    fn drop(&mut self) {
        clear_capture_window();
        CAPTURE_BUS.with(|slot| slot.borrow_mut().take());
        if let Some(hook) = self.capture_hook {
            unsafe {
                UnhookWindowsHookEx(hook);
            }
        }
        if self.registered {
            let device = RAWINPUTDEVICE {
                usUsagePage: HID_USAGE_PAGE_GENERIC,
                usUsage: HID_USAGE_GENERIC_MOUSE,
                dwFlags: RIDEV_REMOVE,
                hwndTarget: null_mut(),
            };
            unsafe {
                RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as u32);
            }
        }
        unsafe {
            DestroyWindow(self.hwnd);
        }
    }
}
impl Window {
    fn sync_capture_hook(&mut self, bus: &broker::Broadcast) -> Result<(), String> {
        match (bus.capture_enabled(), self.capture_hook) {
            (true, None) => {
                self.capture_hook = Some(install_mouse_capture_hook(unsafe { GetModuleHandleW(null()) })?);
            }
            (false, Some(hook)) => {
                clear_capture_window();
                unsafe {
                    UnhookWindowsHookEx(hook);
                }
                self.capture_hook = None;
            }
            _ => {}
        }
        Ok(())
    }
}

fn register_mouse_input(hwnd: HWND) -> Result<(), String> {
    let device = RAWINPUTDEVICE {
        usUsagePage: HID_USAGE_PAGE_GENERIC,
        usUsage: HID_USAGE_GENERIC_MOUSE,
        dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
        hwndTarget: hwnd,
    };
    let success = unsafe { RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as u32) };
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

pub(crate) fn should_swallow_low_level_mouse_event(flags: u32, capture_window_active: bool) -> bool {
    capture_window_active && flags & LOW_LEVEL_MOUSE_INJECTED_FLAG == 0
}

unsafe extern "system" fn low_level_mouse_capture_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && lparam != 0 {
        let hook = unsafe { &*(lparam as *const MSLLHOOKSTRUCT) };
        if should_swallow_low_level_mouse_event(hook.flags, capture_window_active()) {
            return 1;
        }
    }

    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

fn capture_window_active() -> bool {
    if !CAPTURE_BUS.with(|slot| slot.borrow().as_ref().is_some_and(broker::Broadcast::capture_enabled)) {
        return false;
    }
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

#[cfg(test)]
pub(crate) fn capture_target_matches_public_selection(
    selection: &str,
    devices: &[DiscoveredMouseDevice],
    device: &str,
) -> bool {
    match selection.trim() {
        AUTO_MOUSE_SELECTION => devices.first().is_some_and(|selected| selected.variant_id == device),
        NO_MOUSE_SELECTION | "" => false,
        selected => {
            selected == device
                || super::super::super::legacy_mouse_variant_identity(selected)
                    .is_some_and(|identity| identity == device)
        }
    }
}

fn enumerate_mice() -> Result<Vec<KnownMouseDevice>, String> {
    let mut device_count = 0u32;
    let query_status =
        unsafe { GetRawInputDeviceList(null_mut(), &mut device_count, size_of::<RAWINPUTDEVICELIST>() as u32) };
    if query_status == u32::MAX {
        return Err("failed to enumerate raw-input devices".to_string());
    }
    if device_count == 0 {
        return Ok(Vec::new());
    }

    if device_count > 4096 {
        return Err("mouse raw device inventory exceeds 4096 entries".into());
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
    if populated as usize > entries.len() {
        return Err("mouse device inventory changed beyond admitted storage".into());
    }
    entries.truncate(populated as usize);

    let mut mice = Vec::new();
    for entry in entries {
        if entry.dwType != RIM_TYPEMOUSE {
            continue;
        }

        if mice.len() >= MAX_DEVICES {
            return Err("mouse catalog exceeds 64 devices".into());
        }
        let path = raw_input_device_name(entry.hDevice)?;
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

    validate(&MouseRuntimeEvent::DevicesChanged(public_devices(&mice)))?;
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
    let status = unsafe { GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, null_mut(), &mut length) };
    if status == u32::MAX {
        return Err("failed to query the raw-input mouse name length".to_string());
    }
    if length == 0 {
        return Ok(String::new());
    }

    if length > 1024 {
        return Err("mouse native name exceeds 1024 UTF-16 units".into());
    }
    let mut buffer = vec![0u16; length as usize];
    let status = unsafe { GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, buffer.as_mut_ptr().cast(), &mut length) };
    if status == u32::MAX || length as usize > buffer.len() {
        return Err("failed to read the raw-input mouse name".to_string());
    }
    buffer.truncate(length as usize);

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

    if size as usize > size_of::<RAWINPUT>() || (size as usize) < size_of::<RAWINPUTHEADER>() {
        return Err("mouse raw packet exceeds fixed storage".into());
    }
    let mut buffer = MaybeUninit::<RAWINPUT>::zeroed();
    let status = unsafe {
        GetRawInputData(
            raw_input_lparam as HRAWINPUT,
            RID_INPUT,
            buffer.as_mut_ptr().cast(),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    if status == u32::MAX || (status as usize) < size_of::<RAWINPUTHEADER>() || status as usize > size_of::<RAWINPUT>()
    {
        return Err("failed to read the raw mouse input packet".to_string());
    }
    let raw = unsafe { buffer.assume_init() };
    if raw.header.dwType != RIM_TYPEMOUSE {
        return Ok(Vec::new());
    }
    if (status as usize) < size_of::<RAWINPUTHEADER>() + size_of::<windows_sys::Win32::UI::Input::RAWMOUSE>() {
        return Err("truncated mouse input packet".into());
    }

    let device = device_lookup
        .get(&raw.header.hDevice)
        .map(|device| device.variant_id.clone())
        .unwrap_or_else(|| fallback_mouse_variant_id(raw.header.hDevice));
    let mut events = Vec::new();
    let mouse = unsafe { raw.data.mouse };
    if let Some((x, y, dx, dy)) =
        next_mouse_position_from_raw_input(mouse.usFlags, mouse.lLastX, mouse.lLastY, *last_pointer_position)
    {
        events.push((device.clone(), MouseInputEvent::Moved { x, y, dx, dy }));
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

pub(crate) fn next_mouse_position_from_raw_input(
    movement_flags: u16,
    raw_x: i32,
    raw_y: i32,
    last_position: (i32, i32),
) -> Option<(i32, i32, i32, i32)> {
    if movement_flags & MOUSE_MOVE_ABSOLUTE != 0 {
        let (left, top, width, height) = desktop_bounds(movement_flags);
        let x = normalize_absolute_axis(raw_x, left, width);
        let y = normalize_absolute_axis(raw_y, top, height);
        let dx = x.saturating_sub(last_position.0);
        let dy = y.saturating_sub(last_position.1);
        return (dx != 0 || dy != 0).then_some((x, y, dx, dy));
    }

    (raw_x != 0 || raw_y != 0).then_some((
        last_position.0.saturating_add(raw_x),
        last_position.1.saturating_add(raw_y),
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
            MouseInputEvent::ButtonChanged { button, pressed: true },
        ));
    }
    if button_flags & up_flag != 0 {
        events.push((
            device.to_string(),
            MouseInputEvent::ButtonChanged { button, pressed: false },
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
    let vendor = extract_four_hex_after(path, "VID_").or_else(|| extract_four_hex_after(path, "VID&"))?;
    let product = extract_four_hex_after(path, "PID_").or_else(|| extract_four_hex_after(path, "PID&"))?;
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
struct Endpoint {
    worker: Option<JoinHandle<()>>,
    thread_id: u32,
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        stop_worker(self.thread_id);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn registry() -> &'static broker::Registry<Endpoint> {
    static REGISTRY: OnceLock<broker::Registry<Endpoint>> = OnceLock::new();
    REGISTRY.get_or_init(broker::Registry::new)
}
pub(super) fn create(config: MouseInputConfig, guard: RevisionGuard) -> Result<impl Provider, String> {
    registry().subscribe_guarded(config, guard, |bus| {
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let worker = spawn_worker(bus, ready_tx)?;
        match ready_rx.recv() {
            Ok(Ok(ready)) => Ok(Endpoint {
                worker: Some(worker),
                thread_id: ready.thread_id,
            }),
            result => {
                let _ = worker.join();
                Err(match result {
                    Ok(Err(error)) => error,
                    _ => "mouse worker exited before readiness".into(),
                })
            }
        }
    })
}
