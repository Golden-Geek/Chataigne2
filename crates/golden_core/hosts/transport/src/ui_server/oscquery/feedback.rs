use super::{graph, input::IMMEDIATE, values};
use golden_engine::{
    engine::EngineTime,
    node::NodeId,
    parameter::ParamValue,
    ui_read_model::{UiReadModel, UiSnapshotCapture},
};
use golden_io::{RevisionReader, WorkerNotifier};
use golden_protocol::{UiReplayCursor, UiSubscriptionScope};
use rosc::{OscBundle, OscMessage, OscPacket, encoder};
use std::{
    collections::HashMap,
    future,
    time::{Duration, Instant},
};
use tokio::sync::Notify;

const MAX_MESSAGE: usize = 64 * 1024;
const MAX_SUBSCRIPTIONS: usize = 128;
const MAX_PACKETS_PER_BATCH: usize = 512;
const FEEDBACK_INTERVAL: Duration = Duration::from_millis(20);

struct Subscription {
    address: String,
    after: EngineTime,
}

/// Connection-owned addresses, cursor and send cadence. One client's backpressure cannot
/// retain publications or delay another client's feedback.
pub(super) struct Feedback {
    subscriptions: HashMap<NodeId, Subscription>,
    cursor: UiReplayCursor,
    seen_revision: u64,
    next_send: Option<Instant>,
}

impl Feedback {
    pub(super) fn new(cursor: UiReplayCursor, seen_revision: u64) -> Self {
        Self {
            subscriptions: HashMap::new(),
            cursor,
            seen_revision,
            next_send: None,
        }
    }

    pub(super) fn project_generation(&self) -> u64 {
        self.cursor.project_generation
    }

    pub(super) fn subscribe(
        &mut self,
        snapshot: &UiSnapshotCapture,
        address: String,
        send_value: bool,
    ) -> Result<Option<Vec<u8>>, &'static str> {
        let node = graph::resolve(snapshot, &address).ok_or("Unknown OSC address")?;
        let param = graph::parameter(node).ok_or("The OSC address does not have a value or trigger")?;
        if self.subscriptions.len() == MAX_SUBSCRIPTIONS && !self.subscriptions.contains_key(&node.node_id) {
            return Err("OSCQuery subscription limit reached");
        }
        if self.subscriptions.is_empty() {
            self.cursor.at = snapshot.revision();
        }
        let initial = if send_value && values::readable(&param.value) {
            message(&address, &param.value)
                .and_then(|message| encoder::encode(&message).ok())
                .filter(|bytes| bytes.len() <= MAX_MESSAGE)
        } else {
            None
        };
        self.subscriptions
            .entry(node.node_id)
            .and_modify(|entry| {
                if entry.address != address {
                    entry.address = address.clone();
                    entry.after = snapshot.revision();
                }
            })
            .or_insert(Subscription {
                address,
                after: snapshot.revision(),
            });
        Ok(initial)
    }

    pub(super) fn ignore(&mut self, address: &str) {
        self.subscriptions.retain(|_, entry| entry.address != address);
        if self.subscriptions.is_empty() {
            self.next_send = None;
        }
    }

    pub(super) async fn wait(&self, revision: &RevisionReader, notifier: &WorkerNotifier, wake: &Notify) -> bool {
        if self.subscriptions.is_empty() {
            return future::pending().await;
        }
        let _subscription = revision.guard_at(self.seen_revision).subscribe(notifier.clone());
        wake.notified().await;
        let buffered = self.next_send.filter(|deadline| *deadline > Instant::now());
        if let Some(deadline) = buffered {
            tokio::time::sleep_until(deadline.into()).await;
        }
        buffered.is_some()
    }

    pub(super) fn collect(
        &mut self,
        model: &UiReadModel,
        observed_revision: u64,
        buffered: bool,
    ) -> Result<Option<Vec<u8>>, &'static str> {
        self.seen_revision = observed_revision;
        let delta = model
            .parameter_feedback_after(self.cursor, |id, at, value| {
                fits_message(value)
                    && self
                        .subscriptions
                        .get(&id)
                        .is_some_and(|subscription| at > subscription.after)
            })
            .ok_or("OSCQuery feedback cursor expired")?;
        self.cursor.at = delta.through;
        if delta.topology_changed {
            let snapshot = model.capture_snapshot(UiSubscriptionScope::WholeGraph);
            if snapshot.project_generation().get() != self.project_generation() {
                return Err("OSCQuery project changed");
            }
            self.subscriptions
                .retain(|id, entry| graph::resolve(&snapshot, &entry.address).is_some_and(|node| node.node_id == *id));
        }
        let bundle = buffered || delta.changes.len() > 1;
        let mut states = HashMap::new();
        let mut ordered = Vec::new();
        for (index, event) in delta.changes.into_iter().enumerate() {
            if !self.subscriptions.contains_key(&event.node) {
                continue;
            }
            if matches!(event.value, ParamValue::Trigger()) {
                ordered.push((index, event));
            } else {
                states.insert(event.node, (index, event));
            }
        }
        ordered.extend(states.into_values());
        ordered.sort_unstable_by_key(|(index, _)| *index);
        let packets = ordered
            .into_iter()
            .filter_map(|(_, event)| message(&self.subscriptions[&event.node].address, &event.value));
        let frame = encode_feedback(packets, bundle)?;
        if frame.is_some() {
            self.next_send = Some(Instant::now() + FEEDBACK_INTERVAL);
        }
        Ok(frame)
    }
}

fn message(address: &str, value: &ParamValue) -> Option<OscPacket> {
    if !fits_message(value) {
        return None;
    }
    Some(OscPacket::Message(OscMessage {
        addr: address.into(),
        args: values::outgoing(value)?,
    }))
}

fn fits_message(value: &ParamValue) -> bool {
    match value {
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => value.len() <= MAX_MESSAGE,
        ParamValue::Bytes(value) => value.len() <= MAX_MESSAGE,
        _ => true,
    }
}

fn encode_feedback(packets: impl Iterator<Item = OscPacket>, bundle: bool) -> Result<Option<Vec<u8>>, &'static str> {
    let mut contents = Vec::new();
    let mut total_bytes = 16; // OSC bundle header.
    for packet in packets {
        let bytes = encoder::encode(&packet).map_err(|_| "OSCQuery cannot encode the parameter value")?;
        if !bundle {
            return Ok((bytes.len() <= MAX_MESSAGE).then_some(bytes));
        }
        if bytes.len() + 20 > MAX_MESSAGE {
            continue;
        }
        total_bytes += 4 + bytes.len();
        if total_bytes > MAX_MESSAGE || contents.len() == MAX_PACKETS_PER_BATCH {
            return Err("OSCQuery feedback batch exceeded its limit");
        }
        contents.push(packet);
    }
    if contents.is_empty() {
        return Ok(None);
    }
    encoder::encode(&OscPacket::Bundle(OscBundle {
        timetag: IMMEDIATE,
        content: contents,
    }))
    .map(Some)
    .map_err(|_| "OSCQuery cannot encode the feedback bundle")
}
