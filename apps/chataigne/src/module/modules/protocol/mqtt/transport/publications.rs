use super::*;
use golden_io::{RetainedPayload, WorkerNotifier};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum PublishStatus {
    Pending,
    DispatchedUnconfirmed,
    Acknowledged,
    FailedUnconfirmed,
}

struct State {
    status: AtomicU8,
    notifier: WorkerNotifier,
}

#[derive(Clone)]
pub(crate) struct PublishReceipt(Arc<State>);
impl PublishReceipt {
    pub(crate) fn status(&self) -> PublishStatus {
        match self.0.status.load(Ordering::Acquire) {
            0 => PublishStatus::Pending,
            1 => PublishStatus::DispatchedUnconfirmed,
            2 => PublishStatus::Acknowledged,
            _ => PublishStatus::FailedUnconfirmed,
        }
    }
}

pub(super) struct AdmittedPublish {
    pub(super) request: RetainedPayload<MqttPublishRequest>,
    state: Arc<State>,
    origin: NodeId,
    log: bool,
}
impl AdmittedPublish {
    pub(super) fn new(
        request: RetainedPayload<MqttPublishRequest>,
        notifier: WorkerNotifier,
        origin: NodeId,
        log: bool,
    ) -> (Self, PublishReceipt) {
        let state = Arc::new(State {
            status: AtomicU8::new(0),
            notifier,
        });
        (
            Self {
                request,
                state: state.clone(),
                origin,
                log,
            },
            PublishReceipt(state),
        )
    }
    fn complete(self, status: PublishStatus) {
        if self.log {
            let bytes = &self.request.payload[..self.request.payload.len().min(256)];
            let outcome = match status {
                PublishStatus::Acknowledged => "acknowledged by broker",
                _ => "dispatched without broker acknowledgement",
            };
            golden_core::log!(origin = self.origin; format!("MQTT {} on {}: {}{}", outcome, self.request.topic, super::super::format_mqtt_payload_for_log(bytes), if self.request.payload.len() > bytes.len() { " (preview)" } else { "" }));
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
