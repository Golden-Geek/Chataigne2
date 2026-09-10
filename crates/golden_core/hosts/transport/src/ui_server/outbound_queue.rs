use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

use golden_protocol::{UiDataPlane, UiEventKind, UiServerMessage};

use super::WsOutbound;

pub(super) const DEFAULT_OUTBOUND_CAPACITY: usize = 64;
pub(super) const DEFAULT_OUTBOUND_BYTES_CAPACITY: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum QueuePushResult {
    Queued,
    Superseded,
    Full,
}

pub(super) struct WsOutboundQueue {
    capacity: usize,
    bytes_capacity: usize,
    queue: Mutex<OutboundState>,
}

struct OutboundState {
    entries: VecDeque<WeightedOutbound>,
    retained_bytes: usize,
}

struct WeightedOutbound {
    outbound: WsOutbound,
    bytes: usize,
}

impl WsOutboundQueue {
    pub(super) fn new(capacity: usize) -> Self {
        Self::with_limits(capacity, DEFAULT_OUTBOUND_BYTES_CAPACITY)
    }

    pub(super) fn with_limits(capacity: usize, bytes_capacity: usize) -> Self {
        assert!(capacity > 0, "websocket outbound capacity must be non-zero");
        assert!(bytes_capacity > 0, "websocket outbound byte capacity must be non-zero");
        Self {
            capacity,
            bytes_capacity,
            queue: Mutex::new(OutboundState {
                entries: VecDeque::with_capacity(capacity),
                retained_bytes: 0,
            }),
        }
    }

    pub(super) fn push(&self, outbound: WsOutbound) -> QueuePushResult {
        let bytes = outbound_retained_bytes(&outbound);
        if bytes > self.bytes_capacity {
            return QueuePushResult::Full;
        }
        let mut state = self.queue.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        if matches!(outbound, WsOutbound::Close) {
            state.entries.clear();
            state.retained_bytes = bytes;
            state.entries.push_back(WeightedOutbound { outbound, bytes });
            return QueuePushResult::Queued;
        }

        if let Some(key) = latest_wins_key(&outbound) {
            for index in (0..state.entries.len()).rev() {
                let queued = &state.entries[index];
                if outbound_subscription_id(&queued.outbound)
                    .is_none_or(|subscription_id| subscription_id != key.0.as_str())
                {
                    continue;
                }
                if latest_wins_key(&queued.outbound) == Some(key.clone()) {
                    let mut merged = queued.outbound.clone();
                    merge_latest(&mut merged, outbound);
                    let merged_bytes = outbound_retained_bytes(&merged);
                    let retained_bytes = state.retained_bytes - queued.bytes;
                    if retained_bytes
                        .checked_add(merged_bytes)
                        .is_none_or(|next| next > self.bytes_capacity)
                    {
                        return QueuePushResult::Full;
                    }
                    state.retained_bytes = retained_bytes + merged_bytes;
                    state.entries[index] = WeightedOutbound {
                        outbound: merged,
                        bytes: merged_bytes,
                    };
                    return QueuePushResult::Superseded;
                }
                // Any intervening delta for this subscription is an ordering
                // barrier. Merging newer events into an older queue slot would
                // make the browser receive them before this message.
                break;
            }
        }

        let fits = |state: &OutboundState| {
            state.entries.len() < self.capacity
                && state
                    .retained_bytes
                    .checked_add(bytes)
                    .is_some_and(|next| next <= self.bytes_capacity)
        };
        if fits(&state) {
            state.retained_bytes += bytes;
            state.entries.push_back(WeightedOutbound { outbound, bytes });
            return QueuePushResult::Queued;
        }

        let (reclaimable_count, reclaimable_bytes) = state
            .entries
            .iter()
            .filter(|queued| is_latest_wins(&queued.outbound))
            .fold((0usize, 0usize), |(count, bytes), queued| {
                (count + 1, bytes + queued.bytes)
            });
        if state.entries.len() - reclaimable_count >= self.capacity
            || state.retained_bytes - reclaimable_bytes + bytes > self.bytes_capacity
        {
            return QueuePushResult::Full;
        }

        let mut superseded = false;
        while !fits(&state) {
            let Some(index) = state.entries.iter().position(|queued| is_latest_wins(&queued.outbound)) else {
                return QueuePushResult::Full;
            };
            let removed = state.entries.remove(index).expect("located outbound exists");
            state.retained_bytes -= removed.bytes;
            superseded = true;
        }
        state.retained_bytes += bytes;
        state.entries.push_back(WeightedOutbound { outbound, bytes });
        if superseded {
            return QueuePushResult::Superseded;
        }

        QueuePushResult::Queued
    }

    pub(super) fn pop(&self) -> Option<WsOutbound> {
        let mut state = self.queue.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let queued = state.entries.pop_front()?;
        state.retained_bytes -= queued.bytes;
        Some(queued.outbound)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entries
            .len()
    }

    #[cfg(test)]
    pub(super) fn retained_bytes(&self) -> usize {
        self.queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retained_bytes
    }
}

fn outbound_retained_bytes(outbound: &WsOutbound) -> usize {
    match outbound {
        WsOutbound::Message(message) => serde_json::to_vec(message).map_or(usize::MAX, |bytes| bytes.len()),
        WsOutbound::Ping(payload) => payload.len(),
        WsOutbound::Close => 1,
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
