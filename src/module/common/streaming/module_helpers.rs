use std::collections::HashSet;

use golden_core::{
    node::NodeId,
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::received_values::{
    apply_received_value_payload, ReceivedValueApplyOptions, ReceivedValueApplyResult,
};

use super::{
    commands::STREAMING_COMMAND_NODE_TYPES,
    parser::{StreamingIncomingMessage, StreamingParseConfig, StreamingSeparator},
};

pub(crate) struct StreamingIncomingQueue {
    ignored_param_changes: HashSet<NodeId>,
    pending_messages: Vec<StreamingIncomingMessage>,
}

impl StreamingIncomingQueue {
    pub(crate) fn new() -> Self {
        Self {
            ignored_param_changes: HashSet::new(),
            pending_messages: Vec::new(),
        }
    }

    pub(crate) fn ignore_next_param_change(&mut self, param: NodeId) {
        self.ignored_param_changes.insert(param);
    }

    pub(crate) fn take_ignored_param_change(&mut self, param: NodeId) -> bool {
        self.ignored_param_changes.remove(&param)
    }

    pub(crate) fn push_messages(&mut self, messages: Vec<StreamingIncomingMessage>) {
        self.pending_messages.extend(messages);
    }

    pub(crate) fn has_pending_messages(&self) -> bool {
        !self.pending_messages.is_empty()
    }

    pub(crate) fn process_pending(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        values_id: Option<NodeId>,
        auto_add: bool,
    ) -> bool {
        let Some(values_id) = values_id else {
            self.pending_messages.clear();
            return false;
        };
        if self.pending_messages.is_empty() {
            return false;
        }

        let mut remaining = Vec::new();
        let mut messages = std::mem::take(&mut self.pending_messages).into_iter();

        while let Some(message) = messages.next() {
            let result = apply_received_value_payload(
                ctx,
                snapshot,
                values_id,
                message.path_segments.as_slice(),
                &message.payload,
                ReceivedValueApplyOptions {
                    auto_add,
                    //source_description: message.source_description.as_str(),
                    event_behaviour: ParameterEventBehaviour::Append,
                },
            );

            match result {
                ReceivedValueApplyResult::Applied {
                    needs_snapshot_refresh,
                } => {
                    for param in changed_parameter_targets(snapshot, values_id, message.path_segments.as_slice()) {
                        self.ignore_next_param_change(param);
                    }
                    if needs_snapshot_refresh {
                        remaining.extend(messages);
                        self.pending_messages = remaining;
                        return true;
                    }
                }
                ReceivedValueApplyResult::Ignored => {}
                ReceivedValueApplyResult::Retry => {
                    remaining.push(message);
                    remaining.extend(messages);
                    self.pending_messages = remaining;
                    return true;
                }
            }
        }

        self.pending_messages = remaining;
        false
    }
}

pub(crate) fn streaming_parse_config(
    mode: &str,
    name_separator: Option<&str>,
    value_separator: Option<&str>,
    hierarchy_separator: Option<&str>,
) -> StreamingParseConfig {
    StreamingParseConfig {
        mode: mode.to_string(),
        name_separator: name_separator.and_then(StreamingSeparator::from_variant),
        value_separator: value_separator.and_then(StreamingSeparator::from_variant),
        hierarchy_separator: hierarchy_separator.and_then(StreamingSeparator::from_variant),
    }
}

pub(crate) fn streaming_command_type_supported(command_type: &str) -> bool {
    STREAMING_COMMAND_NODE_TYPES.contains(&command_type)
}

pub(crate) fn child_string_param(snapshot: &ProcessTreeSnapshot, parent: NodeId, child_name: &str) -> Option<String> {
    snapshot.find_child_by_decl_id(parent, child_name).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
}

pub(crate) fn child_int_param(snapshot: &ProcessTreeSnapshot, parent: NodeId, child_name: &str) -> Option<i32> {
    snapshot.find_child_by_decl_id(parent, child_name).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_int)
    })
}

pub(crate) fn format_bytes_for_log(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "<empty>".to_string();
    }

    match std::str::from_utf8(bytes) {
        Ok(text) if !text.chars().any(char::is_control) => format!("\"{text}\""),
        _ => bytes
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn changed_parameter_targets(
    snapshot: &ProcessTreeSnapshot,
    values_id: NodeId,
    path_segments: &[String],
) -> Vec<NodeId> {
    let mut current = values_id;
    for segment in path_segments {
        let Some(child_id) = snapshot.find_child(current, segment.as_str()) else {
            return Vec::new();
        };
        current = child_id;
    }

    let Some(target) = snapshot.node(current) else {
        return Vec::new();
    };
    if target.param_value.is_some() {
        return vec![current];
    }

    snapshot
        .child_ids(current)
        .into_iter()
        .filter(|child_id| {
            snapshot
                .node(*child_id)
                .is_some_and(|child| child.param_value.is_some())
        })
        .collect()
}
