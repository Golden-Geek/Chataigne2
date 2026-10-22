use super::*;
use golden_io::{RetainedPayload, WorkerNotifier};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, OnceLock,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum PublishStatus {
    Pending,
    DispatchedUnconfirmed,
    Acknowledged,
    FailedUnconfirmed,
    Rejected,
}

struct State {
    status: AtomicU8,
    notifier: WorkerNotifier,
    failure: OnceLock<String>,
}

#[derive(Clone)]
pub(crate) struct PublishReceipt(Arc<State>);
impl PublishReceipt {
    pub(crate) fn status(&self) -> PublishStatus {
        match self.0.status.load(Ordering::Acquire) {
            0 => PublishStatus::Pending,
            1 => PublishStatus::DispatchedUnconfirmed,
            2 => PublishStatus::Acknowledged,
            4 => PublishStatus::Rejected,
            _ => PublishStatus::FailedUnconfirmed,
        }
    }
    pub(crate) fn failure(&self) -> Option<&str> {
        self.0.failure.get().map(String::as_str)
    }
}

enum Data {
    Captured {
        // Native bytes must drop before the source releases their reservation.
        prepared: Option<MqttPublishRequest>,
        source: RetainedPayload<super::super::output::Request>,
    },
    #[cfg(test)]
    Ready(RetainedPayload<MqttPublishRequest>),
}

pub(super) struct AdmittedPublish {
    data: Data,
    state: Arc<State>,
    origin: NodeId,
    log: bool,
}
impl AdmittedPublish {
    #[cfg(test)]
    pub(super) fn new(
        request: RetainedPayload<MqttPublishRequest>,
        notifier: WorkerNotifier,
        origin: NodeId,
        log: bool,
    ) -> (Self, PublishReceipt) {
        Self::with_data(Data::Ready(request), notifier, origin, log)
    }
    pub(super) fn captured(
        source: RetainedPayload<super::super::output::Request>,
        notifier: WorkerNotifier,
        origin: NodeId,
        log: bool,
    ) -> (Self, PublishReceipt) {
        Self::with_data(Data::Captured { prepared: None, source }, notifier, origin, log)
    }
    fn with_data(data: Data, notifier: WorkerNotifier, origin: NodeId, log: bool) -> (Self, PublishReceipt) {
        let state = Arc::new(State {
            status: AtomicU8::new(0),
            notifier,
            failure: OnceLock::new(),
        });
        (
            Self {
                data,
                state: state.clone(),
                origin,
                log,
            },
            PublishReceipt(state),
        )
    }
    pub(super) fn prepare(&mut self) -> Result<(), String> {
        match &mut self.data {
            Data::Captured { prepared, source } => {
                if !source.is_live() {
                    return Err("MQTT producer retired; request was not sent".into());
                }
                if prepared.is_none() {
                    let request = source.prepare()?;
                    let bytes = admission::publish_bytes(&request)?;
                    if bytes > source.reservation_weight() {
                        return Err("MQTT encoded output exceeds its retained reservation".into());
                    }
                    *prepared = Some(request);
                }
            }
            #[cfg(test)]
            Data::Ready(_) => {}
        }
        Ok(())
    }
    pub(super) fn request(&self) -> &MqttPublishRequest {
        match &self.data {
            Data::Captured { prepared, .. } => prepared.as_ref().expect("native preparation precedes dispatch"),
            #[cfg(test)]
            Data::Ready(request) => request,
        }
    }
    pub(super) fn reject(self, message: String) {
        let _ = self.state.failure.set(admission::diagnostic(message));
        self.state
            .status
            .store(PublishStatus::Rejected as u8, Ordering::Release);
        self.state.notifier.notify();
    }
    fn complete(self, status: PublishStatus) {
        if self.log {
            let request = self.request();
            let bytes = &request.payload[..request.payload.len().min(256)];
            let outcome = match status {
                PublishStatus::Acknowledged => "acknowledged by broker",
                _ => "dispatched without broker acknowledgement",
            };
            golden_core::log!(origin = self.origin; format!("MQTT {} on {}: {}{}", outcome, request.topic, super::super::format_mqtt_payload_for_log(bytes), if request.payload.len() > bytes.len() { " (preview)" } else { "" }));
        }
        self.state.status.store(status as u8, Ordering::Release);
        self.state.notifier.notify();
    }
}
impl Drop for AdmittedPublish {
    fn drop(&mut self) {
        if self
            .state
            .status
            .compare_exchange(
                0,
                PublishStatus::FailedUnconfirmed as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.state.notifier.notify();
        }
    }
}

#[derive(Default)]
pub(super) struct Publications {
    waiting: VecDeque<AdmittedPublish>,
    inflight: HashMap<u16, AdmittedPublish>,
}
impl Publications {
    pub(super) fn queued(&mut self, command: AdmittedPublish) {
        self.waiting.push_back(command);
    }
    pub(super) fn dispatched(&mut self, id: u16) {
        if id != 0 && self.inflight.contains_key(&id) {
            return;
        } // Protocol retransmission.
        let Some(command) = self.waiting.pop_front() else {
            return;
        };
        if id == 0 {
            command.complete(PublishStatus::DispatchedUnconfirmed);
        } else {
            self.inflight.insert(id, command);
        }
    }
    pub(super) fn acknowledged(&mut self, id: u16) {
        if let Some(command) = self.inflight.remove(&id) {
            command.complete(PublishStatus::Acknowledged);
        }
    }
    pub(super) fn session_lost(&mut self) {
        self.waiting.clear();
        self.inflight.clear();
    }
}
