use std::collections::HashSet;

use super::history::{HistoryStep, HistoryTransaction};
use super::{Engine, EngineEditError};
use crate::edit::CompletionToken;
use crate::node::{Node, NodeId};

/// One ordered history collector shared by nested edits. Atomic scopes retain
/// original-tree boundaries; ordinary drains retain their applied prefix.
pub(super) struct EditJournal<T: Node> {
    original_nodes: Option<HashSet<NodeId>>,
    entries: Vec<(u64, HistoryStep<T>)>,
    next_order: u64,
    added_nodes: HashSet<NodeId>,
    authored_dirty: bool,
    completions: Vec<CompletionToken>,
}

pub(super) enum JournalMode {
    AppliedPrefix,
    Atomic,
}

impl<T: Node> Engine<T> {
    pub(super) fn has_atomic_journal(&self) -> bool {
        self.edit_journal
            .as_ref()
            .is_some_and(|journal| journal.original_nodes.is_some())
    }

    pub(super) fn capture_opaque_history(&self, capture_history: bool, node: NodeId) -> bool {
        self.edit_journal.as_ref().map_or(capture_history, |journal| {
            journal
                .original_nodes
                .as_ref()
                .is_none_or(|nodes| nodes.contains(&node))
        })
    }

    pub(super) fn begin_edit_journal(&mut self, mode: JournalMode) {
        assert!(self.edit_journal.is_none(), "edit journals cannot be nested");
        self.edit_journal = Some(EditJournal {
            original_nodes: match mode {
                JournalMode::AppliedPrefix => None,
                JournalMode::Atomic => Some(self.nodes.keys().collect()),
            },
            entries: Vec::new(),
            next_order: 0,
            added_nodes: HashSet::new(),
            authored_dirty: false,
            completions: Vec::new(),
        });
    }

    /// Reserve order before executing a request: its callbacks can execute nested
    /// requests before the outer request returns its completed history effect.
    pub(super) fn edit_order(&mut self) -> Option<u64> {
        let journal = self.edit_journal.as_mut()?;
        let order = journal.next_order;
        journal.next_order += 1;
        Some(order)
    }

    pub(super) fn capture_journal_step(&mut self, order: u64, mut step: HistoryStep<T>) {
        let journal = self.edit_journal.as_mut().expect("edit journal is active");
        if journal
            .original_nodes
            .as_ref()
            .is_none_or(|nodes| step.touches_original_nodes(nodes))
        {
            if let Some(node) = step.added_node()
                && !journal.added_nodes.insert(node)
            {
                step.dispose_detached(self);
                return;
            }
            journal.authored_dirty |= matches!(&step, HistoryStep::AuthoredNode(_));
            journal.entries.push((order, step));
        } else {
            // Detached payloads from candidate-only removals must release their
            // reserved identities even though the candidate root owns their history.
            step.dispose_detached(self);
        }
    }

    pub(super) fn commit_edit_journal(&mut self) {
        self.flush_edit_journal();
        let journal = self.edit_journal.take().expect("edit journal is active");
        for completion in journal.completions {
            completion.applied();
        }
    }

    /// Atomic success is provisional until commit; rollback drops these tokens as failed.
    pub(super) fn complete_edit_requests(&mut self, completions: Vec<CompletionToken>) {
        if self.has_atomic_journal() {
            self.edit_journal
                .as_mut()
                .expect("atomic journal is active")
                .completions
                .extend(completions);
        } else {
            for completion in completions {
                completion.applied();
            }
        }
    }

    pub(super) fn flush_edit_journal(&mut self) {
        let authored_dirty = std::mem::take(
            &mut self
                .edit_journal
                .as_mut()
                .expect("edit journal is active")
                .authored_dirty,
        );
        if authored_dirty {
            self.sync_missing_reference_warnings();
            self.rebuild_user_context_registry_from_nodes();
            self.mark_user_context_graph_changed();
        }
        let mut entries = std::mem::take(&mut self.edit_journal.as_mut().expect("edit journal is active").entries);
        entries.sort_by_key(|(order, _)| *order);
        if entries.is_empty() {
            return;
        }
        self.clear_redo_history();
        if self.has_active_edit_session() {
            for (_, step) in entries {
                self.push_step_into_active_history_transaction(step);
            }
        } else {
            let mut transaction = HistoryTransaction::new();
            for (_, step) in entries {
                transaction.push(step);
            }
            self.push_undo_transaction(transaction);
        }
    }

    pub(super) fn rollback_edit_journal(&mut self) -> Result<(), EngineEditError> {
        let mut journal = self.edit_journal.take().expect("edit journal is active");
        let original_nodes = journal.original_nodes.expect("only atomic journals can roll back");
        journal.entries.sort_by_key(|(order, _)| *order);
        self.rollback_replay = true;
        let mut failure = None;
        let mut restored = Vec::new();
        for (_, step) in journal.entries.iter_mut().rev() {
            restored.extend(
                step.nodes_restored_on_undo()
                    .into_iter()
                    .filter(|id| original_nodes.contains(id)),
            );
            if let Err(error) = step.undo_for_rollback(self)
                && failure.is_none()
            {
                failure = Some(error);
            }
        }
        for (_, mut step) in journal.entries {
            step.dispose_detached(self);
        }
        self.rollback_replay = false;
        let mut reactivated = std::collections::HashSet::new();
        restored.retain(|node| self.nodes.contains(*node) && reactivated.insert(*node));
        if let Err(error) = self.run_node_ready_for_batch(&restored, crate::node::NodeCreationContext::HistoryRestore)
            && failure.is_none()
        {
            failure = Some(error);
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}
