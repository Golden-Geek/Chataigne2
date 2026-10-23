use super::{Provider, UltraleapRuntimePoll};
use std::time::{Duration, Instant};
mod api;
#[cfg(test)]
mod tests;
mod tracking;
use api::{Connection, LeapCallError};

pub(super) struct Native {
    connection: Connection,
    connected: bool,
    devices: usize,
    retry_at: Option<Instant>,
}

pub(super) fn create() -> Result<Native, String> {
    Ok(Native {
        connection: Connection::create()?,
        connected: false,
        devices: 0,
        retry_at: None,
    })
}
impl Provider for Native {
    fn poll(&mut self) -> Result<Vec<UltraleapRuntimePoll>, String> {
        if self.retry_at.is_some_and(|at| Instant::now() < at) {
            return Ok(Vec::new());
        }
        self.retry_at = None;
        let mut rows = Vec::with_capacity(16);
        for index in 0..16 {
            let message = match self.connection.poll(if index == 0 { 25 } else { 0 }) {
                Ok(message) => message,
                Err(LeapCallError::Timeout) => break,
                Err(error) if error.is_service_unavailable() => {
                    self.retry_at = Some(Instant::now() + Duration::from_millis(250));
                    if self.connected || self.devices != 0 {
                        self.connected = false;
                        self.devices = 0;
                        rows.push(self.status("Ultraleap Tracking Service unavailable"));
                    }
                    break;
                }
                Err(error) => return Err(format!("Ultraleap runtime poll failed: {}", error.describe())),
            };
            let event = tracking::message_event_type(&message);
            let text = match event {
                leap_sys::_eLeapEventType_eLeapEventType_Connection => {
                    self.connected = true;
                    Some("Connected to Ultraleap Tracking Service")
                }
                leap_sys::_eLeapEventType_eLeapEventType_ConnectionLost => {
                    self.connected = false;
                    self.devices = 0;
                    Some("Lost Ultraleap Tracking Service connection")
                }
                leap_sys::_eLeapEventType_eLeapEventType_Device => {
                    self.connected = true;
                    Some("Ultraleap device connected")
                }
                leap_sys::_eLeapEventType_eLeapEventType_DeviceLost => Some("Ultraleap device disconnected"),
                leap_sys::_eLeapEventType_eLeapEventType_DeviceFailure => Some("Ultraleap device failure"),
                leap_sys::_eLeapEventType_eLeapEventType_DeviceStatusChange => Some("Ultraleap device status changed"),
                leap_sys::_eLeapEventType_eLeapEventType_Tracking => {
                    self.connected = true;
                    if self.devices == 0 {
                        self.refresh_devices()?;
                    }
                    if let Some(event) = tracking::tracking_event_from_message(&message) {
                        rows.push(UltraleapRuntimePoll {
                            service_connected: self.connected,
                            connected_devices: self.devices,
                            frame: Some(tracking::snapshot_from_tracking(event)?),
                            last_event: None,
                        });
                    }
                    None
                }
                _ => None,
            };
            if let Some(text) = text {
                if self.connected {
                    self.refresh_devices()?;
                }
                rows.push(self.status(text));
            }
        }
        Ok(rows)
    }
}
impl Native {
    fn refresh_devices(&mut self) -> Result<(), String> {
        self.devices = match self.connection.connected_devices() {
            Ok(count) => count,
            Err(error) if error.is_service_unavailable() => 0,
            Err(error) => return Err(format!("Failed to enumerate Ultraleap devices: {}", error.describe())),
        };
        Ok(())
    }
    fn status(&self, text: &str) -> UltraleapRuntimePoll {
        UltraleapRuntimePoll {
            service_connected: self.connected,
            connected_devices: self.devices,
            frame: None,
            last_event: Some(text.into()),
        }
    }
}
