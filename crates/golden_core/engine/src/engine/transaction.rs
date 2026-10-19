use std::collections::{HashMap, HashSet};

use super::edit_journal::JournalMode;
use super::{Engine, EngineEditError, EngineTime, PendingAddedSubtreeUiBatch};
use crate::edit::EditRequest;
use crate::node::{Node, NodeCreationContext, NodeId};
use crate::parameter::ParamValue;

pub(crate) trait EditOperationError: From<EngineEditError> {
    fn with_rollback(original: Self, rollback: EngineEditError) -> Self;
}

impl EditOperationError for EngineEditError {
    fn with_rollback(original: Self, rollback: EngineEditError) -> Self {
        Self::AtomicRollbackFailed {
            original: Box::new(original),
            rollback: Box::new(rollback),
        }
    }
}

/// The outer operation owns publication rollback; nested preparations join its journal.
pub(crate) struct EditCheckpoint {
    inbox: crate::events::Inbox,
    ui_event_log: Vec<crate::events::Event>,
    ui_event_log_start: usize,
    ui_latest_event_times: HashMap<(String, Option<NodeId>), EngineTime>,
    ui_pending_param_event_times: HashMap<NodeId, EngineTime>,
    ui_epoch: u64,
    next_ui_tx_id: u64,
    ui_graph_version: u64,
    time: EngineTime,
    pending_node_ready: Vec<(NodeId, NodeCreationContext)>,
    pending_added_subtree_ui_batches: Vec<PendingAddedSubtreeUiBatch>,
    parameter_values: Vec<(NodeId, ParamValue)>,
    project_dirty_nodes: HashSet<NodeId>,
    pending_edits: Vec<EditRequest>,
    event_listeners: super::listener_index::ListenerIndex,
    user_contexts: crate::contexts::UserContextRegistry,
}

impl<T: Node> Engine<T> {
    pub(crate) fn settle_edit_notifications(
        &mut self,
        max_passes: usize,
    ) -> Result<(), crate::engine::EngineEditError> {
        let pass_limit = max_passes.max(1);
        for _ in 0..pass_limit {
            if !self.edits.pending.is_empty() {
                self.apply_edits()?;
            }

            if !self.inbox.events.is_empty() {
                self.dispatch_inbox(crate::process_ctx::ExecutionPhase::EndOfTickStabilization)?;
            }

            if self.edits.pending.is_empty() && self.inbox.events.is_empty() {
                return Ok(());
            }
        }

        Err(crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "EditStabilizationFixedPoint",
            node: self.root,
            node_type: self
                .nodes
                .get(self.root)
                .map(|node| node.get_type().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            message: format!(
                "edit operation left pending edits or inbox events after {pass_limit} stabilization passes"
            ),
        })
    }

    pub(crate) fn edit_checkpoint(&mut self) -> Option<EditCheckpoint> {
        if self.edit_journal.is_some() {
            return None;
        }
        let pending_edits = std::mem::take(&mut self.edits.pending);
        self.begin_edit_journal(JournalMode::Atomic);
        Some(EditCheckpoint {
            inbox: std::mem::take(&mut self.inbox),
            ui_event_log: self.ui_event_log.clone(),
            ui_event_log_start: self.ui_event_log_start,
            ui_latest_event_times: self.ui_latest_event_times.clone(),
            ui_pending_param_event_times: self.ui_pending_param_event_times.clone(),
            ui_epoch: self.ui_epoch,
            next_ui_tx_id: self.next_ui_tx_id,
            ui_graph_version: self.ui_graph_version,
            time: self.time,
            pending_node_ready: self.pending_node_ready.clone(),
            pending_added_subtree_ui_batches: self.pending_added_subtree_ui_batches.clone(),
            project_dirty_nodes: self.project_dirty_nodes.clone(),
            pending_edits,
            event_listeners: self.event_listeners.clone(),
            user_contexts: self.user_contexts.clone(),
            parameter_values: self
                .nodes
                .iter()
                .filter_map(|(id, node)| node.engine_param_snapshot().map(|s| (id, s.value)))
                .collect(),
        })
    }

    pub(crate) fn commit_edit_checkpoint(&mut self, checkpoint: Option<EditCheckpoint>) {
        let Some(mut checkpoint) = checkpoint else {
            return;
        };
        self.commit_edit_journal();
        let mut pending = checkpoint.pending_edits;
        pending.append(&mut self.edits.pending);
        self.edits.pending = pending;
        checkpoint.inbox.events.append(&mut self.inbox.events);
        self.inbox = checkpoint.inbox;
    }

    pub(crate) fn rollback_edit_checkpoint(
        &mut self,
        checkpoint: Option<EditCheckpoint>,
    ) -> Result<(), EngineEditError> {
        let Some(checkpoint) = checkpoint else {
            return Ok(());
        };
        let result = self.rollback_edit_journal();
        for (node, value) in &checkpoint.parameter_values {
            if let Some(target) = self.nodes.get_mut(*node) {
                target.engine_set_param_value(value.clone());
            }
        }
        self.parameter_values_cache = checkpoint.parameter_values.into_iter().collect();
        let values = &self.parameter_values_cache;
        for node in self.nodes.keys().collect::<Vec<_>>() {
            if let Some(target) = self.nodes.get_mut(node) {
                target.engine_sync_bound_param_handles(&mut |id| values.get(&id).cloned());
            }
        }
        self.inbox = checkpoint.inbox;
        self.edits.pending = checkpoint.pending_edits;
        self.ui_event_log = checkpoint.ui_event_log;
        self.ui_event_log_start = checkpoint.ui_event_log_start;
        self.ui_latest_event_times = checkpoint.ui_latest_event_times;
        self.ui_pending_param_event_times = checkpoint.ui_pending_param_event_times;
        self.ui_epoch = checkpoint.ui_epoch;
        self.next_ui_tx_id = checkpoint.next_ui_tx_id;
        self.ui_graph_version = checkpoint.ui_graph_version;
        self.time = checkpoint.time;
        self.pending_node_ready = checkpoint.pending_node_ready;
        self.pending_added_subtree_ui_batches = checkpoint.pending_added_subtree_ui_batches;
        self.project_dirty_nodes = checkpoint.project_dirty_nodes;
        self.event_listeners = checkpoint.event_listeners;
        self.resolve_reference_caches();
        self.user_contexts = checkpoint.user_contexts;
        self.mark_schedule_dirty();
        result
    }

    /// Nested operations participate in the outer transaction and propagate rejection.
    /// The caller owns notification timing; UI operations settle before commit.
    pub(crate) fn apply_atomic_edit_operation<R, E: EditOperationError>(
        &mut self,
        operation: impl FnOnce(&mut Self) -> Result<R, E>,
    ) -> Result<R, E> {
        let checkpoint = self.edit_checkpoint();
        match operation(self) {
            Ok(value) => {
                self.commit_edit_checkpoint(checkpoint);
                Ok(value)
            }
            Err(original) => match self.rollback_edit_checkpoint(checkpoint) {
                Ok(()) => Err(original),
                Err(rollback) => Err(E::with_rollback(original, rollback)),
            },
        }
    }
}
