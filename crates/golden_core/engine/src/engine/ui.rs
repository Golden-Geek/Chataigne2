use crate::events::{CustomEvent, Event, EventKind};
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
                ops,
            },
        });
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
        if let Some(param) = self.ui_coalescable_param_value_event(&event) {
            let mut previous_index = None;
            for index in (self.ui_event_log_start..self.ui_event_log.len()).rev() {
                let Some(existing_param) = self.ui_coalescable_param_value_event(&self.ui_event_log[index]) else {
                    break;
                };
                if existing_param == param {
                    previous_index = Some(index);
                    break;
                }
            }

            let mut event = event;
            if let Some(index) = previous_index {
                let previous = self.ui_event_log.remove(index);
                preserve_param_changed_old_value(&mut event.kind, previous.kind);
            }
            self.ui_event_log.push(event);
            self.trim_ui_event_log();
            return;
        }

        self.ui_event_log.push(event);
        self.trim_ui_event_log();
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
