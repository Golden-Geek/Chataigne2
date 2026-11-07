use super::*;
use crate::{app::from_sparse_project_json, node};

#[node("test.persistent_identity")]
struct PersistentIdentity {}
#[node("test.persistent_identity", from_struct)]
impl Node for PersistentIdentity {
    fn needs_update(&self) -> bool {
        false
    }
    fn project_preserve_identity(&self) -> bool {
        true
    }
}

#[node("test.ephemeral_identity")]
struct EphemeralIdentity {}
#[node("test.ephemeral_identity", from_struct)]
impl Node for EphemeralIdentity {
    fn needs_update(&self) -> bool {
        false
    }
}

#[node("test.identity_owner")]
#[children(
    node stable: PersistentIdentity = (PersistentIdentity::new());
    node transient: EphemeralIdentity = (EphemeralIdentity::new());
)]
struct IdentityOwner {}
#[node("test.identity_owner", from_struct)]
impl Node for IdentityOwner {
    fn needs_update(&self) -> bool {
        false
    }
}

define_node_enum!(
    enum IdentityApplicationNode {
        Owner(IdentityOwner),
        Stable(PersistentIdentity),
        Transient(EphemeralIdentity),
    }
);
impl ProjectLifecycle for IdentityApplicationNode {}

#[test]
fn unreferenced_declared_identity_survives_live_and_captured_sparse_codecs() {
    let mut engine = Engine::<IdentityApplicationNode>::new(Folder::new("Root").into());
    engine.add_node(IdentityOwner::new().into(), None);
    engine.apply_edits().unwrap();
    let stable = engine
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == "test.persistent_identity")
        .unwrap()
        .1
        .node_data()
        .meta
        .uuid;
    let wire = to_sparse_project_json_pretty(&engine).unwrap();
    let graph = ProjectGraphCapture::from_engine(&engine).unwrap();
    let captured = sparse_project_file_from_capture::<IdentityApplicationNode>(&graph, None).unwrap();
    assert_eq!(
        serde_json::to_value(capture_sparse_project_file_with_ui_state(&engine, None).unwrap()).unwrap(),
        serde_json::to_value(&captured).unwrap()
    );
    let child_types = &captured.root.children[0].children;
    assert!(
        child_types
            .iter()
            .any(|child| child.node_type == "test.persistent_identity" && child.uuid == stable)
    );
    assert!(
        !child_types
            .iter()
            .any(|child| child.node_type == "test.ephemeral_identity"),
        "ordinary unreferenced defaults remain sparse"
    );
    let loaded = from_sparse_project_json::<IdentityApplicationNode>(&wire).unwrap();
    assert!(loaded.node_id_by_uuid(stable).is_some());
    assert_eq!(
        loaded
            .nodes
            .iter()
            .filter(|(_, node)| node.get_type() == "test.persistent_identity")
            .count(),
        1
    );
}
