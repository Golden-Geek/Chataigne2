use super::*;
use device_query::{DeviceQuery, DeviceState};
use std::{
    collections::BTreeSet,
    sync::mpsc::{self, Receiver, SyncSender},
    thread::{self, JoinHandle},
};
const GLOBAL_KEYBOARD_VARIANT: &str = "system|System Keyboard";
#[derive(Clone, Debug, Default, PartialEq)]
struct KeyboardObservedState {
    pressed_keys: BTreeSet<KeyboardKey>,
}

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

fn diff_keyboard_states(previous: &KeyboardObservedState, next: &KeyboardObservedState) -> Vec<KeyboardInputEvent> {
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

#[derive(Clone)]
struct GlobalWorkerReady {
    devices: Vec<DiscoveredKeyboardDevice>,
}

#[derive(Clone, Debug)]
enum GlobalWorkerEvent {
    Input { device: String, event: KeyboardInputEvent },
}

pub(super) struct GlobalKeyboardInputRuntime {
    event_rx: PendingReceiver<GlobalWorkerEvent>,
    pending_events: Vec<KeyboardRuntimeEvent>,
    shutdown_tx: SyncSender<()>,
    failure: Failure,
    worker: Option<JoinHandle<()>>,
}

impl GlobalKeyboardInputRuntime {
    pub(super) fn create() -> Result<Self, String> {
        let (event_tx, event_rx, failure) = channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (shutdown_tx, shutdown_rx) = mpsc::sync_channel(1);
        let worker = spawn_global_worker(event_tx, ready_tx, shutdown_rx)?;
        let ready = match ready_rx.recv() {
            Ok(Ok(ready)) => ready,
            result => {
                let _ = worker.join();
                return Err(match result {
                    Ok(Err(error)) => error,
                    _ => "keyboard global-input worker exited before becoming ready".into(),
                });
            }
        };

        Ok(Self {
            event_rx,
            failure,
            pending_events: vec![KeyboardRuntimeEvent::DevicesChanged(ready.devices)],
            shutdown_tx,
            worker: Some(worker),
        })
    }

    fn poll_events(&mut self) -> Result<Vec<KeyboardRuntimeEvent>, String> {
        drain(
            &self.event_rx,
            &mut self.pending_events,
            &self.failure,
            |event| match event {
                GlobalWorkerEvent::Input { device, event } => KeyboardRuntimeEvent::Input { device, event },
            },
        )
    }
}

impl Drop for GlobalKeyboardInputRuntime {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.try_send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn spawn_global_worker(
    event_tx: Sender<GlobalWorkerEvent>,
    ready_tx: mpsc::SyncSender<Result<GlobalWorkerReady, String>>,
    shutdown_rx: Receiver<()>,
) -> Result<JoinHandle<()>, String> {
    thread::Builder::new()
        .name("keyboard-global-input".to_string())
        .spawn(move || {
            if let Err(error) = global_worker_main(event_tx.clone(), ready_tx, shutdown_rx) {
                event_tx.fail(error);
            }
        })
        .map_err(|error| format!("failed to spawn the keyboard global-input worker: {error}"))
}

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

    let _stopped = NativeStopped(event_tx.failure.clone());
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

fn global_keyboard_device() -> DiscoveredKeyboardDevice {
    DiscoveredKeyboardDevice {
        index: 0,
        variant_id: GLOBAL_KEYBOARD_VARIANT.to_string(),
        label: "System Keyboard".to_string(),
        details: "Global system keyboard input".to_string(),
    }
}

impl Record for GlobalWorkerEvent {
    fn weight(&self) -> Result<usize, String> {
        Ok(INPUT_WEIGHT)
    }
}
impl Provider for GlobalKeyboardInputRuntime {
    fn poll(&mut self) -> Result<Vec<KeyboardRuntimeEvent>, String> {
        self.poll_events()
    }
}
