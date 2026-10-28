//! Full application project I/O and generic subtree serialization.

use std::fs;
use std::path::Path;

use crate::engine::{Engine, PROJECT_FILE_VERSION, ProjectFile, ProjectLoadRecoveryReport, ProjectPersistenceError};
use crate::node::{Folder, Node, NodeId};

use super::sparse::{
    SparseEncodeCache, collect_referenced_uuids, encode_sparse_node_record, expand_sparse_project_file,
};
use super::{ProjectLifecycle, ProjectNode};

/// Serializes one project using the app node codec path used for project files.
pub fn to_sparse_project_json_pretty<T>(engine: &Engine<T>) -> Result<String, ProjectPersistenceError>
where
    T: ProjectLifecycle,
{
    to_sparse_project_json_pretty_with_ui_state(engine, None)
}

/// Serializes one project with an optional project-owned UI state payload.
pub fn to_sparse_project_json_pretty_with_ui_state<T>(
    engine: &Engine<T>,
    ui_state: Option<serde_json::Value>,
) -> Result<String, ProjectPersistenceError>
where
    T: ProjectLifecycle,
{
    let project = capture_sparse_project_file_with_ui_state(engine, ui_state)?;
    let document = golden_persistence::ApplicationProjectDocument {
        application_id: T::application_id().to_owned(),
        project,
    };
    Ok(golden_persistence::encode_application_project_document(
        &document,
        T::application_id(),
    )?)
}

/// Captures one owned sparse project document for encoding outside the live engine boundary.
pub fn capture_sparse_project_file_with_ui_state<T>(
    engine: &Engine<T>,
    ui_state: Option<serde_json::Value>,
) -> Result<ProjectFile, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let mut project = to_sparse_project_file(engine)?;
    project.ui_state = ui_state;
    Ok(project)
}

/// Serializes one node subtree using the same sparse codec as project files.
pub fn to_sparse_subtree_json_pretty<T>(engine: &Engine<T>, root: NodeId) -> Result<String, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let project = capture_sparse_subtree_file(engine, root)?;
    Ok(golden_persistence::encode_project_document(&project)?)
}

/// Captures one node subtree as a typed sparse project document.
///
/// This is the non-string boundary for archival features that need to retain a
/// restorable authored hierarchy without keeping live nodes or process-local
/// identifiers alive.
pub fn capture_sparse_subtree_file<T>(engine: &Engine<T>, root: NodeId) -> Result<ProjectFile, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    to_sparse_subtree_file(engine, root)
}

/// Loads one sparse project JSON document by first expanding declared deltas
/// against the app-owned schema defaults.
pub fn from_sparse_project_json<T>(json: &str) -> Result<Engine<T>, ProjectPersistenceError>
where
    T: ProjectLifecycle,
{
    Ok(from_sparse_project_json_with_ui_state(json)?.0)
}

/// Loads one sparse project JSON document and returns its project-owned UI state.
pub fn from_sparse_project_json_with_ui_state<T>(
    json: &str,
) -> Result<(Engine<T>, Option<serde_json::Value>), ProjectPersistenceError>
where
    T: ProjectLifecycle,
{
    from_sparse_project_json_with_ui_state_inner(json, false).map(|(engine, ui_state, _)| (engine, ui_state))
}

/// Loads one sparse project JSON document, skipping user-approved recoverable rebuild problems.
pub fn from_sparse_project_json_with_ui_state_recovering<T>(
    json: &str,
) -> Result<(Engine<T>, Option<serde_json::Value>, ProjectLoadRecoveryReport), ProjectPersistenceError>
where
    T: ProjectLifecycle,
{
    from_sparse_project_json_with_ui_state_inner(json, true)
}

fn from_sparse_project_json_with_ui_state_inner<T>(
    json: &str,
    recover: bool,
) -> Result<(Engine<T>, Option<serde_json::Value>, ProjectLoadRecoveryReport), ProjectPersistenceError>
where
    T: ProjectLifecycle,
{
    let parse_started = std::time::Instant::now();
    let project = golden_persistence::decode_application_project_document(json, T::application_id())?.project;
    let parse_elapsed = parse_started.elapsed();
    let ui_state = project.ui_state.clone();
    let expand_started = std::time::Instant::now();
    let expanded = expand_sparse_project_file::<T>(project)?;
    let expand_elapsed = expand_started.elapsed();
    let build_started = std::time::Instant::now();
    let engine = if recover {
        Engine::<T>::from_project_file_with_recovery(expanded, T::project_decode_node)
    } else {
        Engine::<T>::from_project_file_with(expanded, T::project_decode_node)
            .map(|engine| (engine, ProjectLoadRecoveryReport::default()))
    };
    eprintln!(
        "[project-load] parse_ms={} expand_ms={} engine_build_ms={}",
        parse_elapsed.as_millis(),
        expand_elapsed.as_millis(),
        build_started.elapsed().as_millis()
    );
    engine.map(|(engine, recovery)| (engine, ui_state, recovery))
}

/// Loads one sparse project file by first expanding declared deltas against the
/// app-owned schema defaults.
pub fn load_sparse_project_file<T, P>(path: P) -> Result<Engine<T>, ProjectPersistenceError>
where
    T: ProjectLifecycle,
    P: AsRef<Path>,
{
    Ok(load_sparse_project_file_with_ui_state(path)?.0)
}

/// Loads one sparse project file and returns its project-owned UI state.
pub fn load_sparse_project_file_with_ui_state<T, P>(
    path: P,
) -> Result<(Engine<T>, Option<serde_json::Value>), ProjectPersistenceError>
where
    T: ProjectLifecycle,
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let json = fs::read_to_string(path)?;
    let loaded = from_sparse_project_json_with_ui_state(&json)?;
    let _ = golden_persistence::clear_recovery_journal(path);
    Ok(loaded)
}

/// Loads one sparse project file, skipping user-approved recoverable rebuild problems.
pub fn load_sparse_project_file_with_ui_state_recovering<T, P>(
    path: P,
) -> Result<(Engine<T>, Option<serde_json::Value>, ProjectLoadRecoveryReport), ProjectPersistenceError>
where
    T: ProjectLifecycle,
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let candidates = golden_persistence::read_recovery_candidates(path)?;
    let mut primary_failure = candidates.primary_error.clone();

    if let Some(primary) = candidates.primary.as_deref() {
        match std::str::from_utf8(primary) {
            Ok(json) => match from_sparse_project_json_with_ui_state_recovering(json) {
                Ok(loaded) => {
                    let _ = golden_persistence::clear_recovery_journal(path);
                    return Ok(loaded);
                }
                Err(
                    error @ (ProjectPersistenceError::MissingApplicationId
                    | ProjectPersistenceError::ApplicationMismatch { .. }),
                ) => {
                    return Err(error);
                }
                Err(error) => primary_failure = Some(error.to_string()),
            },
            Err(error) => primary_failure = Some(format!("project file is not UTF-8: {error}")),
        }
    }

    let backup = candidates
        .backup
        .as_deref()
        .ok_or_else(|| ProjectPersistenceError::Recovery {
            message: format!(
                "primary failed ({}) and no readable backup exists{}",
                primary_failure.as_deref().unwrap_or("unknown error"),
                candidates
                    .backup_error
                    .as_deref()
                    .map(|error| format!(": {error}"))
                    .unwrap_or_default()
            ),
        })?;
    let backup_json = std::str::from_utf8(backup).map_err(|error| ProjectPersistenceError::Recovery {
        message: format!(
            "primary failed ({}) and backup is not UTF-8: {error}",
            primary_failure.as_deref().unwrap_or("unknown error")
        ),
    })?;
    let (engine, ui_state, mut recovery) =
        from_sparse_project_json_with_ui_state_recovering(backup_json).map_err(|backup_error| {
            ProjectPersistenceError::Recovery {
                message: format!(
                    "primary failed ({}); backup failed ({backup_error})",
                    primary_failure.as_deref().unwrap_or("unknown error")
                ),
            }
        })?;
    golden_persistence::restore_primary_from_backup(&candidates.paths, backup)?;
    let journal_diagnostic = candidates
        .journal_error
        .as_deref()
        .map(|error| format!("; journal={error}"))
        .unwrap_or_default();
    recovery.push_project_file_recovery(format!(
        "primary project file failed ({}); loaded last complete backup {}{}",
        primary_failure.as_deref().unwrap_or("unknown error"),
        candidates.paths.backup.display(),
        journal_diagnostic
    ));
    Ok((engine, ui_state, recovery))
}

/// Imports one sparse persisted subtree beneath `parent`.
pub fn insert_sparse_subtree_json<T>(
    engine: &mut Engine<T>,
    parent: NodeId,
    prev_sibling: Option<NodeId>,
    json: &str,
) -> Result<NodeId, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let project = golden_persistence::decode_project_document(json)?;
    let expanded = expand_sparse_project_file::<T>(project)?;
    engine.insert_project_subtree_with(expanded, parent, prev_sibling, T::project_decode_node)
}

fn to_sparse_project_file<T>(engine: &Engine<T>) -> Result<ProjectFile, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let referenced_uuids = collect_referenced_uuids(engine);
    let mut cache = SparseEncodeCache::default();
    let structural_root = cache.structural_baseline_for_node(engine, engine.root)?;
    let root = encode_sparse_node_record(
        engine,
        engine.root,
        Some(&structural_root),
        false,
        true,
        &referenced_uuids,
        &mut cache,
    )?
    .ok_or_else(|| ProjectPersistenceError::Codec {
        node_type: engine
            .nodes
            .get(engine.root)
            .map(Node::get_type)
            .unwrap_or("unknown")
            .to_string(),
        message: "root node cannot be omitted from sparse project output".to_string(),
    })?;

    Ok(ProjectFile {
        version: PROJECT_FILE_VERSION.to_string(),
        ui_state: None,
        root,
    })
}

fn to_sparse_subtree_file<T>(engine: &Engine<T>, root_id: NodeId) -> Result<ProjectFile, ProjectPersistenceError>
where
    T: ProjectNode + From<Folder>,
{
    let referenced_uuids = collect_referenced_uuids(engine);
    let mut cache = SparseEncodeCache::default();
    let structural_root = cache.structural_baseline_for_node(engine, root_id)?;
    let root = encode_sparse_node_record(
        engine,
        root_id,
        Some(&structural_root),
        false,
        false,
        &referenced_uuids,
        &mut cache,
    )?
    .ok_or_else(|| ProjectPersistenceError::Codec {
        node_type: engine
            .nodes
            .get(root_id)
            .map(Node::get_type)
            .unwrap_or("unknown")
            .to_string(),
        message: "subtree root cannot be omitted from sparse output".to_string(),
    })?;

    Ok(ProjectFile {
        version: PROJECT_FILE_VERSION.to_string(),
        ui_state: None,
        root,
    })
}
