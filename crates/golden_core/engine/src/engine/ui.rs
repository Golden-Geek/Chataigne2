use std::collections::{HashMap, HashSet};

use crate::events::{CustomEvent, CustomEventRetention, Event, EventKind};
use crate::node::{Node, NodeId};
use crate::parameter::{ParamValue, ParameterEventBehaviour};
use crate::ui_sync::{UiChildrenOrderPatch, UiGraphOp, UiGraphTransaction};

use super::{Engine, EngineTime};

/// Default retention size for the UI replay event log.
pub const DEFAULT_UI_EVENT_LOG_CAPACITY: usize = 8192;
const UI_EVENT_LOG_COMPACT_THRESHOLD: usize = 4096;

impl<T: Node> Engine<T> {
    /// Returns the retained UI event replay buffer.
    pub fn ui_event_log(&self) -> &[Event] {
        &self.ui_event_log[self.ui_event_log_start..]
    }

    /// Returns the current UI replay buffer capacity.
    pub fn ui_event_log_capacity(&self) -> usize {
        self.ui_event_log_capacity
    }

    /// Updates the UI replay buffer capacity, trimming oldest events when needed.
    pub fn set_ui_event_log_capacity(&mut self, capacity: usize) {
        self.ui_event_log_capacity = capacity.max(1);
        self.trim_ui_event_log();
    }

    pub(crate) fn ui_event_log_start_index(&self, after: Option<EngineTime>) -> usize {
        let retained = self.ui_event_log();
        match after {
            Some(after_time) => retained.partition_point(|event| event.time <= after_time),
            None => 0,
        }
    }

    /// Returns cloned events newer than `after`.
    pub fn ui_events_since(&self, after: Option<EngineTime>) -> Vec<Event> {
        let start_index = self.ui_event_log_start_index(after);
        self.ui_event_log()[start_index..].to_vec()
    }

    /// Clears the UI replay buffer.
    pub fn clear_ui_event_log(&mut self) {
        self.ui_event_log.clear();
        self.ui_event_log_start = 0;
        self.ui_latest_event_times.clear();
        self.ui_pending_param_event_times.clear();
    }

    /// Pushes a custom UI event into the replay log.
    pub fn push_ui_custom_event(
        &mut self,
        topic: impl Into<String>,
        origin: Option<crate::node::NodeId>,
        payload: serde_json::Value,
    ) {
        let event = Event {
            time: self.time,
            kind: EventKind::Custom(CustomEvent::new(topic, origin, payload)),
        };
        self.push_ui_event_log(event);
        self.time.seq = self.time.seq.saturating_add(1);
    }

    pub(crate) fn push_ui_event_kind(&mut self, kind: EventKind) {
        let event = Event { time: self.time, kind };
        self.push_ui_event_log(event);
        self.time.seq = self.time.seq.saturating_add(1);
    }

    pub(crate) fn push_ui_graph_transaction(&mut self, ops: Vec<UiGraphOp>) {
        if ops.is_empty() {
            return;
        }

        // A transaction is applied atomically. When many roots share a parent, only its last
        // child-order patch matters; repeating the complete sibling list on every insertion
        // makes a large paste quadratic in the destination's existing child count.
        let mut ordered_parents = HashSet::new();
        let mut compacted = Vec::with_capacity(ops.len());
        for mut op in ops.into_iter().rev() {
            match &mut op {
                UiGraphOp::SubtreeInserted {
                    parent,
                    parent_children_after,
                    ..
                } => {
                    if parent_children_after.is_some() && !ordered_parents.insert(*parent) {
                        *parent_children_after = None;
                    }
                }
                UiGraphOp::ChildrenReordered { parent, .. } if !ordered_parents.insert(*parent) => continue,
                UiGraphOp::ChildrenReordered { .. } => {}
                _ => {}
            }
            compacted.push(op);
        }
        compacted.reverse();

        // An insertion-only transaction can describe a contiguous group of new direct children
        // with one splice. Mixed edits or interleaved insertions retain the exact full order.
        if compacted
            .iter()
            .all(|op| matches!(op, UiGraphOp::SubtreeInserted { .. }))
        {
            let mut parents = Vec::new();
            let mut roots_by_parent: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
            for op in &compacted {
                if let UiGraphOp::SubtreeInserted { root, parent, .. } = op {
                    if !roots_by_parent.contains_key(parent) {
                        parents.push(*parent);
                    }
                    roots_by_parent.entry(*parent).or_default().push(*root);
                }
            }
            let mut insertions = Vec::new();
            for parent in parents {
                let roots = &roots_by_parent[&parent];
                let Some((order_op_index, order)) = compacted.iter().enumerate().rev().find_map(|(index, op)| {
                    if let UiGraphOp::SubtreeInserted {
                        parent: op_parent,
                        parent_children_after: Some(order),
                        ..
                    } = op
                        && *op_parent == parent
                    {
                        Some((index, order))
                    } else {
                        None
                    }
                }) else {
                    continue;
                };
                let root_set: HashSet<_> = roots.iter().copied().collect();
                if root_set.len() != roots.len() || order.len() < roots.len() {
                    continue;
                }
                let Some(index) = order.iter().position(|child| root_set.contains(child)) else {
                    continue;
                };
                let Some(inserted) = order.get(index..index + roots.len()) else {
                    continue;
                };
                if inserted.iter().copied().collect::<HashSet<_>>() != root_set
                    || order.iter().filter(|child| root_set.contains(child)).count() != roots.len()
                {
                    continue;
                }
                let insertion = UiGraphOp::ChildrenInserted {
                    parent,
                    expected_before_count: order.len() - roots.len(),
                    index,
                    children: inserted.to_vec(),
                };
                if let UiGraphOp::SubtreeInserted {
                    parent_children_after, ..
                } = &mut compacted[order_op_index]
                {
                    *parent_children_after = None;
                }
                insertions.push(insertion);
            }
            compacted.extend(insertions);
        }

        let base_graph_version = self.ui_graph_version;
        let next_graph_version = base_graph_version.saturating_add(1);
        self.ui_graph_version = next_graph_version;

        let tx_id = self.next_ui_tx_id;
        self.next_ui_tx_id = self.next_ui_tx_id.saturating_add(1);

        self.push_ui_event_kind(EventKind::GraphTransaction {
            transaction: UiGraphTransaction {
                tx_id,
                epoch: self.ui_epoch,
                base_graph_version,
                next_graph_version,
                ops: compacted,
            },
        });
    }

    /// The materializing graph transaction already contains the final parameter and metadata
    /// state produced by loaded-node lifecycle callbacks. Publishing those earlier patches would
    /// make a client apply them before the new nodes exist. Triggers carry an edge, not snapshot
    /// state, so callers must publish them again after the transaction.
    pub(crate) fn squash_pre_materialization_ui_events(&mut self, inserted: &HashSet<NodeId>) -> Vec<EventKind> {
        if inserted.is_empty() {
            return Vec::new();
        }

        self.ui_event_log.drain(..self.ui_event_log_start);
        self.ui_event_log_start = 0;
        let mut deferred_triggers = Vec::new();
        self.ui_event_log.retain(|event| {
            let owned = match &event.kind {
                EventKind::ParamChanged { param, .. }
                | EventKind::ParamControlChanged { param, .. }
                | EventKind::ParamConstraintsChanged { param, .. } => inserted.contains(param),
                EventKind::MetaChanged { node, .. } => inserted.contains(node),
                _ => false,
            };
            if owned
                && matches!(
                    &event.kind,
                    EventKind::ParamChanged {
                        new_value: ParamValue::Trigger(),
                        ..
                    }
                )
            {
                deferred_triggers.push(event.kind.clone());
            }
            !owned
        });
        self.ui_pending_param_event_times
            .retain(|param, _| !inserted.contains(param));
        deferred_triggers
    }

    pub(crate) fn ui_children_order_patch(&self, parent: NodeId) -> Option<UiChildrenOrderPatch> {
        Some(UiChildrenOrderPatch {
            parent,
            children: self.ui_direct_children(parent)?,
        })
    }

    pub(crate) fn ui_child_index(&self, parent: NodeId, child: NodeId) -> Option<usize> {
        self.ui_direct_children(parent)?
            .into_iter()
            .position(|candidate| candidate == child)
    }

    /// Flushes newly buffered logger records into the UI event replay log.
    pub fn sync_logger_ui_events(&mut self) {
        let records = crate::logger::records_since_cursor(
            self.last_synced_logger_record_id,
            self.last_synced_logger_repeat_count,
        );
        for record in &records {
            if let Ok(payload) = serde_json::to_value(record) {
                self.push_ui_custom_event(crate::logger::UI_LOG_RECORD_TOPIC, record.origin, payload);
            }
        }
        if let Some(record) = records.last() {
            self.last_synced_logger_record_id = record.id;
            self.last_synced_logger_repeat_count = record.repeat_count;
        }
    }

    pub(crate) fn push_ui_event_log(&mut self, event: Event) {
        if let Some((topic, origin)) = ui_latest_custom_event_key(&event) {
            let key = (topic.to_owned(), origin);
            self.ui_pending_param_event_times.clear();
            let previous_index = self
                .ui_latest_event_times
                .get(&key)
                .copied()
                .and_then(|time| self.ui_event_log_index_at(time))
                .filter(|index| {
                    ui_latest_custom_event_key(&self.ui_event_log[*index])
                        .is_some_and(|existing| existing == (key.0.as_str(), key.1))
                });
            if let Some(index) = previous_index {
                self.ui_event_log.remove(index);
            }
            self.ui_latest_event_times.insert(key, event.time);
            self.ui_event_log.push(event);
            self.trim_ui_event_log();
            return;
        }

        if let Some(param) = self.ui_coalescable_param_value_event(&event) {
            let previous_index = self
                .ui_pending_param_event_times
                .get(&param)
                .copied()
                .and_then(|time| self.ui_event_log_index_at(time))
                .filter(|index| self.ui_coalescable_param_value_event(&self.ui_event_log[*index]) == Some(param));
            let mut event = event;
            if let Some(index) = previous_index {
                let previous = self.ui_event_log.remove(index);
                preserve_param_changed_old_value(&mut event.kind, previous.kind);
            }
            self.ui_pending_param_event_times.insert(param, event.time);
            self.ui_event_log.push(event);
            self.trim_ui_event_log();
            return;
        }

        self.ui_pending_param_event_times.clear();
        self.ui_event_log.push(event);
        self.trim_ui_event_log();
    }

    fn ui_event_log_index_at(&self, time: EngineTime) -> Option<usize> {
        let retained = self.ui_event_log();
        let retained_index = retained.partition_point(|event| event.time < time);
        retained
            .get(retained_index)
            .is_some_and(|event| event.time == time)
            .then_some(self.ui_event_log_start + retained_index)
    }

    fn ui_coalescable_param_value_event(&self, event: &Event) -> Option<NodeId> {
        let EventKind::ParamChanged { param, new_value, .. } = &event.kind else {
            return None;
        };
        if matches!(new_value, ParamValue::Trigger()) {
            return None;
        }

        let snapshot = self.nodes.get(*param)?.engine_param_snapshot()?;
        (snapshot.event_behaviour == ParameterEventBehaviour::Coalesce).then_some(*param)
    }

    fn trim_ui_event_log(&mut self) {
        let retained_len = self.ui_event_log.len().saturating_sub(self.ui_event_log_start);
        if retained_len > self.ui_event_log_capacity {
            let overflow = retained_len - self.ui_event_log_capacity;
            let eviction_end = self.ui_event_log_start + overflow;
            for index in self.ui_event_log_start..eviction_end {
                let event = &self.ui_event_log[index];
                let time = event.time;
                let latest_key = ui_latest_custom_event_key(event).map(|(topic, origin)| (topic.to_owned(), origin));
                let param = match &event.kind {
                    EventKind::ParamChanged { param, .. } => Some(*param),
                    _ => None,
                };
                if let Some(key) = latest_key
                    && self
                        .ui_latest_event_times
                        .get(&key)
                        .is_some_and(|indexed| *indexed == time)
                {
                    self.ui_latest_event_times.remove(&key);
                }
                if let Some(param) = param
                    && self
                        .ui_pending_param_event_times
                        .get(&param)
                        .is_some_and(|indexed| *indexed == time)
                {
                    self.ui_pending_param_event_times.remove(&param);
                }
            }
            self.ui_event_log_start = self.ui_event_log_start.saturating_add(overflow);
        }

        if self.ui_event_log_start == 0 {
            return;
        }

        if self.ui_event_log_start >= UI_EVENT_LOG_COMPACT_THRESHOLD
            || self.ui_event_log_start * 2 >= self.ui_event_log.len()
        {
            self.ui_event_log.drain(0..self.ui_event_log_start);
            self.ui_event_log_start = 0;
        }
    }

    #[cfg(test)]
    pub(crate) fn ui_event_index_sizes_for_tests(&self) -> (usize, usize) {
        (
            self.ui_latest_event_times.len(),
            self.ui_pending_param_event_times.len(),
        )
    }
}

fn ui_latest_custom_event_key(event: &Event) -> Option<(&str, Option<NodeId>)> {
    let EventKind::Custom(event) = &event.kind else {
        return None;
    };
    (event.retention == CustomEventRetention::Latest).then_some((event.topic.as_str(), event.origin))
}

fn preserve_param_changed_old_value(new_kind: &mut EventKind, previous_kind: EventKind) {
    let (
        EventKind::ParamChanged {
            old_value: new_old_value,
            ..
        },
        EventKind::ParamChanged {
            old_value: previous_old_value,
            ..
        },
    ) = (new_kind, previous_kind)
    else {
        return;
    };

    *new_old_value = previous_old_value;
}
