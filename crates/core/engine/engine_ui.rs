use crate::events::{CustomEvent, Event, EventKind};
use crate::node::Node;

use super::{Engine, EngineTime};

/// Default retention size for the UI replay event log.
pub const DEFAULT_UI_EVENT_LOG_CAPACITY: usize = 8192;

impl<T: Node> Engine<T> {
    /// Returns the retained UI event replay buffer.
    pub fn ui_event_log(&self) -> &[Event] {
        &self.ui_event_log
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

    /// Returns cloned events newer than `after`.
    pub fn ui_events_since(&self, after: Option<EngineTime>) -> Vec<Event> {
        match after {
            Some(after_time) => self.ui_event_log.iter().filter(|event| event.time > after_time).cloned().collect(),
            None => self.ui_event_log.clone(),
        }
    }

    /// Clears the UI replay buffer.
    pub fn clear_ui_event_log(&mut self) {
        self.ui_event_log.clear();
    }

    pub(crate) fn push_ui_custom_event(
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

    pub(crate) fn sync_logger_ui_events(&mut self) {
        for record in crate::logger::drain_pending() {
            if let Ok(payload) = serde_json::to_value(&record) {
                self.push_ui_custom_event(crate::logger::UI_LOG_RECORD_TOPIC, record.origin, payload);
            }
        }
    }

    pub(crate) fn push_ui_event_log(&mut self, event: Event) {
        self.ui_event_log.push(event);
        self.trim_ui_event_log();
    }

    fn trim_ui_event_log(&mut self) {
        if self.ui_event_log.len() > self.ui_event_log_capacity {
            let overflow = self.ui_event_log.len() - self.ui_event_log_capacity;
            self.ui_event_log.drain(0..overflow);
        }
    }
}
