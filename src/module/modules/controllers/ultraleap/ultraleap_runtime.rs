#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use std::thread::{self, JoinHandle};

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
use leaprs::{Connection, ConnectionConfig, Error as LeapError, EventRef, HandType, TrackingEventRef};

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
const ULTRALEAP_WORKER_POLL_TIMEOUT_MS: u32 = 25;
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
        Err("Ultraleap is only supported on Windows, macOS, and Linux.".to_string())
    }

    pub fn poll(&mut self) -> Result<UltraleapRuntimePoll, String> {
        Ok(UltraleapRuntimePoll::default())
    }
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
    let mut connection = match Connection::create(ConnectionConfig::default()) {
        Ok(connection) => connection,
        Err(error) => {
            let _ = ready_tx.send(Err(format!("Failed to create Ultraleap connection: {error}")));
            return;
        }
    };

    if let Err(error) = connection.open() {
        let _ = ready_tx.send(Err(format!("Failed to open Ultraleap connection: {error}")));
        return;
    }

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
    connection: &mut Connection,
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
                message.event(),
                &mut service_connected,
                &mut device_list_dirty,
                &mut latest_frame,
                &mut last_event,
                &mut poll_dirty,
            );
        }
        Err(LeapError::Timeout) => {}
        Err(error) => {
            return Err(format!("Ultraleap runtime poll failed: {error}"));
        }
    }

    if first_poll_received {
        for _ in 0..63 {
            match connection.poll(0) {
                Ok(message) => handle_event(
                    message.event(),
                    &mut service_connected,
                    &mut device_list_dirty,
                    &mut latest_frame,
                    &mut last_event,
                    &mut poll_dirty,
                ),
                Err(LeapError::Timeout) => break,
                Err(error) => {
                    return Err(format!("Ultraleap runtime poll failed: {error}"));
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
fn handle_event(
    event: EventRef<'_>,
    service_connected: &mut bool,
    device_list_dirty: &mut bool,
    latest_frame: &mut Option<UltraleapFrameSnapshot>,
    last_event: &mut Option<String>,
    poll_dirty: &mut bool,
) {
    match event {
        EventRef::None => {}
        EventRef::Connection(_) => {
            *service_connected = true;
            *device_list_dirty = true;
            *poll_dirty = true;
            *last_event = Some("Connected to Ultraleap Tracking Service".to_string());
        }
        EventRef::ConnectionLost(_) => {
            *service_connected = false;
            *device_list_dirty = true;
            *poll_dirty = true;
            *latest_frame = None;
            *last_event = Some("Lost Ultraleap Tracking Service connection".to_string());
        }
        EventRef::Device(_) => {
            *service_connected = true;
            *device_list_dirty = true;
            *poll_dirty = true;
            *last_event = Some("Ultraleap device connected".to_string());
        }
        EventRef::DeviceLost => {
            *device_list_dirty = true;
            *poll_dirty = true;
            *latest_frame = None;
            *last_event = Some("Ultraleap device disconnected".to_string());
        }
        EventRef::DeviceFailure(_) => {
            *device_list_dirty = true;
            *poll_dirty = true;
            *last_event = Some("Ultraleap device failure".to_string());
        }
        EventRef::DeviceStatusChange(_) => {
            *device_list_dirty = true;
            *poll_dirty = true;
            *last_event = Some("Ultraleap device status changed".to_string());
        }
        EventRef::Tracking(event) => {
            *service_connected = true;
            *latest_frame = Some(snapshot_from_tracking(event));
        }
        _ => {}
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn refresh_connected_devices(connection: &mut Connection) -> Result<usize, String> {
    match connection.get_device_list() {
        Ok(devices) => Ok(devices.len()),
        Err(LeapError::NotConnected | LeapError::HandshakeIncomplete | LeapError::NotAvailable) => Ok(0),
        Err(error) => Err(format!("Failed to enumerate Ultraleap devices: {error}")),
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn snapshot_from_tracking(event: TrackingEventRef<'_>) -> UltraleapFrameSnapshot {
    let mut snapshot = UltraleapFrameSnapshot::default();
    let hands = event.hands();
    snapshot.hand_count = hands.len();

    for hand in hands {
        match hand.hand_type() {
            HandType::Left => snapshot.left = snapshot_from_hand(hand),
            HandType::Right => snapshot.right = snapshot_from_hand(hand),
        }
    }

    snapshot
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn snapshot_from_hand(hand: leaprs::HandRef<'_>) -> UltraleapHandSnapshot {
    let palm = hand.palm();
    UltraleapHandSnapshot {
        active: true,
        grab_strength: hand.grab_strength as f64,
        pinch_strength: hand.pinch_strength as f64,
        pinch_distance: millimeters_to_meters(hand.pinch_distance as f64),
        thumb_extended: hand.thumb().is_extended(),
        index_extended: hand.index().is_extended(),
        middle_extended: hand.middle().is_extended(),
        ring_extended: hand.ring().is_extended(),
        pinky_extended: hand.pinky().is_extended(),
        palm_position: leap_vec3_meters(palm.position()),
        palm_stabilized_position: leap_vec3_meters(palm.stabilized_position()),
        palm_velocity: leap_vec3_meters(palm.velocity()),
        palm_direction: leap_vec3(palm.direction()),
        palm_normal: leap_vec3(palm.normal()),
    }
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn leap_vec3(vector: leaprs::LeapVectorRef<'_>) -> UltraleapVec3 {
    let [x, y, z] = vector.array();
    UltraleapVec3::new(x as f64, y as f64, z as f64)
}

#[cfg(any(windows, target_os = "linux", target_os = "macos"))]
fn leap_vec3_meters(vector: leaprs::LeapVectorRef<'_>) -> UltraleapVec3 {
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
