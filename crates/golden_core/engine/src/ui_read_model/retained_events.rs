//! Ordered, indexed retention for UI replay events.
//!
//! The replay log has two independent replacement policies:
//! `Latest` custom events replace the previous event with the same topic/origin
//! anywhere in the retained window, while coalescable parameter events replace
//! only within the current uninterrupted value run. The indexes below preserve
//! those semantics without reverse-scanning the retained window on every append.

use std::collections::{BTreeMap, HashMap};
use std::ops::Bound::{Excluded, Unbounded};

use crate::engine::EngineTime;
use crate::events::CustomEventRetention;
use crate::node::NodeId;
use crate::parameter::{ParamValue, ParameterEventBehaviour};
use crate::ui_sync::{UiEventDto, UiEventKind, UiNodeDataDto, UiNodeDto};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RetainedEventKey {
    time: EngineTime,
    ordinal: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LatestCustomKey {
    topic: String,
    origin: Option<NodeId>,
}

#[derive(Clone, Debug)]
enum ReplacementKey {
    LatestCustom(LatestCustomKey),
    PendingParam(NodeId),
    None,
}

struct RetainedEvent {
    event: UiEventDto,
    replacement: ReplacementKey,
}

/// A time-ordered replay log with constant-time replacement-key lookup.
#[derive(Default)]
pub(super) struct RetainedEventLog {
    events: BTreeMap<RetainedEventKey, RetainedEvent>,
    latest_custom: HashMap<LatestCustomKey, RetainedEventKey>,
    pending_params: HashMap<NodeId, RetainedEventKey>,
    next_ordinal: u64,
}

impl RetainedEventLog {
    pub(super) fn clear(&mut self) {
        self.events.clear();
        self.latest_custom.clear();
        self.pending_params.clear();
        self.next_ordinal = 0;
    }

    pub(super) fn first_event_time(&self) -> Option<EngineTime> {
        self.events.first_key_value().map(|(_, retained)| retained.event.time)
    }

    /// Returns a cloned suffix after a logarithmic seek to the first event newer
    /// than `from`. Events sharing the cursor time are excluded as a group.
    pub(super) fn events_after(&self, from: Option<EngineTime>) -> Vec<UiEventDto> {
        match from {
            Some(cursor) => self
                .events
                .range((
                    Excluded(RetainedEventKey {
                        time: cursor,
                        ordinal: u64::MAX,
                    }),
                    Unbounded,
                ))
                .map(|(_, retained)| retained.event.clone())
                .collect(),
            None => self.events.values().map(|retained| retained.event.clone()).collect(),
        }
    }

    /// Appends one event and returns the highest time evicted by the capacity limit.
    pub(super) fn append(
        &mut self,
        nodes: &HashMap<NodeId, UiNodeDto>,
        mut event: UiEventDto,
        capacity: usize,
    ) -> Option<EngineTime> {
        debug_assert!(
            self.events
                .last_key_value()
                .is_none_or(|(_, retained)| retained.event.time <= event.time),
            "UI replay events must be published in monotonic EngineTime order"
        );

        let replacement = replacement_key(nodes, &event);
        match &replacement {
            ReplacementKey::LatestCustom(custom_key) => {
                self.pending_params.clear();
                if let Some(previous_key) = self.latest_custom.remove(custom_key) {
                    self.remove_replaced(previous_key);
                }
            }
            ReplacementKey::PendingParam(param) => {
                if let Some(previous_key) = self.pending_params.remove(param)
                    && let Some(previous) = self.remove_replaced(previous_key)
                {
                    preserve_param_changed_old_value(&mut event.kind, previous.event.kind);
                }
            }
            ReplacementKey::None => self.pending_params.clear(),
        }

        let key = RetainedEventKey {
            time: event.time,
            ordinal: self.next_ordinal,
        };
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .expect("UI replay event ordinal exhausted");
        self.events.insert(
            key,
            RetainedEvent {
                event,
                replacement: replacement.clone(),
            },
        );
        match replacement {
            ReplacementKey::LatestCustom(custom_key) => {
                self.latest_custom.insert(custom_key, key);
            }
            ReplacementKey::PendingParam(param) => {
                self.pending_params.insert(param, key);
            }
            ReplacementKey::None => {}
        }

        let mut evicted_through: Option<EngineTime> = None;
        while self.events.len() > capacity {
            let Some((evicted_key, evicted)) = self.events.pop_first() else {
                break;
            };
            self.remove_indexes_for(evicted_key, &evicted.replacement);
            evicted_through = Some(evicted_through.map_or(evicted.event.time, |time| time.max(evicted.event.time)));
        }
        evicted_through
    }

    fn remove_replaced(&mut self, key: RetainedEventKey) -> Option<RetainedEvent> {
        let retained = self.events.remove(&key)?;
        self.remove_indexes_for(key, &retained.replacement);
        Some(retained)
    }

    fn remove_indexes_for(&mut self, key: RetainedEventKey, replacement: &ReplacementKey) {
        match replacement {
            ReplacementKey::LatestCustom(custom_key)
                if self
                    .latest_custom
                    .get(custom_key)
                    .is_some_and(|indexed| *indexed == key) =>
            {
                self.latest_custom.remove(custom_key);
            }
            ReplacementKey::PendingParam(param)
                if self.pending_params.get(param).is_some_and(|indexed| *indexed == key) =>
            {
                self.pending_params.remove(param);
            }
            _ => {}
        }
    }
}

pub(super) fn event_is_coalescable_value(nodes: &HashMap<NodeId, UiNodeDto>, event: &UiEventDto) -> bool {
    coalescable_param(nodes, event).is_some()
}

fn replacement_key(nodes: &HashMap<NodeId, UiNodeDto>, event: &UiEventDto) -> ReplacementKey {
    if let UiEventKind::Custom {
        topic,
        origin,
        retention: CustomEventRetention::Latest,
        ..
    } = &event.kind
    {
        return ReplacementKey::LatestCustom(LatestCustomKey {
            topic: topic.clone(),
            origin: *origin,
        });
    }

    coalescable_param(nodes, event).map_or(ReplacementKey::None, ReplacementKey::PendingParam)
}

fn coalescable_param(nodes: &HashMap<NodeId, UiNodeDto>, event: &UiEventDto) -> Option<NodeId> {
    let UiEventKind::ParamChanged { param, new_value, .. } = &event.kind else {
        return None;
    };
    if matches!(new_value, ParamValue::Trigger()) {
        return None;
    }

    let node = nodes.get(param)?;
    let UiNodeDataDto::Parameter { param: param_dto } = &node.data else {
        return None;
    };
    (param_dto.event_behaviour == ParameterEventBehaviour::Coalesce).then_some(*param)
}

fn preserve_param_changed_old_value(new_kind: &mut UiEventKind, previous_kind: UiEventKind) {
    let (
        UiEventKind::ParamChanged {
            old_value: new_old_value,
            ..
        },
        UiEventKind::ParamChanged {
            old_value: previous_old_value,
            ..
        },
    ) = (new_kind, previous_kind)
    else {
        return;
    };
    *new_old_value = previous_old_value;
}
