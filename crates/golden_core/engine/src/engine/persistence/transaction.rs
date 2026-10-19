use super::*;
use crate::engine::transaction::EditCheckpoint;

impl crate::engine::EditOperationError for ProjectPersistenceError {
    fn with_rollback(original: Self, rollback: EngineEditError) -> Self {
        Self::Rollback {
            original: Box::new(original),
            rollback: Box::new(rollback),
        }
    }
}

impl<T: Node> Engine<T> {
    pub(crate) fn abort_project_subtree_commit(
        &mut self,
        checkpoint: Option<EditCheckpoint>,
        original: ProjectPersistenceError,
    ) -> ProjectPersistenceError {
        match self.rollback_edit_checkpoint(checkpoint) {
            Ok(()) => original,
            Err(rollback) => ProjectPersistenceError::Rollback {
                original: Box::new(original),
                rollback: Box::new(rollback),
            },
        }
    }
}
