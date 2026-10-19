use std::collections::VecDeque;

use golden_core::{
    node::NodeId,
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use super::{
    admission::{PreparedMessages, MAX_STREAM_BYTES, MAX_STREAM_MESSAGES},
    commands::STREAMING_COMMAND_NODE_TYPES,
    parser::{StreamingParseConfig, StreamingSeparator},
};
use crate::app::module::common::received_values::service::{ReceivedValueCursor, ReceivedValueProgress};
#[cfg(test)]
use crate::app::module::common::received_values::ReceivedValueMessage;

struct PendingStreamBatch {
    messages: PreparedMessages,
    cursor: ReceivedValueCursor,
    weight: usize,
}

pub(crate) struct StreamingIncomingQueue {
    batches: VecDeque<PendingStreamBatch>,
    retained_items: usize,
    retained_weight: usize,
    maximum_items: usize,
    maximum_weight: usize,
    dropped_messages: u64,
    failed_batches: u64,
}

impl StreamingIncomingQueue {
    pub(crate) fn new() -> Self {
        Self::with_capacity(MAX_STREAM_MESSAGES, MAX_STREAM_BYTES)
    }

    fn with_capacity(maximum_items: usize, maximum_weight: usize) -> Self {
        Self {
            batches: VecDeque::new(),
            retained_items: 0,
            retained_weight: 0,
            maximum_items,
            maximum_weight,
            dropped_messages: 0,
            failed_batches: 0,
        }
    }

    #[cfg(test)]
    pub(super) fn with_limits(maximum_items: usize, maximum_weight: usize) -> Self {
        Self::with_capacity(maximum_items, maximum_weight)
    }

    /// Admit a whole parse batch or explicitly reject it; never keep a silent partial suffix.
    pub(crate) fn push_messages(&mut self, messages: PreparedMessages) {
        if messages.is_empty() {
            return;
        }
        let weight = std::mem::size_of::<PendingStreamBatch>() + messages.retained_bytes();
        let next_items = self.retained_items.checked_add(messages.len());
        let next_weight = self.retained_weight.checked_add(weight);
        if next_items.is_none_or(|items| items > self.maximum_items)
            || next_weight.is_none_or(|bytes| bytes > self.maximum_weight)
        {
            self.dropped_messages = self.dropped_messages.saturating_add(messages.len() as u64);
            return;
        }
        self.retained_items = next_items.expect("checked admission");
        self.retained_weight = next_weight.expect("checked admission");
        self.batches.push_back(PendingStreamBatch {
            messages,
            cursor: ReceivedValueCursor::default(),
            weight,
        });
    }

    pub(crate) fn take_dropped_message_count(&mut self) -> u64 {
        std::mem::take(&mut self.dropped_messages)
    }

    pub(crate) fn take_failed_batch_count(&mut self) -> u64 {
        std::mem::take(&mut self.failed_batches)
    }

    #[cfg(test)]
    pub(super) fn pending_message_count(&self) -> usize {
        self.retained_items
    }

    #[cfg(test)]
    pub(super) fn take_pending_messages_for_test(&mut self) -> Vec<ReceivedValueMessage> {
        self.retained_items = 0;
        self.retained_weight = 0;
        self.batches
            .drain(..)
            .flat_map(|batch| batch.messages.to_vec())
            .collect()
    }

    pub(crate) fn has_pending_messages(&self) -> bool {
        !self.batches.is_empty() || self.dropped_messages > 0 || self.failed_batches > 0
    }

    pub(crate) fn process_pending(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        values_id: Option<NodeId>,
        auto_add: bool,
    ) -> bool {
        let Some(batch) = self.batches.front_mut() else {
            return false;
        };
        let progress = batch
            .cursor
            .service(&batch.messages, ctx, Some(snapshot), values_id, auto_add, false);
        if progress != ReceivedValueProgress::Pending {
            let batch = self.batches.pop_front().expect("serviced batch");
            self.retained_items -= batch.messages.len();
            self.retained_weight -= batch.weight;
            if progress == ReceivedValueProgress::Failed {
                // A failed receipt may include an applied prefix. Abandon the suffix, never replay it.
                self.failed_batches = self.failed_batches.saturating_add(1);
            }
        }
        self.has_pending_messages()
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
