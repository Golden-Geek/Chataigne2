use super::{metadata::describe_serial_port, DiscoveredSerialPort};
use golden_io::{BoundedWorker, RetirementPool, WorkerCancellation, WorkerLimits};
use std::{
    collections::HashSet,
    num::NonZeroUsize,
    sync::{LazyLock, Mutex, OnceLock},
    time::Duration,
};
const SERIAL_DISCOVERY_POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SerialDiscoverySnapshot {
    pub version: u64,
    pub ports: Vec<DiscoveredSerialPort>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub(crate) struct SerialDiscoveryRegistration {
    id: u64,
}

struct SerialDiscoveryState {
    next_registration_id: u64,
    registrations: HashSet<u64>,
    snapshot: SerialDiscoverySnapshot,
    worker: Option<SerialDiscoveryWorker>,
}

struct SerialDiscoveryWorker {
    worker: BoundedWorker<(), ()>,
}

fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::MIN))
}

static SERIAL_DISCOVERY_STATE: LazyLock<Mutex<SerialDiscoveryState>> =
    LazyLock::new(|| Mutex::new(SerialDiscoveryState::new()));

impl SerialDiscoveryState {
    fn new() -> Self {
        Self {
            next_registration_id: 1,
            registrations: HashSet::new(),
            snapshot: SerialDiscoverySnapshot::default(),
            worker: None,
        }
    }
}

impl SerialDiscoveryRegistration {
    pub(crate) fn register() -> Result<Self, String> {
        let mut state = lock_serial_discovery_state();
        let registration_id = state.next_registration_id;
        state.next_registration_id = state
            .next_registration_id
            .checked_add(1)
            .ok_or("Serial discovery registration identity exhausted")?;
        state.registrations.insert(registration_id);

        if state.worker.is_none() {
            state.snapshot = SerialDiscoverySnapshot::default();
            match SerialDiscoveryWorker::spawn() {
                Ok(worker) => state.worker = Some(worker),
                Err(error) => {
                    state.registrations.remove(&registration_id);
                    return Err(error);
                }
            }
        }

        Ok(Self { id: registration_id })
    }

    pub(crate) fn snapshot_version(&self) -> u64 {
        lock_serial_discovery_state().snapshot.version
    }

    pub(crate) fn snapshot(&self) -> SerialDiscoverySnapshot {
        lock_serial_discovery_state().snapshot.clone()
    }
}

impl Drop for SerialDiscoveryRegistration {
    fn drop(&mut self) {
        let worker = {
            let mut state = lock_serial_discovery_state();
            if !state.registrations.remove(&self.id) {
                return;
            }

            if state.registrations.is_empty() {
                state.worker.take()
            } else {
                None
            }
        };

        if let Some(mut worker) = worker {
            worker.stop();
        }

        let mut state = lock_serial_discovery_state();
        if state.registrations.is_empty() {
            state.snapshot = SerialDiscoverySnapshot::default();
        }
    }
}

impl SerialDiscoveryWorker {
    fn spawn() -> Result<Self, String> {
        Self::spawn_with(
            lifetimes(),
            detect_available_serial_ports,
            publish_serial_discovery_snapshot,
        )
    }
    fn spawn_with(
        pool: &RetirementPool,
        detector: impl FnMut() -> Result<Vec<DiscoveredSerialPort>, String> + Send + 'static,
        publisher: impl FnMut(Vec<DiscoveredSerialPort>, Option<String>) + Send + 'static,
    ) -> Result<Self, String> {
        let worker = BoundedWorker::spawn(
            "serial-discovery",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1,
            },
            move |context| serial_discovery_worker_loop(context.cancellation(), detector, publisher),
        )
        .map_err(|error| format!("Serial discovery worker could not start: {error}"))?;
        Ok(Self { worker })
    }
    fn stop(&mut self) {
        self.worker.cancel();
    }
}

fn detect_available_serial_ports() -> Result<Vec<DiscoveredSerialPort>, String> {
    let ports = serialport::available_ports().map_err(|error| format!("failed to enumerate serial ports: {error}"))?;

    let mut seen = HashSet::<String>::new();
    let mut discovered = ports
        .into_iter()
        .filter(|port| seen.insert(port.port_name.clone()))
        .map(describe_serial_port)
        .collect::<Vec<_>>();
    discovered.sort_by(|left, right| left.label.cmp(&right.label));

    Ok(discovered)
}

fn lock_serial_discovery_state() -> std::sync::MutexGuard<'static, SerialDiscoveryState> {
    SERIAL_DISCOVERY_STATE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

fn serial_discovery_worker_loop(
    cancel: &WorkerCancellation,
    mut detector: impl FnMut() -> Result<Vec<DiscoveredSerialPort>, String>,
    mut publisher: impl FnMut(Vec<DiscoveredSerialPort>, Option<String>),
) {
    let mut initialized = false;
    let mut last_ports = Vec::<DiscoveredSerialPort>::new();
    let mut last_error = None::<String>;

    while !cancel.is_cancelled() {
        let detected = detector();
        if cancel.is_cancelled() {
            return;
        }
        match detected {
            Ok(ports) => {
                if initialized {
                    log_serial_discovery_changes(last_ports.as_slice(), ports.as_slice());
                    if last_error.is_some() {
                        golden_core::log!(
                            tag = "serial";
                            format!("Serial port discovery recovered with {} port(s).", ports.len())
                        );
                    }
                }

                publisher(ports.clone(), None);
                last_ports = ports;
                last_error = None;
                initialized = true;
            }
            Err(error) => {
                if last_error.as_deref() != Some(error.as_str()) {
                    golden_core::logerror!(tag = "serial"; format!("Failed to enumerate serial ports: {}", error));
                }

                publisher(last_ports.clone(), Some(error.clone()));
                last_error = Some(error);
                initialized = true;
            }
        }

        if cancel.wait(SERIAL_DISCOVERY_POLL_INTERVAL) {
            return;
        }
    }
}

fn publish_serial_discovery_snapshot(ports: Vec<DiscoveredSerialPort>, error: Option<String>) {
    let mut state = lock_serial_discovery_state();
    if state.registrations.is_empty() || (state.snapshot.ports == ports && state.snapshot.error == error) {
        return;
    }

    state.snapshot.version = state.snapshot.version.wrapping_add(1);
    if state.snapshot.version == 0 {
        state.snapshot.version = 1;
    }
    state.snapshot.ports = ports;
    state.snapshot.error = error;
}

fn log_serial_discovery_changes(previous: &[DiscoveredSerialPort], current: &[DiscoveredSerialPort]) {
    for port in current {
        match previous.iter().find(|candidate| candidate.port_name == port.port_name) {
            None => {
                golden_core::log!(
                    tag = "serial";
                    format!("Serial port added: {} ({})", port.label, port.details)
                );
            }
            Some(existing) if existing != port => {
                golden_core::log!(
                    tag = "serial";
                    format!("Serial port updated: {} ({})", port.label, port.details)
                );
            }
            Some(_) => {}
        }
    }

    for port in previous {
        if current.iter().any(|candidate| candidate.port_name == port.port_name) {
            continue;
        }

        golden_core::log!(
            tag = "serial", level = warning;
            format!("Serial port removed: {} ({})", port.label, port.details)
        );
    }
}

#[cfg(test)]
mod tests;
