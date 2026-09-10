//! Immutable authored project projection used by persistence workers.
//!
//! The control actor publishes only changed node records into a fixed-shard copy-on-write graph.
//! Save callers clone those shard roots in bounded time and perform sparse materialization and
//! JSON encoding after leaving the actor.

use std::collections::HashSet;
use std::sync::RwLock;

use crate::app::{ProjectGeneration, ProjectGraphCapture, ProjectLifecycle, sparse_project_file_from_capture};
use crate::engine::{Engine, ProjectFile, ProjectPersistenceError};
use crate::node::NodeId;

pub(super) struct ProjectDocumentCapture {
    graph: ProjectGraphCapture,
    pub(super) project_generation: ProjectGeneration,
    pub(super) document_revision: u64,
    pub(super) node_count: usize,
}

impl ProjectDocumentCapture {
    pub(super) fn materialize<T: ProjectLifecycle>(
        &self,
        ui_state: Option<serde_json::Value>,
    ) -> Result<ProjectFile, ProjectPersistenceError> {
        sparse_project_file_from_capture::<T>(&self.graph, ui_state)
    }
}

struct ProjectDocumentState {
    graph: ProjectGraphCapture,
    project_generation: ProjectGeneration,
    document_revision: u64,
    pending_nodes: HashSet<NodeId>,
    pending_error: Option<String>,
}

impl ProjectDocumentState {
    fn from_engine<T: ProjectLifecycle>(
        engine: &Engine<T>,
        project_generation: ProjectGeneration,
    ) -> Result<Self, ProjectPersistenceError> {
        Ok(Self {
            graph: ProjectGraphCapture::from_engine(engine)?,
            project_generation,
            document_revision: engine.current_history_state_id(),
            pending_nodes: HashSet::new(),
            pending_error: None,
        })
    }

    fn synchronize<T: ProjectLifecycle>(&mut self, engine: &mut Engine<T>) -> Result<(), String> {
        self.pending_nodes.extend(engine.take_project_dirty_nodes());
        if self.pending_nodes.is_empty() {
            self.document_revision = engine.current_history_state_id();
            self.pending_error = None;
            return Ok(());
        }

        let mut completed = Vec::with_capacity(self.pending_nodes.len());
        let mut first_error = None;
        for node_id in self.pending_nodes.iter().copied() {
            if engine.nodes.get(node_id).is_none() {
                self.graph.remove(&node_id);
                completed.push(node_id);
                continue;
            }
            match crate::app::CapturedProjectNode::from_engine(engine, node_id) {
                Ok(node) => {
                    self.graph.insert(node);
                    completed.push(node_id);
                }
                Err(error) => {
                    first_error.get_or_insert_with(|| error.to_string());
                }
            }
        }
        for node_id in completed {
            self.pending_nodes.remove(&node_id);
        }
        if let Some(error) = first_error {
            self.pending_error = Some(error.clone());
            return Err(error);
        }

        self.document_revision = engine.current_history_state_id();
        self.pending_error = None;
        Ok(())
    }

    fn capture(&self) -> Result<ProjectDocumentCapture, String> {
        if !self.pending_nodes.is_empty() {
            return Err(self
                .pending_error
                .clone()
                .unwrap_or_else(|| "project document projection has unpublished node changes".to_string()));
        }
        Ok(ProjectDocumentCapture {
            graph: self.graph.clone(),
            project_generation: self.project_generation,
            document_revision: self.document_revision,
            node_count: self.graph.len(),
        })
    }
}

pub(super) struct PreparedProjectDocumentReplacement {
    state: ProjectDocumentState,
}

impl PreparedProjectDocumentReplacement {
    pub(super) fn from_engine<T: ProjectLifecycle>(
        engine: &mut Engine<T>,
        project_generation: ProjectGeneration,
    ) -> Result<Self, String> {
        let state = ProjectDocumentState::from_engine(engine, project_generation).map_err(|error| error.to_string())?;
        engine.take_project_dirty_nodes();
        Ok(Self { state })
    }

    pub(super) fn synchronize<T: ProjectLifecycle>(&mut self, engine: &mut Engine<T>) -> Result<(), String> {
        self.state.synchronize(engine)
    }
}

pub(super) struct RetiredProjectDocumentState {
    _state: ProjectDocumentState,
}

pub(super) struct ProjectDocumentReadModel {
    state: RwLock<ProjectDocumentState>,
}

impl ProjectDocumentReadModel {
    pub(super) fn from_engine<T: ProjectLifecycle>(engine: &mut Engine<T>) -> Result<Self, String> {
        let state =
            ProjectDocumentState::from_engine(engine, ProjectGeneration::INITIAL).map_err(|error| error.to_string())?;
        engine.take_project_dirty_nodes();
        Ok(Self {
            state: RwLock::new(state),
        })
    }

    pub(super) fn synchronize<T: ProjectLifecycle>(&self, engine: &mut Engine<T>) -> Result<(), String> {
        self.state
            .write()
            .expect("project document read model poisoned")
            .synchronize(engine)
    }

    pub(super) fn capture(&self) -> Result<ProjectDocumentCapture, String> {
        self.state
            .read()
            .expect("project document read model poisoned")
            .capture()
    }

    pub(super) fn commit_project_replacement(
        &self,
        prepared: PreparedProjectDocumentReplacement,
    ) -> RetiredProjectDocumentState {
        let previous = std::mem::replace(
            &mut *self.state.write().expect("project document read model poisoned"),
            prepared.state,
        );
        RetiredProjectDocumentState { _state: previous }
    }
}
