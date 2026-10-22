use super::transport::MqttReceivedPublish;
use crate::app::module::{common::received_values::ReceivedValueMessage, script_api::NativeCallbackPayload};
use golden_core::{
    node::NodeId,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::{RetainedPayload, RevisionGuard, RevisionSource, WorkerNotifier};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

mod build;
mod decode;
mod delivery;
mod preparation;
mod source;
#[cfg(test)]
mod tests;

use delivery::Delivery;
use preparation::Preparation;
use source::Source;

const MAX_PAYLOAD: usize = 64 * 1024;
const MAX_UNITS: usize = 8192;
const MAX_PATH: usize = 24;
const MAX_VALUES_BYTES: usize = 4 * 1024 * 1024;
const MAX_PREPARED_BYTES: usize = 16 * 1024 * 1024;
const QUEUE_ITEMS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PayloadMode {
    Auto,
    Text,
    Json,
    Raw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Mode {
    payload: PayloadMode,
    auto_add: bool,
}

struct Prepared {
    source: Source,
    values: Vec<ReceivedValueMessage>,
    callback: NativeCallbackPayload,
    error: Option<String>,
    _raw: RetainedPayload<MqttReceivedPublish>,
}

#[derive(Default)]
pub(super) struct Inbox {
    queued: VecDeque<RetainedPayload<MqttReceivedPublish>>,
    preparing: Option<Preparation>,
    delivery: Option<Delivery>,
    lifetime: Option<RevisionSource>,
    retry: Option<Instant>,
    mode: Option<Mode>,
    pub(super) feedback: Option<String>,
    pub(super) clear_feedback: bool,
}

impl Inbox {
    pub(super) fn activate(&mut self) {
        self.lifetime.get_or_insert_default();
    }
    pub(super) fn suspend(&mut self) {
        let count = self.len();
        self.lifetime = None;
        self.queued.clear();
        self.preparing = None;
        self.delivery = None;
        self.retry = None;
        self.mode = None;
        if count != 0 {
            golden_core::logerror!(format!(
                "MQTT input source retired; {count} pending messages cancelled without replay"
            ));
            self.feedback = Some(format!(
                "MQTT input source retired; {count} pending messages cancelled without replay"
            ));
        }
    }
    pub(super) fn len(&self) -> usize {
        self.queued.len() + usize::from(self.preparing.is_some()) + usize::from(self.delivery.is_some())
    }
    pub(super) fn can_receive(&self) -> bool {
        self.len() < QUEUE_ITEMS
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(super) fn push(&mut self, message: RetainedPayload<MqttReceivedPublish>) {
        if self.lifetime.is_none() {
            self.feedback = Some("MQTT input was cancelled while its configuration source is changing".into());
        } else if !self.can_receive() {
            self.feedback = Some("MQTT incoming message queue is full; message was not admitted".into());
        } else {
            self.queued.push_back(message);
        }
    }
    pub(super) fn requires_snapshot(&self) -> bool {
        self.delivery.as_ref().is_some_and(Delivery::requires_snapshot)
            || self.preparing.as_ref().is_none_or(Preparation::is_complete)
                && !self.queued.is_empty()
                && self.retry.is_none_or(|at| Instant::now() >= at)
    }
    pub(super) fn needs_service(&self) -> bool {
        self.delivery.is_some()
            || self.preparing.as_ref().is_some_and(Preparation::is_complete)
            || !self.queued.is_empty() && self.preparing.is_none() && self.retry.is_none_or(|at| Instant::now() >= at)
    }
    pub(super) fn retry_at(&self) -> Option<Instant> {
        self.retry
    }
    pub(super) fn notice_processing(&mut self, snapshot: &ProcessTreeSnapshot, module: NodeId) {
        if self
            .mode
            .is_some_and(|mode| Mode::capture(snapshot, module) != Some(mode))
        {
            let active = self.lifetime.is_some();
            self.suspend();
            if active {
                self.activate();
            }
        }
    }
    pub(super) fn service(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: Option<&ProcessTreeSnapshot>,
        module: NodeId,
        log: bool,
        wake: WorkerNotifier,
    ) {
        if let Some(delivery) = &mut self.delivery {
            let done = delivery.service(ctx, snapshot);
            if let Some(error) = delivery.take_error() {
                self.feedback = Some(error);
            }
            if done {
                self.delivery = None;
            }
            return;
        }
        if let Some(request) = &mut self.preparing {
            if request.is_pressured() {
                self.feedback =
                    Some("MQTT incoming preparation is waiting for retained payload/callback capacity".into());
            }
            match request.poll() {
                Some(Ok(input)) => {
                    self.clear_feedback = input.error.is_none();
                    self.preparing = None;
                    self.delivery = Some(Delivery::new(input));
                }
                Some(Err(error)) => {
                    self.preparing = None;
                    self.feedback = Some(error);
                }
                None => {}
            }
            return;
        }
        if self.retry.is_some_and(|at| Instant::now() < at) || self.queued.is_empty() {
            return;
        }
        let Some(snapshot) = snapshot else { return };
        let Some(lifetime) = &self.lifetime else {
            self.suspend();
            return;
        };
        let guard = lifetime.reader().guard_at(0);
        let source = match Source::capture(snapshot, module, guard) {
            Ok(source) => source,
            Err(error) => {
                self.suspend();
                self.feedback = Some(error);
                return;
            }
        };
        self.mode = Some(source.mode);
        match Preparation::start(self.queued.front().unwrap().clone(), source, log, wake) {
            Ok(request) => {
                self.queued.pop_front();
                self.preparing = Some(request);
                self.retry = None;
            }
            Err(error) => {
                self.feedback = Some(error);
                self.retry = Some(Instant::now() + Duration::from_millis(250));
            }
        }
    }
}
