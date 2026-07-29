use std::sync::atomic::{AtomicUsize, Ordering};

use uuid::Uuid;

use super::{PROJECT_FILE_VERSION, ProjectFile, ProjectNodeMeta, ProjectNodeRecord, ProjectPersistenceError};
use crate::app::ProjectNode;
use crate::define_node_enum;
use crate::edit::Edit;
use crate::engine::Engine;
use crate::node::{
    FOLDER_NODE_TYPE, Folder, Node, NodeId, NodeScriptDescriptor, NodeUuid, UserContainerRules, UserNodeRole,
};
use crate::parameter::{ParamValue, ParameterEventBehaviour};
use crate::process_ctx::ProcessCtx;

static DUPLICATE_SNAPSHOT_DESCRIPTOR_CALLS: AtomicUsize = AtomicUsize::new(0);

#[crate::node("recover_bad_init_node")]
struct RecoverBadInitNode {}

#[crate::node("recover_bad_init_node", from_struct)]
impl Node for RecoverBadInitNode {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        ctx.edits.push(Edit::SetParam {
            node: NodeId(u64::MAX),
            value: ParamValue::Int(1),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
}

#[crate::node("snapshot_free_duplicate_node")]
struct SnapshotFreeDuplicateNode {}

#[crate::node("snapshot_free_duplicate_node", from_struct)]
impl Node for SnapshotFreeDuplicateNode {
    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        DUPLICATE_SNAPSHOT_DESCRIPTOR_CALLS.fetch_add(1, Ordering::Relaxed);
        NodeScriptDescriptor::default()
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
}

#[crate::node("decode_fallback_node")]
struct DecodeFallbackNode {}

#[crate::node("decode_fallback_node", from_struct)]
impl Node for DecodeFallbackNode {
    fn project_create(_node_type: &str) -> Option<Self>
    where
        Self: Sized,
    {
        None
    }
}

#[crate::node("detached_factory_child")]
struct DetachedFactoryChild {
    created_from_decoded_parent: bool,
}

#[crate::node("detached_factory_child", from_struct)]
impl Node for DetachedFactoryChild {
    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::Value::Null)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(|| Self::new(false))
    }

    fn user_item_kind(&self) -> &str {
        Self::NODE_TYPE
    }
}

#[crate::node("detached_factory_parent")]
struct DetachedFactoryParent {
    child_factory_enabled: bool,
}

#[crate::node("detached_factory_parent", from_struct)]
impl Node for DetachedFactoryParent {
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == DetachedFactoryChild::NODE_TYPE)
            .then(|| Box::new(DetachedFactoryChild::new(self.child_factory_enabled)) as Box<dyn Node>)
    }

    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::Value::Bool(self.child_factory_enabled))
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        self.child_factory_enabled = data
            .as_bool()
            .ok_or_else(|| "detached factory parent expects a boolean".to_string())?;
        Ok(())
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(|| Self::new(false))
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[DetachedFactoryChild::NODE_TYPE]))
    }
}

define_node_enum!(
    enum RecoveryTestNode {
        DecodeFallbackNode,
        DetachedFactoryChild,
        DetachedFactoryParent,
        RecoverBadInitNode,
        SnapshotFreeDuplicateNode,
    }
);

fn project_record(node_type: &str, label: &str, children: Vec<ProjectNodeRecord>) -> ProjectNodeRecord {
    ProjectNodeRecord {
        uuid: NodeUuid(Uuid::new_v4()),
        node_type: node_type.to_string(),
        user_role: UserNodeRole::Regular,
        meta: ProjectNodeMeta {
            label: Some(label.to_string()),
            ..ProjectNodeMeta::default()
        },
        data: None,
        children,
    }
}

fn recovery_test_project() -> ProjectFile {
    ProjectFile {
        version: PROJECT_FILE_VERSION.to_string(),
        ui_state: None,
        root: project_record(
            FOLDER_NODE_TYPE,
            "root",
            vec![project_record("recover_bad_init_node", "bad init", Vec::new())],
        ),
    }
}

#[test]
fn project_file_recovery_reports_and_skips_rebuild_error() {
    let project = recovery_test_project();
    let strict_error =
        match Engine::<RecoveryTestNode>::from_project_file_with(project.clone(), |node_type, _data, _meta| {
            match node_type {
                FOLDER_NODE_TYPE => Ok(Folder::new("root").into()),
                "recover_bad_init_node" => Ok(RecoverBadInitNode::new().into()),
                _ => Err(format!("unknown node type '{node_type}'")),
            }
        }) {
            Ok(_) => panic!("strict load should reject the load-time invalid edit"),
            Err(error) => error,
        };
    assert!(
        strict_error.to_string().contains("SetParam"),
        "strict error should describe the failed edit: {strict_error}"
    );

    let (loaded_engine, recovery) = Engine::<RecoveryTestNode>::from_project_file_with_recovery(
        project,
        |node_type, _data, _meta| match node_type {
            FOLDER_NODE_TYPE => Ok(Folder::new("root").into()),
            "recover_bad_init_node" => Ok(RecoverBadInitNode::new().into()),
            _ => Err(format!("unknown node type '{node_type}'")),
        },
    )
    .expect("recovering load should keep the valid graph");

    assert_eq!(loaded_engine.nodes.iter().count(), 2);
    assert_eq!(recovery.problems.len(), 1);
    assert!(
        recovery.problems[0].message.contains("SetParam"),
        "recovery problem should describe the skipped edit: {:?}",
        recovery.problems[0]
    );
}

#[test]
fn duplicate_without_contextual_catalog_avoids_whole_graph_snapshot() {
    let mut engine = Engine::<RecoveryTestNode>::new(Folder::new("root").into());
    engine.add_node(SnapshotFreeDuplicateNode::new().into(), None);
    engine.apply_edits().expect("source node should attach");
    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("source node should exist");

    DUPLICATE_SNAPSHOT_DESCRIPTOR_CALLS.store(0, Ordering::Relaxed);
    let duplicate = engine
        .duplicate_subtree_with(
            source,
            engine.root,
            Some(source),
            None,
            Node::project_encode_data,
            RecoveryTestNode::project_decode_node,
        )
        .expect("duplicate should succeed");

    assert_ne!(duplicate, source);
    assert_eq!(
        DUPLICATE_SNAPSHOT_DESCRIPTOR_CALLS.load(Ordering::Relaxed),
        0,
        "UI event construction should not snapshot unrelated graph state when no catalog needs context"
    );
}

#[test]
fn duplicate_decode_failure_does_not_mutate_the_live_engine() {
    let mut engine = Engine::<RecoveryTestNode>::new(Folder::new("root").into());
    engine.add_node(DecodeFallbackNode::new().into(), None);
    engine.apply_edits().expect("source root should attach");
    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("source root should exist");
    engine.add_node(DecodeFallbackNode::new().into(), Some(source));
    engine.apply_edits().expect("source child should attach");

    let before_node_count = engine.nodes.len();
    let before_uuid_index = engine.uuid_to_node_id_map();
    let before_root_links = engine.nodes.get(engine.root).map(|node| {
        let data = node.node_data();
        (data.first_child, data.last_child)
    });
    let before_source_links = engine.nodes.get(source).map(|node| {
        let data = node.node_data();
        (data.first_child, data.last_child)
    });
    let before_ui_event_count = engine.ui_event_log().len();
    let before_inbox_event_count = engine.inbox.events.len();
    let before_history = (engine.undo_len(), engine.redo_len(), engine.current_history_state_id());

    let mut decoded = 0usize;
    let error = engine
        .duplicate_subtree_with(
            source,
            engine.root,
            Some(source),
            None,
            Node::project_encode_data,
            |node_type, _data, _meta| {
                if node_type != "decode_fallback_node" {
                    return Err(format!("unexpected node type '{node_type}'"));
                }
                decoded += 1;
                if decoded == 2 {
                    Err("injected nested decode failure".to_string())
                } else {
                    Ok(DecodeFallbackNode::new().into())
                }
            },
        )
        .expect_err("nested codec failure should reject duplication");

    assert!(
        error.to_string().contains("injected nested decode failure"),
        "error should preserve the codec failure: {error}"
    );
    assert_eq!(decoded, 2, "both detached records should reach the decoder");
    assert_eq!(engine.nodes.len(), before_node_count);
    assert_eq!(engine.uuid_to_node_id_map(), before_uuid_index);
    assert_eq!(
        engine.nodes.get(engine.root).map(|node| {
            let data = node.node_data();
            (data.first_child, data.last_child)
        }),
        before_root_links
    );
    assert_eq!(
        engine.nodes.get(source).map(|node| {
            let data = node.node_data();
            (data.first_child, data.last_child)
        }),
        before_source_links
    );
    assert_eq!(engine.ui_event_log().len(), before_ui_event_count);
    assert_eq!(engine.inbox.events.len(), before_inbox_event_count);
    assert_eq!(
        (engine.undo_len(), engine.redo_len(), engine.current_history_state_id()),
        before_history
    );
}

#[test]
fn detached_duplicate_decode_uses_the_parents_decoded_intrinsic_configuration() {
    let mut engine = Engine::<RecoveryTestNode>::new(Folder::new("root").into());
    engine.add_node(DetachedFactoryParent::new(true).into(), None);
    engine.apply_edits().expect("source parent should attach");
    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("source parent should exist");
    engine.add_user_item(DetachedFactoryChild::new(true).into(), Some(source));
    engine.apply_edits().expect("source item should attach");

    let duplicate = engine
        .duplicate_subtree_with(
            source,
            engine.root,
            Some(source),
            None,
            Node::project_encode_data,
            RecoveryTestNode::project_decode_node,
        )
        .expect("duplicate should succeed");
    let duplicate_child = engine
        .nodes
        .get(duplicate)
        .and_then(|node| node.node_data().first_child)
        .expect("duplicated item should exist");

    let RecoveryTestNode::DetachedFactoryChild(child) = engine
        .nodes
        .get(duplicate_child)
        .expect("duplicated child should exist")
    else {
        panic!("duplicated item should use the parent factory");
    };
    assert!(
        child.created_from_decoded_parent,
        "the detached parent must be decoded before its child factory runs"
    );
}

#[test]
fn duplicate_invalid_sibling_does_not_allocate_an_orphan_node() {
    let mut engine = Engine::<RecoveryTestNode>::new(Folder::new("root").into());
    engine.add_node(SnapshotFreeDuplicateNode::new().into(), None);
    engine.apply_edits().expect("source root should attach");
    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("source root should exist");
    engine.add_node(SnapshotFreeDuplicateNode::new().into(), Some(source));
    engine.apply_edits().expect("source child should attach");
    let invalid_sibling = engine
        .nodes
        .get(source)
        .and_then(|node| node.node_data().first_child)
        .expect("nested source child should exist");

    let before_node_count = engine.nodes.len();
    let before_uuid_index = engine.uuid_to_node_id_map();
    let before_root_links = engine.nodes.get(engine.root).map(|node| {
        let data = node.node_data();
        (data.first_child, data.last_child)
    });

    let error = engine
        .duplicate_subtree_with(
            source,
            engine.root,
            Some(invalid_sibling),
            None,
            Node::project_encode_data,
            RecoveryTestNode::project_decode_node,
        )
        .expect_err("a sibling under another parent should reject duplication");

    assert!(
        matches!(
            error,
            ProjectPersistenceError::Engine(crate::engine::EngineEditError::InvalidSiblingParent { .. })
        ),
        "destination validation should report the invalid sibling parent"
    );
    assert_eq!(engine.nodes.len(), before_node_count);
    assert_eq!(engine.uuid_to_node_id_map(), before_uuid_index);
    assert_eq!(
        engine.nodes.get(engine.root).map(|node| {
            let data = node.node_data();
            (data.first_child, data.last_child)
        }),
        before_root_links
    );
}
