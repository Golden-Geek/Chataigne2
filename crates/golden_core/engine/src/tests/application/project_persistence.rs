use super::*;

use tempfile::tempdir;

use crate::application::{ProjectSaveRequest, ProjectSaveStage};

fn simple_replacement(label: &str, current_path: Option<String>) -> ProjectReplacement<FacadeTestNode> {
    ProjectReplacement {
        engine: Engine::new(Folder::new(label).into()),
        project_file: UiProjectFileSpec::from_project_file_spec(FacadeTestNode::project_file_spec(), current_path),
        reason: "persistence-test".to_string(),
        recover: false,
    }
}

fn runtime_with_project_parameter() -> (ProductionRuntime<FacadeTestNode>, NodeId) {
    let mut engine: Engine<FacadeTestNode> = Engine::new(Folder::new("Root").into());
    engine.add_node(
        crate::parameter::Parameter::new(
            "Project Value",
            ParamValue::Int(1),
            crate::parameter::ParameterChangeCheck::None,
        )
        .into(),
        None,
    );
    engine.apply_edits().expect("project parameter should attach");
    let parameter = engine
        .nodes
        .iter()
        .find_map(|(node_id, node)| (node.node_data().meta.label == "Project Value").then_some(node_id))
        .expect("project parameter");
    crate::app::prepare_engine_for_runtime(&mut engine).expect("project runtime preparation");
    (
        ProductionRuntime::new(
            engine,
            UiProjectFileSpec::from_project_file_spec(FacadeTestNode::project_file_spec(), None),
        ),
        parameter,
    )
}

#[test]
fn opaque_script_config_mutation_is_published_to_the_project_document() {
    let mut engine: Engine<FacadeTestNode> = Engine::new(Folder::new("Root").into());
    engine.add_node(
        ScriptNode::new(
            "Project Script",
            ScriptNodeConfig {
                source: ScriptSource::Inline { text: String::new() },
            },
        )
        .into(),
        None,
    );
    engine.apply_edits().expect("script should attach");
    let script = engine
        .nodes
        .iter()
        .find_map(|(node_id, node)| (node.node_data().meta.label == "Project Script").then_some(node_id))
        .expect("project script");
    crate::app::prepare_engine_for_runtime(&mut engine).expect("project runtime preparation");
    let runtime = ProductionRuntime::new(
        engine,
        UiProjectFileSpec::from_project_file_spec(FacadeTestNode::project_file_spec(), None),
    );
    let expected_source = "const persistedMarker = 7;";
    runtime
        .set_script_config(
            script,
            crate::script::ScriptUiConfig {
                source: crate::script::ScriptUiSource::Inline {
                    text: expected_source.to_string(),
                },
            },
            false,
        )
        .0
        .expect("script config update");

    let directory = tempdir().expect("temporary project directory");
    let target = directory.path().join("script.noisette");
    runtime
        .save_project(ProjectSaveRequest {
            path: target.to_string_lossy().into_owned(),
            ui_state: None,
        })
        .expect("script project save");
    let json = std::fs::read_to_string(target).expect("saved script project JSON");
    let loaded = crate::app::from_sparse_project_json::<FacadeTestNode>(&json).expect("script project should decode");
    let state = loaded
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.node_data().meta.label == "Project Script")
                .then(|| node.engine_script_state())
                .flatten()
        })
        .expect("saved script state");
    assert_eq!(
        state.config.source,
        crate::script::ScriptUiSource::Inline {
            text: expected_source.to_string()
        }
    );
}

#[test]
fn successful_save_publishes_path_and_exact_clean_revision() {
    let (runtime, project_value) = runtime_with_project_parameter();
    let directory = tempdir().expect("temporary project directory");
    let target = directory.path().join("saved.noisette");
    let initial = runtime.project_persistence_status();
    assert!(initial.dirty);
    assert!(initial.saved_document_revision.is_none());

    let saved = runtime
        .save_project(ProjectSaveRequest {
            path: target.to_string_lossy().into_owned(),
            ui_state: None,
        })
        .expect("initial save");
    assert!(saved.metadata_applied);
    assert!(!saved.dirty);
    assert_eq!(saved.document_revision, 0);
    let clean = runtime.project_persistence_status();
    assert_eq!(clean.saved_document_revision, Some(clean.document_revision));
    assert_eq!(clean.current_path.as_deref(), Some(saved.path.as_str()));

    runtime
        .apply_graph_edit(UiEditIntent::SetParam {
            node: project_value,
            value: ParamValue::Int(199),
            behaviour: Default::default(),
        })
        .expect("authored edit");
    let dirty = runtime.project_persistence_status();
    assert!(dirty.dirty);
    assert_ne!(dirty.saved_document_revision, Some(dirty.document_revision));

    let saved_again = runtime
        .save_project(ProjectSaveRequest {
            path: target.to_string_lossy().into_owned(),
            ui_state: None,
        })
        .expect("updated save");
    assert!(!saved_again.dirty);
    assert_eq!(saved_again.document_revision, dirty.document_revision);
    let json = std::fs::read_to_string(&target).expect("saved project JSON");
    let loaded = crate::app::from_sparse_project_json::<FacadeTestNode>(&json).expect("saved project should decode");
    let saved_value = loaded
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.node_data().meta.label == "Project Value")
                .then(|| node.engine_param_snapshot().map(|snapshot| snapshot.value))
                .flatten()
        })
        .expect("saved project parameter");
    assert_eq!(saved_value, ParamValue::Int(199));
}

#[test]
fn edit_after_capture_keeps_the_later_authored_revision_dirty() {
    let (runtime, project_value) = runtime_with_project_parameter();
    let directory = tempdir().expect("temporary project directory");
    let target = directory.path().join("captured-before-edit.noisette");
    let target_for_thread = target.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    runtime.set_project_save_fault_hook(Some(Arc::new(move |stage, request_id| {
        if stage == ProjectSaveStage::AfterAcceptance && request_id == 1 {
            entered_tx.send(()).expect("capture signal");
            release_rx.lock().expect("release lock").recv().expect("release signal");
        }
    })));

    let save_runtime = runtime.clone();
    let save = thread::spawn(move || {
        save_runtime.save_project(ProjectSaveRequest {
            path: target_for_thread.to_string_lossy().into_owned(),
            ui_state: None,
        })
    });
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("save accepted after capture");
    runtime
        .apply_graph_edit(UiEditIntent::SetParam {
            node: project_value,
            value: ParamValue::Int(198),
            behaviour: Default::default(),
        })
        .expect("later edit");
    release_tx.send(()).expect("release save");

    let saved = save.join().expect("save thread").expect("captured save commits");
    assert!(saved.metadata_applied);
    assert!(saved.dirty);
    assert!(saved.current_document_revision > saved.document_revision);
    let status = runtime.project_persistence_status();
    assert_eq!(status.saved_document_revision, Some(saved.document_revision));
    assert!(status.dirty);
    let json = std::fs::read_to_string(&target).expect("captured project JSON");
    let loaded = crate::app::from_sparse_project_json::<FacadeTestNode>(&json).expect("captured project should decode");
    let captured_value = loaded
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.node_data().meta.label == "Project Value")
                .then(|| node.engine_param_snapshot().map(|snapshot| snapshot.value))
                .flatten()
        })
        .expect("captured project parameter");
    assert_eq!(captured_value, ParamValue::Int(1));
    runtime.set_project_save_fault_hook(None);
}

#[test]
fn save_as_metadata_follows_latest_successful_request_not_completion_order() {
    let runtime = runtime();
    let directory = tempdir().expect("temporary project directory");
    let first_target = directory.path().join("first.noisette");
    let second_target = directory.path().join("second.noisette");
    let expected_second_path = second_target.to_string_lossy().into_owned();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    runtime.set_project_save_fault_hook(Some(Arc::new(move |stage, request_id| {
        if stage == ProjectSaveStage::AfterAcceptance && request_id == 1 {
            entered_tx.send(()).expect("first acceptance signal");
            release_rx.lock().expect("release lock").recv().expect("release signal");
        }
    })));

    let first_runtime = runtime.clone();
    let first = thread::spawn(move || {
        first_runtime.save_project(ProjectSaveRequest {
            path: first_target.to_string_lossy().into_owned(),
            ui_state: None,
        })
    });
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("first request accepted");
    let second = runtime
        .save_project(ProjectSaveRequest {
            path: second_target.to_string_lossy().into_owned(),
            ui_state: None,
        })
        .expect("newer Save As commits first");
    assert_eq!(second.request_id, 2);
    assert!(second.metadata_applied);
    release_tx.send(()).expect("release older Save As");
    let first = first.join().expect("first save thread").expect("older Save As writes");
    assert_eq!(first.request_id, 1);
    assert!(!first.metadata_applied);

    let status = runtime.project_persistence_status();
    assert_eq!(status.latest_save_request_id, 2);
    assert_eq!(status.current_path.as_deref(), Some(expected_second_path.as_str()));
    runtime.set_project_save_fault_hook(None);
}

#[test]
fn replacement_invalidates_an_accepted_save_that_has_not_started_writing() {
    let runtime = runtime();
    let directory = tempdir().expect("temporary project directory");
    let target = directory.path().join("stale.noisette");
    let target_for_thread = target.clone();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    runtime.set_project_save_fault_hook(Some(Arc::new(move |stage, request_id| {
        if stage == ProjectSaveStage::AfterAcceptance && request_id == 1 {
            entered_tx.send(()).expect("acceptance signal");
            release_rx.lock().expect("release lock").recv().expect("release signal");
        }
    })));

    let save_runtime = runtime.clone();
    let save = thread::spawn(move || {
        save_runtime.save_project(ProjectSaveRequest {
            path: target_for_thread.to_string_lossy().into_owned(),
            ui_state: None,
        })
    });
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("old-generation save accepted");
    let replacement_path = directory.path().join("loaded.noisette").to_string_lossy().into_owned();
    let replacement = runtime
        .replace_project(simple_replacement("Replacement", Some(replacement_path.clone())))
        .expect("replacement commits");
    assert_eq!(replacement.project_generation.get(), 2);
    release_tx.send(()).expect("release stale save");
    let error = save
        .join()
        .expect("save thread")
        .expect_err("old-generation save must be rejected before writing");
    assert!(error.contains("stale"));
    assert!(!target.exists());

    let status = runtime.project_persistence_status();
    assert_eq!(status.project_generation.get(), 2);
    assert_eq!(status.current_path.as_deref(), Some(replacement_path.as_str()));
    runtime.set_project_save_fault_hook(None);
}

#[test]
fn replacement_waits_for_save_metadata_publication_before_cutover() {
    let runtime = runtime();
    let directory = tempdir().expect("temporary project directory");
    let target = directory.path().join("active.noisette");
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let release_rx = Arc::new(Mutex::new(release_rx));
    runtime.set_project_save_fault_hook(Some(Arc::new(move |stage, request_id| {
        if stage == ProjectSaveStage::BeforeMetadataPublication && request_id == 1 {
            entered_tx.send(()).expect("publication signal");
            release_rx.lock().expect("release lock").recv().expect("release signal");
        }
    })));

    let save_runtime = runtime.clone();
    let save = thread::spawn(move || {
        save_runtime.save_project(ProjectSaveRequest {
            path: target.to_string_lossy().into_owned(),
            ui_state: None,
        })
    });
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("save reached metadata publication with active lease");

    let replacement_runtime = runtime.clone();
    let (replacement_done_tx, replacement_done_rx) = mpsc::channel();
    let replacement = thread::spawn(move || {
        let result = replacement_runtime.replace_project(simple_replacement("After Save", None));
        replacement_done_tx.send(()).expect("replacement done signal");
        result
    });
    assert!(
        replacement_done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
        "replacement must wait while save publication retains the commit lease"
    );
    release_tx.send(()).expect("release save publication");
    save.join().expect("save thread").expect("save completion");
    replacement_done_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("replacement completes after save lease");
    replacement
        .join()
        .expect("replacement thread")
        .expect("replacement completion");
    assert_eq!(runtime.current_project_generation().get(), 2);
    runtime.set_project_save_fault_hook(None);
}
