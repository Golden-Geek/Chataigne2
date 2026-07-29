use std::collections::HashMap;

use golden_engine::engine::EngineTime;
use golden_engine::node::NodeId;
use golden_protocol::{UiEventBatch, UiEventDto, UiEventKind};

const MIN_STALE_SLOTS_BEFORE_COMPACTION: usize = 256;

/// Latest-wins parameter events waiting for the next value-plane flush.
///
/// Replacements leave holes and append the new event, preserving the previous
/// deterministic "last arrival wins and moves to the end" ordering without a
/// linear scan or vector shift for every update.
#[derive(Default)]
pub(super) struct PendingValueEvents {
    from: Option<EngineTime>,
    to: Option<EngineTime>,
    slots: Vec<Option<UiEventDto>>,
    param_indexes: HashMap<NodeId, usize>,
    live_len: usize,
    #[cfg(test)]
    operation_count: usize,
}

impl PendingValueEvents {
    pub(super) fn queue(&mut self, from: Option<EngineTime>, events: Vec<UiEventDto>) {
        if events.is_empty() {
            return;
        }
        if self.live_len == 0 {
            self.from = from;
        }

        for event in events {
            #[cfg(test)]
            {
                self.operation_count += 1;
            }

            let param = value_plane_param(&event);
            if let Some(param) = param
                && let Some(stale_index) = self.param_indexes.remove(&param)
                && self.slots[stale_index].take().is_some()
            {
                self.live_len -= 1;
            }

            self.to = Some(event.time);
            let index = self.slots.len();
            if let Some(param) = param {
                self.param_indexes.insert(param, index);
            }
            self.slots.push(Some(event));
            self.live_len += 1;
            self.compact_if_needed();
        }
    }

    pub(super) fn take_batch(&mut self) -> Option<UiEventBatch> {
        if self.live_len == 0 {
            return None;
        }

        let slots = std::mem::take(&mut self.slots);
        let mut events = Vec::with_capacity(self.live_len);
        events.extend(slots.into_iter().flatten());
        debug_assert_eq!(events.len(), self.live_len);
        self.param_indexes.clear();
        self.live_len = 0;

        Some(UiEventBatch {
            from: self.from.take(),
            to: self.to.take().or_else(|| events.last().map(|event| event.time)),
            runtime: None,
            events,
        })
    }

    pub(super) fn clear(&mut self) {
        self.from = None;
        self.to = None;
        self.slots.clear();
        self.param_indexes.clear();
        self.live_len = 0;
    }

    fn compact_if_needed(&mut self) {
        let stale_len = self.slots.len() - self.live_len;
        if stale_len < MIN_STALE_SLOTS_BEFORE_COMPACTION || stale_len < self.live_len {
            return;
        }

        let old_slots = std::mem::take(&mut self.slots);
        #[cfg(test)]
        {
            self.operation_count += old_slots.len();
        }
        self.slots.reserve(self.live_len);
        self.param_indexes.clear();
        for event in old_slots.into_iter().flatten() {
            let index = self.slots.len();
            if let Some(param) = value_plane_param(&event) {
                self.param_indexes.insert(param, index);
            }
            self.slots.push(Some(event));
        }
        debug_assert_eq!(self.slots.len(), self.live_len);
    }

    #[cfg(test)]
    pub(super) fn storage_len(&self) -> usize {
        self.slots.len()
    }

    #[cfg(test)]
    pub(super) fn operation_count(&self) -> usize {
        self.operation_count
    }
}

fn value_plane_param(event: &UiEventDto) -> Option<NodeId> {
    let UiEventKind::ParamChanged { param, .. } = &event.kind else {
        return None;
    };
    Some(*param)
}
