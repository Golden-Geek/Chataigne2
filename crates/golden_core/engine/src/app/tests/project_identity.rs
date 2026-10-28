use super::*;
use crate::app::{from_sparse_project_json, to_sparse_subtree_json_pretty};
use crate::engine::ProjectPersistenceError;

define_node_enum!(
    enum FirstApplicationNode {}
);
impl ProjectLifecycle for FirstApplicationNode {
    fn application_id() -> &'static str {
        "first-application"
    }
}

define_node_enum!(
    enum SecondApplicationNode {}
);
impl ProjectLifecycle for SecondApplicationNode {
    fn application_id() -> &'static str {
        "second-application"
    }
}

#[test]
fn shared_folder_graph_round_trips_only_for_its_application() {
    let engine = Engine::<FirstApplicationNode>::new(Folder::new("Shared Folder").into());
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<FirstApplicationNode>(&json).unwrap();
    assert_eq!(
        loaded.nodes.get(loaded.root).unwrap().node_data().meta.label,
        "Shared Folder"
    );
    assert!(matches!(
        from_sparse_project_json::<SecondApplicationNode>(&json),
        Err(ProjectPersistenceError::ApplicationMismatch { found, expected })
            if found == "first-application" && expected == "second-application"
    ));
}

#[test]
fn full_project_rejects_missing_identity_while_subtrees_remain_generic() {
    let engine = Engine::<FirstApplicationNode>::new(Folder::new("Shared Folder").into());
    let subtree = to_sparse_subtree_json_pretty(&engine, engine.root).unwrap();
    assert!(matches!(
        from_sparse_project_json::<FirstApplicationNode>(&subtree),
        Err(ProjectPersistenceError::MissingApplicationId)
    ));

    let mut target = Engine::<SecondApplicationNode>::new(Folder::new("Target").into());
    let parent = target.root;
    crate::app::insert_sparse_subtree_json(&mut target, parent, None, &subtree).unwrap();
}

#[test]
fn application_identity_is_checked_before_node_codec_expansion() {
    let engine = Engine::<FirstApplicationNode>::new(Folder::new("Shared Folder").into());
    let mut wire: serde_json::Value = serde_json::from_str(&to_sparse_project_json_pretty(&engine).unwrap()).unwrap();
    wire["root"]["type"] = "unregistered.node.codec".into();
    assert!(matches!(
        from_sparse_project_json::<SecondApplicationNode>(&wire.to_string()),
        Err(ProjectPersistenceError::ApplicationMismatch { .. })
    ));
}

#[test]
fn identity_failure_never_recovers_through_an_own_application_backup() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("project.json");
    let own = Engine::<FirstApplicationNode>::new(Folder::new("Own backup").into());
    let own_json = to_sparse_project_json_pretty(&own).unwrap();
    let foreign = Engine::<SecondApplicationNode>::new(Folder::new("Foreign primary").into());
    let foreign_json = to_sparse_project_json_pretty(&foreign).unwrap();

    for primary in [foreign_json, to_sparse_subtree_json_pretty(&own, own.root).unwrap()] {
        golden_persistence::write_file_atomically_with_recovery(&path, own_json.as_bytes()).unwrap();
        golden_persistence::write_file_atomically_with_recovery(&path, primary.as_bytes()).unwrap();
        let error = load_sparse_project_file_with_ui_state_recovering::<FirstApplicationNode, _>(&path)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            ProjectPersistenceError::ApplicationMismatch { .. } | ProjectPersistenceError::MissingApplicationId
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), primary);
    }
}
