#[cfg(test)]
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::app::{ProjectGeneration, ProjectLifecycle, capture_sparse_project_file_with_ui_state};
use crate::engine::ProjectFile;
use crate::ui_sync::UiProjectFileSpec;

use super::ProductionRuntime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProjectSaveStage {
    AfterAcceptance,
    BeforeMetadataPublication,
}

#[cfg(test)]
pub(crate) type ProjectSaveFaultCallback = Arc<dyn Fn(ProjectSaveStage, u64) + Send + Sync + 'static>;

#[derive(Clone, Default)]
pub(super) struct ProjectSaveFaultHook {
    #[cfg(test)]
    callback: Arc<Mutex<Option<ProjectSaveFaultCallback>>>,
}

impl ProjectSaveFaultHook {
    pub(super) fn invoke(&self, stage: ProjectSaveStage, request_id: u64) {
        #[cfg(test)]
        {
            let callback = self.callback.lock().expect("project-save fault hook poisoned").clone();
            if let Some(callback) = callback {
                callback(stage, request_id);
            }
        }
        #[cfg(not(test))]
        let _ = (stage, request_id);
    }

    #[cfg(test)]
    pub(super) fn set(&self, callback: Option<ProjectSaveFaultCallback>) {
        *self.callback.lock().expect("project-save fault hook poisoned") = callback;
    }
}

/// Request to durably save one captured project document.
#[derive(Clone, Debug)]
pub struct ProjectSaveRequest {
    /// Destination selected by the host after applying the app's extension policy.
    pub path: String,
    /// Optional project-owned UI state stored in the persistence envelope.
    pub ui_state: Option<serde_json::Value>,
}

/// Durable save receipt and authoritative document-state result.
#[derive(Clone, Debug)]
pub struct ProjectSaveResult {
    /// Absolute normalized path used for the physical transaction.
    pub path: String,
    /// Monotonic persistence request identity.
    pub request_id: u64,
    /// Project generation captured and committed by the save.
    pub project_generation: ProjectGeneration,
    /// Document revision represented by the saved bytes.
    pub document_revision: u64,
    /// Current document revision after post-commit metadata publication.
    pub current_document_revision: u64,
    /// Whether this receipt won current-path and saved-revision publication.
    pub metadata_applied: bool,
    /// Whether the current document differs from its most recently published saved revision.
    pub dirty: bool,
    /// Number of nodes represented by the saved document.
    pub node_count: usize,
    /// Pretty-encoded document size in bytes.
    pub encoded_bytes: usize,
    /// Time spent waiting for the production control actor during capture.
    pub lock_wait: Duration,
    /// Time spent creating the owned sparse document capture inside the actor.
    pub capture: Duration,
    /// Time spent encoding the owned capture outside the actor.
    pub serialize: Duration,
    /// Time spent waiting for destination order, capacity, or replacement fencing.
    pub coordination_wait: Duration,
    /// Time spent in the complete durable file transaction.
    pub write: Duration,
    /// Total save duration.
    pub total: Duration,
}

/// Current authored and persisted state of one runtime project.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectPersistenceStatus {
    /// Authoritative project generation.
    pub project_generation: ProjectGeneration,
    /// Current authored document revision.
    pub document_revision: u64,
    /// Most recently published saved revision for this generation.
    pub saved_document_revision: Option<u64>,
    /// Highest save request whose metadata was applied to this generation.
    pub latest_save_request_id: u64,
    /// Current normalized project path, when the project has been saved or loaded.
    pub current_path: Option<String>,
    /// Whether the authored revision differs from the saved revision or has never been saved.
    pub dirty: bool,
}

struct ProjectDocumentCapture {
    project: ProjectFile,
    project_generation: ProjectGeneration,
    document_revision: u64,
    node_count: usize,
    capture: Duration,
}

#[derive(Clone, Copy, Debug)]
struct SavePublication {
    metadata_applied: bool,
    current_document_revision: u64,
    saved_document_revision: Option<u64>,
}

impl<T: ProjectLifecycle> ProductionRuntime<T> {
    /// Captures, encodes, and commits one project through the coordinated persistence service.
    pub fn save_project(&self, request: ProjectSaveRequest) -> Result<ProjectSaveResult, String> {
        let started = Instant::now();
        let ProjectSaveRequest { path, ui_state } = request;
        let capture_receipt = self
            .inner
            .control
            .call(move |state| {
                let capture_started = Instant::now();
                let project = capture_sparse_project_file_with_ui_state(&state.engine, ui_state)
                    .map_err(|error| error.to_string())?;
                let persistence = state.project_persistence_snapshot();
                Ok::<_, String>(ProjectDocumentCapture {
                    project,
                    project_generation: persistence.project_generation,
                    document_revision: persistence.document_revision,
                    node_count: state.engine.nodes.iter().count(),
                    capture: capture_started.elapsed(),
                })
            })
            .map_err(|error| error.to_string())?;
        let capture = capture_receipt.output?;

        let ticket = self
            .inner
            .persistence_coordinator
            .accept_save(&path, capture.project_generation.get(), capture.document_revision)
            .map_err(|error| error.to_string())?;
        self.inner
            .project_save_fault_hook
            .invoke(ProjectSaveStage::AfterAcceptance, ticket.info().request_id);

        let serialize_started = Instant::now();
        let json = serde_json::to_string_pretty(&capture.project).map_err(|error| error.to_string())?;
        let encoded_bytes = json.len();
        let serialize = serialize_started.elapsed();
        let target_path = ticket.info().target.to_string_lossy().into_owned();
        let project_file = UiProjectFileSpec::from_project_file_spec(T::project_file_spec(), Some(target_path.clone()));
        let read_model = self.inner.read_model.clone();
        let save_fault_hook = self.inner.project_save_fault_hook.clone();
        let committed = self
            .inner
            .persistence_coordinator
            .commit_save(ticket, json.as_bytes(), |ticket| {
                save_fault_hook.invoke(ProjectSaveStage::BeforeMetadataPublication, ticket.request_id);
                let project_generation = ProjectGeneration::new(ticket.project_generation);
                let request_id = ticket.request_id;
                let document_revision = ticket.document_revision;
                self.inner
                    .control
                    .call(move |state| {
                        let metadata_applied =
                            state.publish_saved_document(project_generation, request_id, document_revision);
                        if metadata_applied {
                            read_model.set_project_file(project_file);
                        }
                        let persistence = state.project_persistence_snapshot();
                        SavePublication {
                            metadata_applied,
                            current_document_revision: persistence.document_revision,
                            saved_document_revision: persistence.saved_document_revision,
                        }
                    })
                    .expect("production control actor disconnected during save publication")
                    .output
            })
            .map_err(|error| error.to_string())?;
        let publication = committed.publication;

        Ok(ProjectSaveResult {
            path: target_path,
            request_id: committed.ticket.request_id,
            project_generation: capture.project_generation,
            document_revision: capture.document_revision,
            current_document_revision: publication.current_document_revision,
            metadata_applied: publication.metadata_applied,
            dirty: publication.saved_document_revision != Some(publication.current_document_revision),
            node_count: capture.node_count,
            encoded_bytes,
            lock_wait: capture_receipt.queue_wait,
            capture: capture.capture,
            serialize,
            coordination_wait: committed.coordination_wait,
            write: committed.write,
            total: started.elapsed(),
        })
    }

    /// Returns one actor-consistent view of authored revision and save metadata.
    pub fn project_persistence_status(&self) -> ProjectPersistenceStatus {
        let read_model = self.inner.read_model.clone();
        self.inner
            .control
            .call(move |state| {
                let persistence = state.project_persistence_snapshot();
                let current_path = read_model.current_project_file().current_path;
                ProjectPersistenceStatus {
                    project_generation: persistence.project_generation,
                    document_revision: persistence.document_revision,
                    saved_document_revision: persistence.saved_document_revision,
                    latest_save_request_id: persistence.latest_save_request_id,
                    current_path,
                    dirty: persistence.saved_document_revision != Some(persistence.document_revision),
                }
            })
            .expect("production control actor disconnected")
            .output
    }
}
