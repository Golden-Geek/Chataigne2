extern crate golden_engine as golden_core;

use golden_engine::{
    app::{ProjectLifecycle, from_sparse_project_json, to_sparse_project_json_pretty},
    application::apply_project_intent,
    engine::Engine,
    node::{DeclId, Folder, Gradient, GradientNode, Node, NodeData, NodeId, gradient_from_snapshot},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ProcessCtx,
    ui_sync::{UiAck, UiCreateUserItemInitialParam, UiEditIntent},
};
use std::{collections::HashSet, time::Duration};

struct GradientOwner {
    data: NodeData,
}

impl GradientOwner {
    fn new() -> Self {
        Self {
            data: NodeData::new("Gradient owner".into()),
        }
    }
}

impl Node for GradientOwner {
    fn node_data(&self) -> &NodeData {
        &self.data
    }
    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.data
    }
    fn get_type(&self) -> &str {
        "gradient_owner"
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "gradient_owner").then(Self::new)
    }
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        true
    }
    fn engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        let present = ctx.tree_snapshot().is_some_and(|snapshot| {
            snapshot
                .child_ids(self.id())
                .iter()
                .any(|id| snapshot.node(*id).is_some_and(|child| child.decl_id == "gradient"))
        });
        if !present {
            ctx.add_child_boxed(self.id(), Box::new(GradientNode::new()), None);
        }
    }
}

golden_engine::define_node_enum!(
    enum EditorNode {
        Owner(GradientOwner),
    }
);
impl ProjectLifecycle for EditorNode {}

fn fixture() -> (Engine<EditorNode>, NodeId) {
    let mut engine = Engine::<EditorNode>::new(Folder::new("Project").into());
    engine.add_node(GradientNode::new().into(), Some(engine.root));
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    assert_eq!(engine.nodes.get(id).unwrap().get_type(), "gradient");
    assert_eq!(model(&engine, id).stops().len(), 2);
    engine.clear_history();
    (engine, id)
}

fn apply(engine: &mut Engine<EditorNode>, intent: UiEditIntent) -> UiAck {
    let ack = apply_project_intent(engine, intent);
    assert!(ack.success, "{ack:?}");
    ack
}

fn child(engine: &Engine<EditorNode>, parent: NodeId, declaration: &str) -> NodeId {
    engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().parent == Some(parent) && node.node_data().meta.decl_id.0 == declaration)
        .unwrap()
        .0
}

fn model(engine: &Engine<EditorNode>, id: NodeId) -> Gradient {
    gradient_from_snapshot(&engine.process_tree_snapshot(), id).unwrap()
}

fn values(engine: &Engine<EditorNode>, stop: NodeId) -> [ParamValue; 3] {
    ["position", "color", "interpolation"].map(|declaration| {
        engine
            .nodes
            .get(child(engine, stop, declaration))
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value
    })
}

fn create(engine: &mut Engine<EditorNode>, parent: NodeId) -> golden_engine::ui_sync::UiCreatedNode {
    apply(
        engine,
        UiEditIntent::CreateUserItem {
            parent,
            node_type: "gradient_stop".into(),
            label: None,
            initial_params: vec![
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("position".into()),
                    value: ParamValue::Float(0.4),
                },
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("color".into()),
                    value: ParamValue::Color(0.2, 0.5, 0.8, 0.6),
                },
                UiCreateUserItemInitialParam {
                    decl_id: DeclId("interpolation".into()),
                    value: ParamValue::Enum("smooth".into()),
                },
            ],
        },
    )
    .created_node
    .unwrap()
}

#[test]
fn gradient_ui_creation_live_edits_and_release_are_exact_history_groups() {
    let (mut engine, gradient) = fixture();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    apply(
        &mut engine,
        UiEditIntent::BeginEdit {
            client_edit_id: "create".into(),
            label: Some("Add Gradient stop".into()),
        },
    );
    let created = create(&mut engine, gradient);
    assert_eq!(
        engine.nodes.get(created.node_id).unwrap().node_data().meta.uuid,
        created.uuid
    );
    assert_eq!(
        values(&engine, created.node_id),
        [
            ParamValue::Float(0.4),
            ParamValue::Color(0.2, 0.5, 0.8, 0.6),
            ParamValue::Enum("smooth".into())
        ]
    );
    assert_eq!(engine.undo_len(), 0);
    apply(
        &mut engine,
        UiEditIntent::EndEdit {
            client_edit_id: "create".into(),
        },
    );
    assert_eq!(engine.undo_len(), 1);
    let initialized = to_sparse_project_json_pretty(&engine).unwrap();
    apply(&mut engine, UiEditIntent::Undo);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    apply(&mut engine, UiEditIntent::Redo);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), initialized);

    apply(
        &mut engine,
        UiEditIntent::BeginEdit {
            client_edit_id: "drag".into(),
            label: Some("Move Gradient stop".into()),
        },
    );
    let position = child(&engine, created.node_id, "position");
    for value in [0.6, 0.3, 0.7] {
        apply(
            &mut engine,
            UiEditIntent::SetParam {
                node: position,
                value: ParamValue::Float(value),
                behaviour: ParameterEventBehaviour::Coalesce,
            },
        );
        assert_eq!(
            values(&engine, created.node_id)[0],
            ParamValue::Float(value),
            "live values precede release"
        );
    }
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(engine.undo_len(), 1);
    apply(
        &mut engine,
        UiEditIntent::EndEdit {
            client_edit_id: "drag".into(),
        },
    );
    assert_eq!(engine.undo_len(), 2);
    let dragged = to_sparse_project_json_pretty(&engine).unwrap();
    apply(&mut engine, UiEditIntent::Undo);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), initialized);
    apply(&mut engine, UiEditIntent::Redo);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), dragged);

    let owner_uuid = engine.nodes.get(gradient).unwrap().node_data().meta.uuid;
    let restored = from_sparse_project_json::<EditorNode>(&dragged).unwrap();
    let restored_gradient = restored.node_id_by_uuid(owner_uuid).unwrap();
    let restored_stop = restored.node_id_by_uuid(created.uuid).unwrap();
    assert_eq!(model(&restored, restored_gradient), model(&engine, gradient));
    assert_eq!(values(&restored, restored_stop), values(&engine, created.node_id));
}

#[test]
fn gradient_ui_copy_remove_and_invalid_initialization_preserve_atomic_documents() {
    let (mut engine, gradient) = fixture();
    let created = create(&mut engine, gradient);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let rejected = apply_project_intent(
        &mut engine,
        UiEditIntent::CreateUserItem {
            parent: gradient,
            node_type: "gradient_stop".into(),
            label: None,
            initial_params: vec![UiCreateUserItemInitialParam {
                decl_id: DeclId("missing".into()),
                value: ParamValue::Float(0.2),
            }],
        },
    );
    assert!(!rejected.success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);

    let original_ids: HashSet<_> = engine.nodes.iter().map(|(id, _)| id).collect();
    apply(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source: created.node_id,
            new_parent: gradient,
            new_prev_sibling: Some(created.node_id),
            initial_params: vec![],
        },
    );
    let copies: Vec<_> = engine
        .nodes
        .iter()
        .filter(|(id, node)| !original_ids.contains(id) && node.node_data().parent == Some(gradient))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(copies.len(), 1);
    let copy = copies[0];
    assert_eq!(values(&engine, copy), values(&engine, created.node_id));
    assert_ne!(engine.nodes.get(copy).unwrap().node_data().meta.uuid, created.uuid);
    for declaration in ["position", "color", "interpolation"] {
        assert_ne!(
            engine
                .nodes
                .get(child(&engine, copy, declaration))
                .unwrap()
                .node_data()
                .meta
                .uuid,
            engine
                .nodes
                .get(child(&engine, created.node_id, declaration))
                .unwrap()
                .node_data()
                .meta
                .uuid
        );
    }
    let copied = to_sparse_project_json_pretty(&engine).unwrap();
    apply(
        &mut engine,
        UiEditIntent::RemoveNodes {
            nodes: vec![copy, created.node_id],
        },
    );
    assert_eq!(model(&engine, gradient).stops().len(), 2);
    apply(&mut engine, UiEditIntent::Undo);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), copied);
    apply(&mut engine, UiEditIntent::Redo);
    assert_eq!(model(&engine, gradient).stops().len(), 2);
}

#[test]
fn removing_every_gradient_stop_survives_history_and_project_reload() {
    let (mut engine, gradient) = fixture();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let owner = engine.nodes.get(gradient).unwrap().node_data().meta.uuid;
    let nodes = engine.process_tree_snapshot().child_ids(gradient).to_vec();
    apply(&mut engine, UiEditIntent::RemoveNodes { nodes });
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert!(model(&engine, gradient).is_empty());
    apply(&mut engine, UiEditIntent::Undo);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    apply(&mut engine, UiEditIntent::Redo);
    assert!(model(&engine, gradient).is_empty());
    let root = engine.root;
    apply(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source: gradient,
            new_parent: root,
            new_prev_sibling: Some(gradient),
            initial_params: vec![],
        },
    );
    let copied = engine
        .nodes
        .iter()
        .find(|(id, node)| *id != gradient && node.get_type() == "gradient")
        .unwrap()
        .0;
    assert!(model(&engine, copied).is_empty());
    let copied_owner = engine.nodes.get(copied).unwrap().node_data().meta.uuid;
    let empty = to_sparse_project_json_pretty(&engine).unwrap();
    let restored = from_sparse_project_json::<EditorNode>(&empty).unwrap();
    assert!(model(&restored, restored.node_id_by_uuid(owner).unwrap()).is_empty());
    assert!(model(&restored, restored.node_id_by_uuid(copied_owner).unwrap()).is_empty());
}

#[test]
fn authored_history_restore_preserves_fresh_gradient_initialization() {
    let mut gradient = GradientNode::new();
    let authored = gradient.engine_capture_authored_data().unwrap();
    gradient.engine_restore_authored_data(authored).unwrap();
    assert!(
        gradient
            .project_decode_data(&serde_json::json!({ "unsupported": true }))
            .is_err()
    );
    let mut engine = Engine::<EditorNode>::new(Folder::new("Project").into());
    engine.add_node(gradient.into(), Some(engine.root));
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    assert_eq!(model(&engine, id).stops().len(), 2);
}

#[test]
fn an_empty_declared_gradient_overrides_its_owners_schema_defaults_on_reload() {
    let mut engine = Engine::<EditorNode>::new(Folder::new("Project").into());
    engine.add_node(GradientOwner::new().into(), Some(engine.root));
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    let owner = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let gradient = child(&engine, owner, "gradient");
    assert_eq!(model(&engine, gradient).stops().len(), 2);
    let owner_uuid = engine.nodes.get(owner).unwrap().node_data().meta.uuid;
    let initial = engine.process_tree_snapshot().child_ids(gradient).to_vec();
    let remaining_uuid = engine.nodes.get(initial[1]).unwrap().node_data().meta.uuid;
    apply(&mut engine, UiEditIntent::RemoveNode { node: initial[0] });
    let subset = to_sparse_project_json_pretty(&engine).unwrap();
    let restored_subset = from_sparse_project_json::<EditorNode>(&subset).unwrap();
    let subset_owner = restored_subset.node_id_by_uuid(owner_uuid).unwrap();
    assert_eq!(
        model(&restored_subset, child(&restored_subset, subset_owner, "gradient"))
            .stops()
            .len(),
        1
    );
    assert!(restored_subset.node_id_by_uuid(remaining_uuid).is_some());
    apply(&mut engine, UiEditIntent::Undo);
    let nodes = engine.process_tree_snapshot().child_ids(gradient).to_vec();
    apply(&mut engine, UiEditIntent::RemoveNodes { nodes });
    assert!(model(&engine, gradient).is_empty());
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let restored = from_sparse_project_json::<EditorNode>(&saved).unwrap();
    let restored_owner = restored.node_id_by_uuid(owner_uuid).unwrap();
    assert!(model(&restored, child(&restored, restored_owner, "gradient")).is_empty());

    let root = engine.root;
    apply(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source: owner,
            new_parent: root,
            new_prev_sibling: Some(owner),
            initial_params: vec![],
        },
    );
    let copied_owner = engine
        .nodes
        .iter()
        .find(|(id, node)| *id != owner && node.get_type() == "gradient_owner")
        .unwrap()
        .0;
    assert!(model(&engine, child(&engine, copied_owner, "gradient")).is_empty());
}
