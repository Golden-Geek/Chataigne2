use super::DiscoveredSerialPort;
use crate::app::module::common::serial::metadata::describe_serial_port;
use golden_io::{BoundedWorker, RetirementPool, WorkerCancellation, WorkerLimits, WorkerNotifier};
use std::{collections::HashSet, time::Duration};
const SERIAL_DISCOVERY_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub(super) struct SerialDiscoveryWorker {
    worker: BoundedWorker<(), ()>,
}
impl SerialDiscoveryWorker {
    pub fn spawn_using(
        pool: &RetirementPool,
        detector: impl FnMut() -> Result<Vec<DiscoveredSerialPort>, String> + Send + 'static,
        publisher: impl FnMut(Vec<DiscoveredSerialPort>, Option<String>) + Send + 'static,
        notifier: WorkerNotifier,
    ) -> Result<Self, String> {
        let worker = BoundedWorker::spawn_notified(
            "serial-discovery",
            pool,
            WorkerLimits {
                command_items: 1,
                command_weight: 1,
                event_items: 1,
                event_weight: 1,
            },
            notifier,
            move |context| serial_discovery_worker_loop(context.cancellation(), detector, publisher),
        )
        .map_err(|error| format!("Serial discovery worker could not start: {error}"))?;
        Ok(Self { worker })
    }
    pub fn stopped(&self) -> bool {
        self.worker.has_pending()
    }
    pub fn stop(&mut self) {
        self.worker.cancel();
    }
    #[cfg(test)]
    pub fn spawn_with(
        pool: &RetirementPool,
        detector: impl FnMut() -> Result<Vec<DiscoveredSerialPort>, String> + Send + 'static,
        publisher: impl FnMut(Vec<DiscoveredSerialPort>, Option<String>) + Send + 'static,
    ) -> Result<Self, String> {
        Self::spawn_using(pool, detector, publisher, WorkerNotifier::new(|| {}))
    }
}
pub(super) fn detect_available_serial_ports() -> Result<Vec<DiscoveredSerialPort>, String> {
    let ports = serialport::available_ports().map_err(|error| format!("failed to enumerate serial ports: {error}"))?;

    if ports.len() > super::catalog::MAX_PORTS {
        return Err("Serial catalog exceeds 128 ports".into());
    }
    let mut seen = HashSet::<String>::new();
    let mut discovered = ports
        .into_iter()
        .filter(|port| seen.insert(port.port_name.clone()))
        .map(describe_serial_port)
        .collect::<Vec<_>>();
    discovered.sort_by(|left, right| left.label.cmp(&right.label));

    Ok(discovered)
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
        let detected = detector().and_then(|ports| {
            super::SerialCatalog::validate(&ports)?;
            Ok(ports)
        });
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
                let error = super::bounded_error(error);
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
