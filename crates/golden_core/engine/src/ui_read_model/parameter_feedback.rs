use super::UiReadModel;
use crate::{engine::EngineTime, node::NodeId, parameter::ParamValue};
use golden_protocol::{UiEventKind, UiGraphOp, UiReplayCursor};

/// One committed parameter value or trigger pulse from the retained event log.
pub struct PublishedParameterChange {
    /// Node that produced this event.
    pub node: NodeId,
    /// Committed value, after validation and runtime evaluation.
    pub value: ParamValue,
}

/// A filtered observation window that also identifies possible address changes.
pub struct PublishedParameterFeedback {
    /// Last observed event, including events excluded by the parameter filter.
    pub through: EngineTime,
    /// Node paths might have changed in this window.
    pub topology_changed: bool,
    /// Matching parameter events in publication order.
    pub changes: Vec<PublishedParameterChange>,
}

impl UiReadModel {
    /// Reads committed parameter updates without cloning unrelated graph deltas or log payloads.
    /// A stale project or an evicted cursor requires the consumer to reestablish its baseline.
    /// The filter executes under the replay lock and must be a nonblocking lookup.
    pub fn parameter_feedback_after(
        &self,
        from: UiReplayCursor,
        includes: impl Fn(NodeId, EngineTime, &ParamValue) -> bool,
    ) -> Option<PublishedParameterFeedback> {
        let before = self.current_cursor();
        if before.project_generation != from.project_generation || from.at > before.at {
            return None;
        }
        let log = self.events.lock().expect("ui event log poisoned");
        if self.last_evicted_event_time().is_some_and(|evicted| evicted > from.at) {
            return None;
        }
        let mut result = PublishedParameterFeedback {
            through: from.at,
            topology_changed: false,
            changes: Vec::new(),
        };
        for event in log.iter_after(Some(from.at)) {
            result.through = event.time;
            let mut append = |node, value: &ParamValue| {
                if includes(node, event.time, value) {
                    result.changes.push(PublishedParameterChange {
                        node,
                        value: value.clone(),
                    });
                }
            };
            match &event.kind {
                UiEventKind::ParamChanged { param, new_value, .. } => append(*param, new_value),
                UiEventKind::GraphTransaction { transaction } => {
                    for op in &transaction.ops {
                        match op {
                            UiGraphOp::ParamPatched { param, patch, .. } => {
                                if let Some(value) = &patch.value {
                                    append(*param, value);
                                }
                            }
                            UiGraphOp::NodeMetaPatched { patch, .. } => {
                                result.topology_changed |= patch.short_name.is_some()
                            }
                            UiGraphOp::NodeCreated { .. }
                            | UiGraphOp::ChildrenInserted { .. }
                            | UiGraphOp::SubtreeRemoved { .. }
                            | UiGraphOp::NodeMoved { .. } => result.topology_changed = true,
                            _ => {}
                        }
                    }
                }
                UiEventKind::ChildAdded { .. }
                | UiEventKind::ChildRemoved { .. }
                | UiEventKind::ChildReplaced { .. }
                | UiEventKind::ChildMoved { .. }
                | UiEventKind::NodeCreated { .. }
                | UiEventKind::NodeDeleted { .. } => {
                    result.topology_changed = true;
                }
                UiEventKind::MetaChanged { patch, .. } => result.topology_changed |= patch.short_name.is_some(),
                _ => {}
            }
        }
        drop(log);
        (self.current_cursor().project_generation == from.project_generation).then_some(result)
    }
}
