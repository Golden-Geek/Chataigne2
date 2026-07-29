use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use golden_protocol::{UiDataPlane, UiEventKind, UiServerMessage};

use super::WsOutbound;

pub(super) const DEFAULT_OUTBOUND_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueuePushResult {
    Queued,
    Superseded,
    Full,
}

pub(super) struct WsOutboundQueue {
    capacity: usize,
    queue: Mutex<VecDeque<WsOutbound>>,
}

impl WsOutboundQueue {
    pub(super) fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "websocket outbound capacity must be non-zero");
        Self {
            capacity,
            queue: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub(super) fn push(&self, outbound: WsOutbound) -> QueuePushResult {
        let mut queue = self.queue.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        if matches!(outbound, WsOutbound::Close) {
            queue.clear();
            queue.push_back(outbound);
            return QueuePushResult::Queued;
        }

        if let Some(key) = latest_wins_key(&outbound) {
            for queued in queue.iter_mut().rev() {
                if outbound_subscription_id(queued).is_none_or(|subscription_id| subscription_id != key.0.as_str()) {
                    continue;
                }
                if latest_wins_key(queued) == Some(key.clone()) {
                    merge_latest(queued, outbound);
                    return QueuePushResult::Superseded;
                }
                // Any intervening delta for this subscription is an ordering
                // barrier. Merging newer events into an older queue slot would
                // make the browser receive them before this message.
                break;
            }
        }

        if queue.len() < self.capacity {
            queue.push_back(outbound);
            return QueuePushResult::Queued;
        }

        if let Some(index) = queue.iter().position(is_latest_wins) {
            queue.remove(index);
            queue.push_back(outbound);
            return QueuePushResult::Superseded;
        }

        QueuePushResult::Full
    }

    pub(super) fn pop(&self) -> Option<WsOutbound> {
        self.queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop_front()
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.queue.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).len()
    }
}

fn is_latest_wins(outbound: &WsOutbound) -> bool {
    latest_wins_key(outbound).is_some() || matches!(outbound, WsOutbound::Ping(_))
}

fn outbound_subscription_id(outbound: &WsOutbound) -> Option<&str> {
    match outbound {
        WsOutbound::Message(
            UiServerMessage::Delta { subscription_id, .. } | UiServerMessage::ResyncRequired { subscription_id, .. },
        ) => Some(subscription_id),
        _ => None,
    }
}

fn latest_wins_key(outbound: &WsOutbound) -> Option<(String, UiDataPlane)> {
    let WsOutbound::Message(UiServerMessage::Delta {
        subscription_id,
        deltas,
    }) = outbound
    else {
        return None;
    };
    let [delta] = deltas.as_slice() else {
        return None;
    };
    delta
        .plane
        .is_latest_wins()
        .then(|| (subscription_id.clone(), delta.plane))
}

fn merge_latest(existing: &mut WsOutbound, replacement: WsOutbound) {
    let WsOutbound::Message(UiServerMessage::Delta {
        deltas: existing_deltas,
        ..
    }) = existing
    else {
        *existing = replacement;
        return;
    };
    let WsOutbound::Message(UiServerMessage::Delta { mut deltas, .. }) = replacement else {
        return;
    };
    let [existing_delta] = existing_deltas.as_mut_slice() else {
        return;
    };
    let Some(replacement_delta) = deltas.pop() else {
        return;
    };
    if !deltas.is_empty() {
        return;
    }

    if existing_delta.plane == UiDataPlane::Preview {
        merge_keyed_preview_events(existing_delta, replacement_delta);
        return;
    }

    if existing_delta.plane != UiDataPlane::Value {
        *existing_delta = replacement_delta;
        return;
    }

    let mut parameter_indexes = HashMap::new();
    for (index, event) in existing_delta.batch.events.iter().enumerate() {
        if let UiEventKind::ParamChanged { param, .. } = &event.kind {
            parameter_indexes.insert(*param, index);
        }
    }
    for event in replacement_delta.batch.events {
        if let UiEventKind::ParamChanged { param, .. } = &event.kind
            && let Some(index) = parameter_indexes.get(param).copied()
        {
            existing_delta.batch.events[index] = event;
            continue;
        }
        if let UiEventKind::ParamChanged { param, .. } = &event.kind {
            parameter_indexes.insert(*param, existing_delta.batch.events.len());
        }
        existing_delta.batch.events.push(event);
    }
    existing_delta.batch.to = replacement_delta.batch.to.or(existing_delta.batch.to);
    existing_delta.batch.runtime = replacement_delta.batch.runtime.or(existing_delta.batch.runtime);
}

fn merge_keyed_preview_events(
    existing: &mut golden_protocol::UiPlaneDelta,
    replacement: golden_protocol::UiPlaneDelta,
) {
    let mut preview_indexes = HashMap::new();
    for (index, event) in existing.batch.events.iter().enumerate() {
        if let UiEventKind::Custom { topic, origin, .. } = &event.kind {
            preview_indexes.insert((topic.clone(), *origin), index);
        }
    }
    for event in replacement.batch.events {
        if let UiEventKind::Custom { topic, origin, .. } = &event.kind
            && let Some(index) = preview_indexes.get(&(topic.clone(), *origin)).copied()
        {
            existing.batch.events[index] = event;
            continue;
        }
        if let UiEventKind::Custom { topic, origin, .. } = &event.kind {
            preview_indexes.insert((topic.clone(), *origin), existing.batch.events.len());
        }
        existing.batch.events.push(event);
    }
    existing.batch.to = replacement.batch.to.or(existing.batch.to);
    existing.batch.runtime = replacement.batch.runtime.or(existing.batch.runtime);
}
