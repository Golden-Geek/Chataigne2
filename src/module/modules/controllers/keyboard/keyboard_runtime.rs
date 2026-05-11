#[cfg(not(windows))]
use std::collections::BTreeSet;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::JoinHandle;
#[cfg(not(windows))]
use std::time::Duration;
#[cfg(not(windows))]
use std::{sync::mpsc::Sender, thread};

#[cfg(not(windows))]
use device_query::{DeviceQuery, DeviceState};
use rdev::{simulate, EventType, SimulateError};

use crate::app::module::common::keyboard::{KeyboardKey, KeyboardKeyAction};

#[cfg(not(windows))]
const GLOBAL_KEYBOARD_VARIANT: &str = "system|System Keyboard";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredKeyboardDevice {
    pub index: usize,
    pub variant_id: String,
    pub label: String,
    pub details: String,
}

#[cfg(not(windows))]
#[derive(Clone, Debug, Default, PartialEq)]
struct KeyboardObservedState {
    pressed_keys: BTreeSet<KeyboardKey>,
}

#[cfg(not(windows))]
impl KeyboardObservedState {
    fn from_device_state(device_state: &DeviceState) -> Self {
        let pressed_keys = device_state
            .get_keys()
            .into_iter()
            .filter_map(KeyboardKey::from_device_query)
            .collect();
        Self { pressed_keys }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum KeyboardInputEvent {
    KeyChanged { key: KeyboardKey, pressed: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum KeyboardRuntimeEvent {
    DevicesChanged(Vec<DiscoveredKeyboardDevice>),
    Input {
        device: String,
        event: KeyboardInputEvent,
    },
}

#[cfg(not(windows))]
fn diff_keyboard_states(
    previous: &KeyboardObservedState,
    next: &KeyboardObservedState,
) -> Vec<KeyboardInputEvent> {
    let mut events = Vec::new();

    for key in next.pressed_keys.difference(&previous.pressed_keys) {
        events.push(KeyboardInputEvent::KeyChanged {
            key: *key,
            pressed: true,
        });
    }
    for key in previous.pressed_keys.difference(&next.pressed_keys) {
        events.push(KeyboardInputEvent::KeyChanged {
            key: *key,
            pressed: false,
        });
    }

    events
}

pub(crate) struct KeyboardInputRuntime {
    inner: KeyboardInputRuntimeInner,
}

impl KeyboardInputRuntime {
    pub(crate) fn create() -> Result<Self, String> {
        #[cfg(windows)]
        {
            return WindowsKeyboardInputRuntime::create().map(|runtime| Self {
                inner: KeyboardInputRuntimeInner::Windows(runtime),
            });
        }

        #[cfg(not(windows))]
        {
            let runtime = GlobalKeyboardInputRuntime::create()?;
            Ok(Self {
                inner: KeyboardInputRuntimeInner::Global(runtime),
            })
        }
    }

    pub(crate) fn poll_events(&mut self) -> Result<Vec<KeyboardRuntimeEvent>, String> {
        match &mut self.inner {
            #[cfg(windows)]
            KeyboardInputRuntimeInner::Windows(runtime) => runtime.poll_events(),
            #[cfg(not(windows))]
            KeyboardInputRuntimeInner::Global(runtime) => runtime.poll_events(),
        }
    }
}

enum KeyboardInputRuntimeInner {
    #[cfg(windows)]
    Windows(WindowsKeyboardInputRuntime),
    #[cfg(not(windows))]
    Global(GlobalKeyboardInputRuntime),
}

#[cfg(not(windows))]
#[derive(Clone)]
struct GlobalWorkerReady {
    devices: Vec<DiscoveredKeyboardDevice>,
}

#[cfg(not(windows))]
#[derive(Clone, Debug)]
enum GlobalWorkerEvent {
    Input {
        device: String,
        event: KeyboardInputEvent,
    },
    Error(String),
}

#[cfg(not(windows))]
struct GlobalKeyboardInputRuntime {
    event_rx: Receiver<GlobalWorkerEvent>,
    pending_events: Vec<KeyboardRuntimeEvent>,
    shutdown_tx: Sender<()>,
    worker: Option<JoinHandle<()>>,
}

#[cfg(not(windows))]
impl GlobalKeyboardInputRuntime {
    fn create() -> Result<Self, String> {
        let (event_tx, event_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let worker = spawn_global_worker(event_tx, ready_tx, shutdown_rx)?;
        let ready = ready_rx
            .recv()
            .map_err(|_| "keyboard global-input worker exited before becoming ready".to_string())??;

        Ok(Self {
            event_rx,
            pending_events: vec![KeyboardRuntimeEvent::DevicesChanged(ready.devices)],
            shutdown_tx,
            worker: Some(worker),
        })
    }

    fn poll_events(&mut self) -> Result<Vec<KeyboardRuntimeEvent>, String> {
        let mut events = std::mem::take(&mut self.pending_events);
        loop {
            match self.event_rx.try_recv() {
                Ok(GlobalWorkerEvent::Input { device, event }) => {
                    events.push(KeyboardRuntimeEvent::Input { device, event });
                }
                Ok(GlobalWorkerEvent::Error(error)) => return Err(error),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err("keyboard global-input worker stopped unexpectedly".to_string());
                }
            }
        }

        Ok(events)
    }
}

#[cfg(not(windows))]
impl Drop for GlobalKeyboardInputRuntime {
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
        .name("keyboard-global-input".to_string())
        .spawn(move || {
            if let Err(error) = global_worker_main(event_tx.clone(), ready_tx, shutdown_rx) {
                let _ = event_tx.send(GlobalWorkerEvent::Error(error));
            }
        })
        .map_err(|error| format!("failed to spawn the keyboard global-input worker: {error}"))
}

#[cfg(not(windows))]
fn global_worker_main(
    event_tx: Sender<GlobalWorkerEvent>,
    ready_tx: mpsc::SyncSender<Result<GlobalWorkerReady, String>>,
    shutdown_rx: Receiver<()>,
) -> Result<(), String> {
    let Some(device_state) = DeviceState::checked_new() else {
        let error = "failed to access the local keyboard input backend".to_string();
        let _ = ready_tx.send(Err(error.clone()));
        return Err(error);
    };

    if ready_tx
        .send(Ok(GlobalWorkerReady {
            devices: vec![global_keyboard_device()],
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

        let next_state = KeyboardObservedState::from_device_state(&device_state);
        if let Some(previous_state) = last_state.as_ref() {
            for event in diff_keyboard_states(previous_state, &next_state) {
                if event_tx
                    .send(GlobalWorkerEvent::Input {
                        device: GLOBAL_KEYBOARD_VARIANT.to_string(),
                        event,
                    })
                    .is_err()
                {
                    return Ok(());
                }
            }
        } else {
            for key in next_state.pressed_keys.iter().copied() {
                if event_tx
                    .send(GlobalWorkerEvent::Input {
                        device: GLOBAL_KEYBOARD_VARIANT.to_string(),
                        event: KeyboardInputEvent::KeyChanged { key, pressed: true },
                    })
                    .is_err()
                {
                    return Ok(());
                }
            }
        }
        last_state = Some(next_state);

        thread::sleep(Duration::from_millis(8));
    }

    Ok(())
}

#[cfg(not(windows))]
fn global_keyboard_device() -> DiscoveredKeyboardDevice {
    DiscoveredKeyboardDevice {
        index: 0,
        variant_id: GLOBAL_KEYBOARD_VARIANT.to_string(),
        label: "System Keyboard".to_string(),
        details: "Global system keyboard input".to_string(),
    }
}

#[cfg(windows)]
mod windows_raw_input {
    use std::collections::HashMap;
    use std::mem::{size_of, MaybeUninit};
    use std::ptr::{null, null_mut};
    use std::sync::mpsc::{self, SyncSender};
    use std::thread::{self, JoinHandle};

    use windows_sys::Win32::Foundation::{HANDLE, HWND, LPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::{
        GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList, HRAWINPUT, RAWINPUT,
        RAWINPUTDEVICE, RAWINPUTDEVICELIST, RAWINPUTHEADER, RegisterRawInputDevices,
        RIDI_DEVICENAME, RID_INPUT, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIM_TYPEKEYBOARD,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
        PostThreadMessageW, RegisterClassW, TranslateMessage, HWND_MESSAGE, MSG, WM_INPUT,
        WM_INPUT_DEVICE_CHANGE, WM_QUIT, WNDCLASSW,
    };

    use super::{DiscoveredKeyboardDevice, KeyboardInputEvent};
    use crate::app::module::common::keyboard::KeyboardKey;

    const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
    const HID_USAGE_GENERIC_KEYBOARD: u16 = 0x06;
    const RI_KEY_BREAK: u16 = 0x0001;
    const RI_KEY_E0: u16 = 0x0002;

    const VK_BACK: u16 = 0x08;
    const VK_TAB: u16 = 0x09;
    const VK_RETURN: u16 = 0x0D;
    const VK_SHIFT: u16 = 0x10;
    const VK_CONTROL: u16 = 0x11;
    const VK_MENU: u16 = 0x12;
    const VK_PAUSE: u16 = 0x13;
    const VK_CAPITAL: u16 = 0x14;
    const VK_ESCAPE: u16 = 0x1B;
    const VK_SPACE: u16 = 0x20;
    const VK_PRIOR: u16 = 0x21;
    const VK_NEXT: u16 = 0x22;
    const VK_END: u16 = 0x23;
    const VK_HOME: u16 = 0x24;
    const VK_LEFT: u16 = 0x25;
    const VK_UP: u16 = 0x26;
    const VK_RIGHT: u16 = 0x27;
    const VK_DOWN: u16 = 0x28;
    const VK_INSERT: u16 = 0x2D;
    const VK_DELETE: u16 = 0x2E;
    const VK_0: u16 = 0x30;
    const VK_9: u16 = 0x39;
    const VK_A: u16 = 0x41;
    const VK_Z: u16 = 0x5A;
    const VK_LWIN: u16 = 0x5B;
    const VK_RWIN: u16 = 0x5C;
    const VK_NUMPAD0: u16 = 0x60;
    const VK_NUMPAD9: u16 = 0x69;
    const VK_MULTIPLY: u16 = 0x6A;
    const VK_ADD: u16 = 0x6B;
    const VK_SUBTRACT: u16 = 0x6D;
    const VK_DECIMAL: u16 = 0x6E;
    const VK_DIVIDE: u16 = 0x6F;
    const VK_F1: u16 = 0x70;
    const VK_F12: u16 = 0x7B;
    const VK_NUMLOCK: u16 = 0x90;
    const VK_SCROLL: u16 = 0x91;
    const VK_LSHIFT: u16 = 0xA0;
    const VK_RSHIFT: u16 = 0xA1;
    const VK_LCONTROL: u16 = 0xA2;
    const VK_RCONTROL: u16 = 0xA3;
    const VK_LMENU: u16 = 0xA4;
    const VK_RMENU: u16 = 0xA5;
    const VK_OEM_1: u16 = 0xBA;
    const VK_OEM_PLUS: u16 = 0xBB;
    const VK_OEM_COMMA: u16 = 0xBC;
    const VK_OEM_MINUS: u16 = 0xBD;
    const VK_OEM_PERIOD: u16 = 0xBE;
    const VK_OEM_2: u16 = 0xBF;
    const VK_OEM_3: u16 = 0xC0;
    const VK_OEM_4: u16 = 0xDB;
    const VK_OEM_5: u16 = 0xDC;
    const VK_OEM_6: u16 = 0xDD;
    const VK_OEM_7: u16 = 0xDE;
    const VK_OEM_102: u16 = 0xE2;
    const VK_SNAPSHOT: u16 = 0x2C;

    #[derive(Clone)]
    pub(super) struct WorkerReady {
        pub thread_id: u32,
        pub devices: Vec<DiscoveredKeyboardDevice>,
    }

    #[derive(Clone, Debug)]
    pub(super) enum WorkerEvent {
        DevicesChanged(Vec<DiscoveredKeyboardDevice>),
        Input {
            device: String,
            event: KeyboardInputEvent,
        },
        Error(String),
    }

    #[derive(Clone)]
    struct KnownKeyboardDevice {
        handle: HANDLE,
        public: DiscoveredKeyboardDevice,
    }

    pub(super) fn spawn_worker(
        event_tx: mpsc::Sender<WorkerEvent>,
        ready_tx: SyncSender<Result<WorkerReady, String>>,
    ) -> Result<JoinHandle<()>, String> {
        thread::Builder::new()
            .name("keyboard-raw-input".to_string())
            .spawn(move || {
                if let Err(error) = worker_main(event_tx.clone(), ready_tx) {
                    let _ = event_tx.send(WorkerEvent::Error(error));
                }
            })
            .map_err(|error| format!("failed to spawn the keyboard raw-input worker: {error}"))
    }

    pub(super) fn stop_worker(thread_id: u32) {
        unsafe {
            PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
        }
    }

    fn worker_main(
        event_tx: mpsc::Sender<WorkerEvent>,
        ready_tx: SyncSender<Result<WorkerReady, String>>,
    ) -> Result<(), String> {
        let thread_id = unsafe { GetCurrentThreadId() };
        let class_name = wide_null("Chataigne2KeyboardRawInputWindow");
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
            let error = "failed to create the hidden keyboard raw-input window".to_string();
            let _ = ready_tx.send(Err(error.clone()));
            return Err(error);
        }

        if let Err(error) = register_keyboard_input(hwnd) {
            let _ = ready_tx.send(Err(error.clone()));
            unsafe {
                DestroyWindow(hwnd);
            }
            return Err(error);
        }

        let mut known_devices = enumerate_keyboards()?;
        let mut device_lookup = build_device_lookup(known_devices.as_slice());

        let _ = ready_tx.send(Ok(WorkerReady {
            thread_id,
            devices: public_devices(known_devices.as_slice()),
        }));

        let mut message = MaybeUninit::<MSG>::zeroed();
        loop {
            let status = unsafe { GetMessageW(message.as_mut_ptr(), null_mut(), 0, 0) };
            if status == -1 {
                unsafe {
                    DestroyWindow(hwnd);
                }
                return Err("keyboard raw-input message pump failed".to_string());
            }
            if status == 0 {
                break;
            }

            let message = unsafe { message.assume_init() };
            match message.message {
                WM_INPUT => {
                    if let Some((device, event)) = read_keyboard_input(message.lParam, &device_lookup)? {
                        let _ = event_tx.send(WorkerEvent::Input { device, event });
                    }
                }
                WM_INPUT_DEVICE_CHANGE => {
                    known_devices = enumerate_keyboards()?;
                    device_lookup = build_device_lookup(known_devices.as_slice());
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

        unsafe {
            DestroyWindow(hwnd);
        }
        Ok(())
    }

    fn register_keyboard_input(hwnd: HWND) -> Result<(), String> {
        let device = RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_KEYBOARD,
            dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
            hwndTarget: hwnd,
        };
        let success = unsafe { RegisterRawInputDevices(&device, 1, size_of::<RAWINPUTDEVICE>() as u32) };
        if success == 0 {
            return Err("failed to register the keyboard raw-input device sink".to_string());
        }

        Ok(())
    }

    fn enumerate_keyboards() -> Result<Vec<KnownKeyboardDevice>, String> {
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

        let mut keyboards = Vec::new();
        for entry in entries {
            if entry.dwType != RIM_TYPEKEYBOARD {
                continue;
            }

            let path = raw_input_device_name(entry.hDevice).unwrap_or_default();
            let label = describe_keyboard_label(path.as_str(), entry.hDevice);
            let details = describe_keyboard_details(path.as_str());
            keyboards.push(KnownKeyboardDevice {
                handle: entry.hDevice,
                public: DiscoveredKeyboardDevice {
                    index: keyboards.len(),
                    variant_id: keyboard_variant_id(path.as_str(), entry.hDevice),
                    label,
                    details,
                },
            });
        }

        Ok(keyboards)
    }

    fn build_device_lookup(
        devices: &[KnownKeyboardDevice],
    ) -> HashMap<HANDLE, DiscoveredKeyboardDevice> {
        devices
            .iter()
            .map(|device| (device.handle, device.public.clone()))
            .collect()
    }

    fn public_devices(devices: &[KnownKeyboardDevice]) -> Vec<DiscoveredKeyboardDevice> {
        devices.iter().map(|device| device.public.clone()).collect()
    }

    fn raw_input_device_name(handle: HANDLE) -> Result<String, String> {
        let mut length = 0u32;
        let status = unsafe { GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, null_mut(), &mut length) };
        if status == u32::MAX {
            return Err("failed to query the raw-input keyboard name length".to_string());
        }
        if length == 0 {
            return Ok(String::new());
        }

        let mut buffer = vec![0u16; length as usize];
        let status = unsafe {
            GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, buffer.as_mut_ptr().cast(), &mut length)
        };
        if status == u32::MAX {
            return Err("failed to read the raw-input keyboard name".to_string());
        }

        if buffer.last().is_some_and(|last| *last == 0) {
            buffer.pop();
        }
        Ok(String::from_utf16_lossy(buffer.as_slice()))
    }

    fn read_keyboard_input(
        raw_input_lparam: LPARAM,
        device_lookup: &HashMap<HANDLE, DiscoveredKeyboardDevice>,
    ) -> Result<Option<(String, KeyboardInputEvent)>, String> {
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
            return Err("failed to measure the raw keyboard input packet".to_string());
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
            return Err("failed to read the raw keyboard input packet".to_string());
        }

        let raw = unsafe { &*(buffer.as_ptr().cast::<RAWINPUT>()) };
        if raw.header.dwType != RIM_TYPEKEYBOARD {
            return Ok(None);
        }

        let keyboard = unsafe { raw.data.keyboard };
        let Some(key) = map_raw_keyboard_key(keyboard.VKey, keyboard.MakeCode, keyboard.Flags) else {
            return Ok(None);
        };
        let pressed = keyboard.Flags & RI_KEY_BREAK == 0;
        let device = device_lookup
            .get(&raw.header.hDevice)
            .map(|device| device.variant_id.clone())
            .unwrap_or_else(|| fallback_keyboard_variant_id(raw.header.hDevice));

        Ok(Some((
            device,
            KeyboardInputEvent::KeyChanged { key, pressed },
        )))
    }

    fn map_raw_keyboard_key(vkey: u16, make_code: u16, flags: u16) -> Option<KeyboardKey> {
        if vkey == 0 || vkey == 0xFF {
            return None;
        }

        let extended = flags & RI_KEY_E0 != 0;
        match vkey {
            VK_0..=VK_9 => Some(match vkey {
                0x30 => KeyboardKey::Digit0,
                0x31 => KeyboardKey::Digit1,
                0x32 => KeyboardKey::Digit2,
                0x33 => KeyboardKey::Digit3,
                0x34 => KeyboardKey::Digit4,
                0x35 => KeyboardKey::Digit5,
                0x36 => KeyboardKey::Digit6,
                0x37 => KeyboardKey::Digit7,
                0x38 => KeyboardKey::Digit8,
                _ => KeyboardKey::Digit9,
            }),
            VK_A..=VK_Z => Some(match vkey {
                0x41 => KeyboardKey::A,
                0x42 => KeyboardKey::B,
                0x43 => KeyboardKey::C,
                0x44 => KeyboardKey::D,
                0x45 => KeyboardKey::E,
                0x46 => KeyboardKey::F,
                0x47 => KeyboardKey::G,
                0x48 => KeyboardKey::H,
                0x49 => KeyboardKey::I,
                0x4A => KeyboardKey::J,
                0x4B => KeyboardKey::K,
                0x4C => KeyboardKey::L,
                0x4D => KeyboardKey::M,
                0x4E => KeyboardKey::N,
                0x4F => KeyboardKey::O,
                0x50 => KeyboardKey::P,
                0x51 => KeyboardKey::Q,
                0x52 => KeyboardKey::R,
                0x53 => KeyboardKey::S,
                0x54 => KeyboardKey::T,
                0x55 => KeyboardKey::U,
                0x56 => KeyboardKey::V,
                0x57 => KeyboardKey::W,
                0x58 => KeyboardKey::X,
                0x59 => KeyboardKey::Y,
                _ => KeyboardKey::Z,
            }),
            VK_F1..=VK_F12 => Some(match vkey {
                0x70 => KeyboardKey::F1,
                0x71 => KeyboardKey::F2,
                0x72 => KeyboardKey::F3,
                0x73 => KeyboardKey::F4,
                0x74 => KeyboardKey::F5,
                0x75 => KeyboardKey::F6,
                0x76 => KeyboardKey::F7,
                0x77 => KeyboardKey::F8,
                0x78 => KeyboardKey::F9,
                0x79 => KeyboardKey::F10,
                0x7A => KeyboardKey::F11,
                _ => KeyboardKey::F12,
            }),
            VK_ESCAPE => Some(KeyboardKey::Escape),
            VK_SPACE => Some(KeyboardKey::Space),
            VK_RETURN => Some(if extended {
                KeyboardKey::NumpadEnter
            } else {
                KeyboardKey::Enter
            }),
            VK_TAB => Some(KeyboardKey::Tab),
            VK_BACK => Some(KeyboardKey::Backspace),
            VK_UP => Some(if extended { KeyboardKey::Up } else { KeyboardKey::Numpad8 }),
            VK_DOWN => Some(if extended { KeyboardKey::Down } else { KeyboardKey::Numpad2 }),
            VK_LEFT => Some(if extended { KeyboardKey::Left } else { KeyboardKey::Numpad4 }),
            VK_RIGHT => Some(if extended { KeyboardKey::Right } else { KeyboardKey::Numpad6 }),
            VK_HOME => Some(if extended { KeyboardKey::Home } else { KeyboardKey::Numpad7 }),
            VK_END => Some(if extended { KeyboardKey::End } else { KeyboardKey::Numpad1 }),
            VK_PRIOR => Some(if extended { KeyboardKey::PageUp } else { KeyboardKey::Numpad9 }),
            VK_NEXT => Some(if extended { KeyboardKey::PageDown } else { KeyboardKey::Numpad3 }),
            VK_INSERT => Some(if extended { KeyboardKey::Insert } else { KeyboardKey::Numpad0 }),
            VK_DELETE => Some(if extended {
                KeyboardKey::Delete
            } else {
                KeyboardKey::NumpadDecimal
            }),
            VK_SHIFT => Some(if make_code == 0x36 {
                KeyboardKey::RightShift
            } else {
                KeyboardKey::LeftShift
            }),
            VK_LSHIFT => Some(KeyboardKey::LeftShift),
            VK_RSHIFT => Some(KeyboardKey::RightShift),
            VK_CONTROL => Some(if extended {
                KeyboardKey::RightControl
            } else {
                KeyboardKey::LeftControl
            }),
            VK_LCONTROL => Some(KeyboardKey::LeftControl),
            VK_RCONTROL => Some(KeyboardKey::RightControl),
            VK_MENU => Some(if extended {
                KeyboardKey::RightAlt
            } else {
                KeyboardKey::LeftAlt
            }),
            VK_LMENU => Some(KeyboardKey::LeftAlt),
            VK_RMENU => Some(KeyboardKey::RightAlt),
            VK_LWIN => Some(KeyboardKey::LeftMeta),
            VK_RWIN => Some(KeyboardKey::RightMeta),
            VK_CAPITAL => Some(KeyboardKey::CapsLock),
            VK_NUMPAD0..=VK_NUMPAD9 => Some(match vkey {
                0x60 => KeyboardKey::Numpad0,
                0x61 => KeyboardKey::Numpad1,
                0x62 => KeyboardKey::Numpad2,
                0x63 => KeyboardKey::Numpad3,
                0x64 => KeyboardKey::Numpad4,
                0x65 => KeyboardKey::Numpad5,
                0x66 => KeyboardKey::Numpad6,
                0x67 => KeyboardKey::Numpad7,
                0x68 => KeyboardKey::Numpad8,
                _ => KeyboardKey::Numpad9,
            }),
            VK_ADD => Some(KeyboardKey::NumpadAdd),
            VK_SUBTRACT => Some(KeyboardKey::NumpadSubtract),
            VK_MULTIPLY => Some(KeyboardKey::NumpadMultiply),
            VK_DIVIDE => Some(KeyboardKey::NumpadDivide),
            VK_DECIMAL => Some(KeyboardKey::NumpadDecimal),
            VK_OEM_3 => Some(KeyboardKey::Grave),
            VK_OEM_MINUS => Some(KeyboardKey::Minus),
            VK_OEM_PLUS => Some(KeyboardKey::Equal),
            VK_OEM_4 => Some(KeyboardKey::LeftBracket),
            VK_OEM_6 => Some(KeyboardKey::RightBracket),
            VK_OEM_5 | VK_OEM_102 => Some(KeyboardKey::BackSlash),
            VK_OEM_1 => Some(KeyboardKey::Semicolon),
            VK_OEM_7 => Some(KeyboardKey::Apostrophe),
            VK_OEM_COMMA => Some(KeyboardKey::Comma),
            VK_OEM_PERIOD => Some(KeyboardKey::Dot),
            VK_OEM_2 => Some(KeyboardKey::Slash),
            VK_NUMLOCK => None,
            VK_SCROLL | VK_PAUSE | VK_SNAPSHOT => None,
            _ => None,
        }
    }

    fn keyboard_variant_id(path: &str, handle: HANDLE) -> String {
        keyboard_identity(path, handle)
    }

    fn fallback_keyboard_variant_id(handle: HANDLE) -> String {
        keyboard_identity("", handle)
    }

    fn keyboard_identity(path: &str, handle: HANDLE) -> String {
        if !path.trim().is_empty() {
            return path.to_string();
        }

        format!("raw:{:016x}", handle as usize)
    }

    fn describe_keyboard_label(path: &str, handle: HANDLE) -> String {
        let upper = path.to_ascii_uppercase();
        if upper.contains("RDP_KBD") {
            return "Remote Desktop Keyboard".to_string();
        }
        if let Some((vendor, product)) = parse_vid_pid(upper.as_str()) {
            return format!("Keyboard VID {vendor} PID {product}");
        }
        if let Some(identity) = stable_keyboard_identity_label(path) {
            return format!("Keyboard {identity}");
        }

        format!("Keyboard {:04X}", (handle as usize) & 0xffff)
    }

    fn describe_keyboard_details(path: &str) -> String {
        if path.trim().is_empty() {
            "Raw Input keyboard".to_string()
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

    fn stable_keyboard_identity_label(path: &str) -> Option<String> {
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
struct WindowsKeyboardInputRuntime {
    event_rx: Receiver<WorkerEvent>,
    pending_events: Vec<KeyboardRuntimeEvent>,
    worker: Option<JoinHandle<()>>,
    thread_id: u32,
}

#[cfg(windows)]
impl WindowsKeyboardInputRuntime {
    fn create() -> Result<Self, String> {
        let (event_tx, event_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let worker = spawn_worker(event_tx, ready_tx)?;
        let ready = ready_rx
            .recv()
            .map_err(|_| "keyboard raw-input worker exited before becoming ready".to_string())??;

        Ok(Self {
            event_rx,
            pending_events: vec![KeyboardRuntimeEvent::DevicesChanged(ready.devices)],
            worker: Some(worker),
            thread_id: ready.thread_id,
        })
    }

    fn poll_events(&mut self) -> Result<Vec<KeyboardRuntimeEvent>, String> {
        let mut events = std::mem::take(&mut self.pending_events);
        loop {
            match self.event_rx.try_recv() {
                Ok(WorkerEvent::DevicesChanged(devices)) => {
                    events.push(KeyboardRuntimeEvent::DevicesChanged(devices));
                }
                Ok(WorkerEvent::Input { device, event }) => {
                    events.push(KeyboardRuntimeEvent::Input { device, event });
                }
                Ok(WorkerEvent::Error(error)) => return Err(error),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    return Err("keyboard raw-input worker stopped unexpectedly".to_string());
                }
            }
        }

        Ok(events)
    }
}

#[cfg(windows)]
impl Drop for WindowsKeyboardInputRuntime {
    fn drop(&mut self) {
        stop_worker(self.thread_id);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(crate) struct KeyboardOutputController;

impl KeyboardOutputController {
    pub(crate) fn create() -> Result<Self, String> {
        Ok(Self)
    }

    pub(crate) fn execute_key(
        &mut self,
        key: KeyboardKey,
        action: KeyboardKeyAction,
    ) -> Result<String, String> {
        let output_key = key.to_rdev();
        match action {
            KeyboardKeyAction::Tap => {
                simulate_event(&EventType::KeyPress(output_key))
                    .map_err(|error| format!("failed to press the keyboard key: {error}"))?;
                simulate_event(&EventType::KeyRelease(output_key))
                    .map_err(|error| format!("failed to release the keyboard key: {error}"))?;
            }
            KeyboardKeyAction::Press => simulate_event(&EventType::KeyPress(output_key))
                .map_err(|error| format!("failed to press the keyboard key: {error}"))?,
            KeyboardKeyAction::Release => simulate_event(&EventType::KeyRelease(output_key))
                .map_err(|error| format!("failed to release the keyboard key: {error}"))?,
        }

        Ok(format!(
            "{} {} key",
            action.summary_label(),
            key.label().to_ascii_lowercase()
        ))
    }
}

fn simulate_event(event: &EventType) -> Result<(), SimulateError> {
    simulate(event)
}