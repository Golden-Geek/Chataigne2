use std::time::{Duration, Instant};

use golden_io::{PendingDrainState, ReconnectBackoff, WorkerNotifier};

use super::{
    outbound::UdpOutboundSend,
    transport::{StreamingWorkerEvent, UdpStreamingTransportConfig, UdpStreamingTransportHandle},
};

pub(super) enum UdpTransportEvent {
    Input(golden_io::RetainedPayload<super::input::PreparedUdpInput>),
    Interfaces(Result<Vec<golden_core::parameter::ParameterEnumOption>, String>),
    OutgoingLog(String),
    Error(String),
}

pub(super) struct UdpTransportPoll {
    pub events: Vec<UdpTransportEvent>,
    pub failure: Option<String>,
    pub disconnected: bool,
}

enum State {
    Idle,
    Active {
        config: UdpStreamingTransportConfig,
        handle: UdpStreamingTransportHandle,
        bound: Option<std::net::SocketAddr>,
    },
    Retiring(UdpStreamingTransportHandle),
}

/// One native generation at a time. Reconfiguration fences admission, drains
/// accepted sends/results, then releases the old socket before a new bind.
pub(super) struct UdpTransports {
    state: State,
    retry_at: Option<Instant>,
    backoff: ReconnectBackoff,
}

impl Default for UdpTransports {
    fn default() -> Self {
        Self {
            state: State::Idle,
            retry_at: None,
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
        }
    }
}

impl UdpTransports {
    /// Returns false while a prior generation or recovery deadline still owns the endpoint.
    pub(super) fn configure(
        &mut self,
        config: UdpStreamingTransportConfig,
        notifier: WorkerNotifier,
    ) -> Result<bool, String> {
        self.configure_using(config, move |config| {
            UdpStreamingTransportHandle::spawn(config, notifier)
        })
    }

    fn configure_using(
        &mut self,
        config: UdpStreamingTransportConfig,
        spawn: impl FnOnce(UdpStreamingTransportConfig) -> Result<UdpStreamingTransportHandle, String>,
    ) -> Result<bool, String> {
        if let State::Active {
            config: active, handle, ..
        } = &mut self.state
        {
            if active.same_socket(&config) {
                handle.update_input(config.input.clone());
                *active = config;
                return Ok(true);
            }
            self.pause_admission();
        }
        if matches!(self.state, State::Retiring(_)) {
            return Ok(false);
        }
        if self.retry_at.is_some_and(|deadline| Instant::now() < deadline) {
            return Ok(false);
        }
        let handle = match spawn(config.clone()) {
            Ok(handle) => handle,
            Err(error) => {
                self.retry_at = Some(self.backoff.schedule(Instant::now()));
                return Err(error);
            }
        };
        self.state = State::Active {
            config,
            handle,
            bound: None,
        };
        self.retry_at = None;
        Ok(true)
    }

    pub(super) fn pause_admission(&mut self) {
        if matches!(self.state, State::Active { .. }) {
            let State::Active { mut handle, .. } = std::mem::replace(&mut self.state, State::Idle) else {
                unreachable!()
            };
            handle.finish_admission();
            self.state = State::Retiring(handle);
        }
    }

    pub(super) fn send(&self, command: UdpOutboundSend) -> Result<(), String> {
        match &self.state {
            State::Active { handle, .. } => handle.send(command),
            _ => Err("UDP configuration is pending or disabled; packet was not admitted".into()),
        }
    }

    pub(super) fn retry_in(&self) -> Option<Duration> {
        self.retry_at.map(|at| at.saturating_duration_since(Instant::now()))
    }

    pub(super) fn waiting_for_retirement(&self) -> bool {
        matches!(self.state, State::Retiring(_))
    }

    pub(super) fn connected(&self) -> bool {
        matches!(self.state, State::Active { bound: Some(_), .. })
    }

    #[cfg(test)]
    pub(super) fn bound_address(&self) -> Option<std::net::SocketAddr> {
        match &self.state {
            State::Active { bound, .. } => *bound,
            _ => None,
        }
    }

    pub(super) fn has_pending(&self) -> bool {
        match &self.state {
            State::Active { handle, .. } | State::Retiring(handle) => handle.has_pending(),
            State::Idle => false,
        }
    }

    #[cfg(test)]
    pub(super) fn configuration(&self) -> Option<&UdpStreamingTransportConfig> {
        match &self.state {
            State::Active { config, .. } => Some(config),
            _ => None,
        }
    }

    pub(super) fn poll(&mut self) -> UdpTransportPoll {
        let mut result = UdpTransportPoll {
            events: Vec::new(),
            failure: None,
            disconnected: false,
        };
        let (handle, active) = match &self.state {
            State::Active { handle, .. } => (handle, true),
            State::Retiring(handle) => (handle, false),
            State::Idle => return result,
        };
        let mut events = Vec::new();
        let drain = handle.drain_events(&mut events);
        for event in events {
            match event {
                StreamingWorkerEvent::Ready(address) if active => {
                    if let State::Active { bound, .. } = &mut self.state {
                        *bound = Some(address);
                    }
                    self.backoff.reset();
                }
                StreamingWorkerEvent::Failed(error) if active => result.failure = Some(error),
                StreamingWorkerEvent::Interfaces(options) => result.events.push(UdpTransportEvent::Interfaces(options)),
                StreamingWorkerEvent::Input(input) => result.events.push(UdpTransportEvent::Input(input)),
                StreamingWorkerEvent::OutgoingLog(message) => {
                    result.events.push(UdpTransportEvent::OutgoingLog(message))
                }
                StreamingWorkerEvent::Error(error) => result.events.push(UdpTransportEvent::Error(error)),
                _ => {}
            }
        }
        if result.failure.is_some() {
            self.retry_at = Some(self.backoff.schedule(Instant::now()));
            self.pause_admission();
        }
        if drain.state == PendingDrainState::Disconnected {
            if active && result.failure.is_none() {
                result.failure = Some(
                    "UDP worker disconnected; outstanding sends have an unknown outcome and are not replayed".into(),
                );
                self.retry_at = Some(self.backoff.schedule(Instant::now()));
            }
            self.state = State::Idle;
            result.disconnected = true;
        }
        result
    }

    pub(super) fn cancel_all(&mut self) {
        let previous = std::mem::replace(&mut self.state, State::Idle);
        match previous {
            State::Active { mut handle, .. } | State::Retiring(mut handle) => handle.stop(),
            State::Idle => {}
        }
        self.retry_at = None;
        self.backoff.reset();
    }
}

#[cfg(test)]
mod tests;
