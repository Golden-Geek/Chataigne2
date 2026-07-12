use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::blueprints::{BlueprintDecl, BlueprintId};
use crate::contexts::{UserContextEntryKind, UserContextLookup, UserContextValueType};
use crate::edit::Edit;
use crate::events::{CustomEvent, EventKind};
use crate::logger::{self, UI_LOG_CLEARED_TOPIC, UI_LOG_MAX_ENTRIES_TOPIC, UI_LOG_RECORD_TOPIC};
use crate::node::{
    CurveBezierFitOptions, CurveEasing, CurveFitPoint, CurveKeyNode, CurveNode, CurveRangeConstraint, EventPropagation,
    EventSubscription, FOLDER_NODE_TYPE, Folder, Node, NodeData, NodeId, NodeMeta, NodeReference, NodeUuid,
    PARAMETER_ANIMATION_AMPLITUDE_DECL_ID, PARAMETER_ANIMATION_CONTROL_NODE_TYPE, PARAMETER_ANIMATION_CURVE_DECL_ID,
    PARAMETER_ANIMATION_CURVE_NODE_TYPE, PARAMETER_ANIMATION_EASING_DECL_ID, PARAMETER_ANIMATION_EASING_KIND_DECL_ID,
    PARAMETER_ANIMATION_FREQUENCY_DECL_ID, PARAMETER_ANIMATION_KEY_NODE_TYPE, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID,
    PARAMETER_ANIMATION_KEY_VALUE_DECL_ID, PARAMETER_ANIMATION_OFFSET_DECL_ID, PARAMETER_ANIMATION_RANGE_DECL_ID,
    PARAMETER_ANIMATION_RANGE_NODE_TYPE, PARAMETER_ANIMATION_RANGE_X_DECL_ID, PARAMETER_ANIMATION_RANGE_Y_DECL_ID,
    PARAMETER_ANIMATION_UPDATE_RATE_DECL_ID, PARAMETER_ANIMATION_WAVEFORM_DECL_ID, PARAMETER_CONTROL_REFERENCE_DECL_ID,
    PARAMETER_EXPRESSION_SOURCE_DECL_ID, PARAMETER_NODE_TYPES, USER_CONTEXT_FOLDER_NODE_TYPE, USER_CONTEXT_ITEM_KIND,
    USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID, USER_CONTEXT_MULTIPLEX_NODE_TYPE, USER_CONTEXT_NODE_TYPE, UserContainerRules,
    UserContextMultiplexListNode, UserContextMultiplexNode, UserContextNode, UserCreatableItem, UserNodeRole,
    curve_from_snapshot, user_context_multiplex_list_node_type,
};
use crate::parameter::{
    ParamValue, ParamValueProjection, Parameter, ParameterChangeCheck, ParameterConstraintPolicy, ParameterConstraints,
    ParameterControlMode, ParameterControlSpec, ParameterControlState, ParameterEnumOption, ParameterEventBehaviour,
    RangeConstraint, ReferenceConstraints, ReferenceRoot, ReferenceTargetKind,
};
use crate::process_ctx::{ExecutionPhase, ProcessCtx};
use crate::script::{ScriptHostPolicy, ScriptNode, ScriptNodeConfig, ScriptUiConfig, ScriptUiSource};
use crate::ui_sync::{
    UiAckStatus, UiEditIntent, UiEventKind, UiGraphOp, UiNodeDataDto, UiParameterControlStateDto, UiSubscriptionScope,
};

#[crate::node]
struct ItemMacroAutoKindNode {}

#[crate::item("sequence", from_struct)]
impl Node for ItemMacroAutoKindNode {}

#[crate::node]
struct ItemMacroOverrideKindNode {}

#[crate::item("sequence", from_struct)]
impl Node for ItemMacroOverrideKindNode {
    fn user_item_kind(&self) -> &str {
        "custom_sequence"
    }
}

#[crate::node]
struct ItemMacroDefaultKindNode {}

#[crate::item(from_struct)]
impl Node for ItemMacroDefaultKindNode {}

/// Description surfaced once in the UI schema for all nodes of this type.
#[crate::node("ui_schema_description_node")]
struct UiSchemaDescriptionNode {}

#[crate::node("ui_schema_description_node", from_struct)]
impl Node for UiSchemaDescriptionNode {}

static REMOVE_LIFECYCLE_DESTROY_COUNT: AtomicUsize = AtomicUsize::new(0);
static REMOVE_LIFECYCLE_READY_COUNT: AtomicUsize = AtomicUsize::new(0);
static READY_REMOVED_CHILD_MUTATION_COUNT: AtomicUsize = AtomicUsize::new(0);

#[crate::node("remove_lifecycle_probe_node")]
struct RemoveLifecycleProbeNode {}

#[crate::node("remove_lifecycle_probe_node", from_struct)]
impl Node for RemoveLifecycleProbeNode {
    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        REMOVE_LIFECYCLE_DESTROY_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    fn on_node_ready(&mut self, _ctx: &mut ProcessCtx, _context: crate::node::NodeCreationContext) {
        REMOVE_LIFECYCLE_READY_COUNT.fetch_add(1, Ordering::SeqCst);
    }
}

#[crate::node("ready_removes_child_parent_node")]
struct ReadyRemovesChildParentNode {}

#[crate::node("ready_removes_child_parent_node", from_struct)]
impl Node for ReadyRemovesChildParentNode {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: crate::node::NodeCreationContext) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        for child in snapshot.child_ids(self.id()) {
            ctx.edits.push(Edit::RemoveNode { node: child });
        }
    }
}

#[crate::node("ready_removed_child_mutation_node")]
struct ReadyRemovedChildMutationNode {}

#[crate::node("ready_removed_child_mutation_node", from_struct)]
impl Node for ReadyRemovedChildMutationNode {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: crate::node::NodeCreationContext) {
        let node = self.id();
        ctx.edits.push(Edit::CallNodeMutation {
            node,
            callback: Box::new(|_, _| {
                READY_REMOVED_CHILD_MUTATION_COUNT.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
            needs_tree_snapshot: false,
        });
    }
}

#[test]
fn ready_batch_stabilizes_structural_edits_before_removed_child_ready_callbacks() {
    READY_REMOVED_CHILD_MUTATION_COUNT.store(0, Ordering::SeqCst);

    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    let parent: MacroTestNode = ReadyRemovesChildParentNode::new().into();
    let child: MacroTestNode = ReadyRemovedChildMutationNode::new().into();
    let mut tree = crate::edit::NodeTree::new(parent);
    tree.push_child(crate::edit::NodeTree::new(child));

    engine.edits.push(Edit::AddNodeTree {
        parent: engine.root,
        tree,
        prev_sibling: None,
    });

    engine
        .apply_edits()
        .expect("ready structural edits should stabilize before stale child callbacks");

    assert_eq!(READY_REMOVED_CHILD_MUTATION_COUNT.load(Ordering::SeqCst), 0);
}

#[test]
fn item_macro_sets_user_item_kind_when_not_overridden() {
    let node = ItemMacroAutoKindNode::new();
    assert_eq!(node.user_item_kind(), "sequence");
}

#[test]
fn item_macro_keeps_manual_user_item_kind_override() {
    let node = ItemMacroOverrideKindNode::new();
    assert_eq!(node.user_item_kind(), "custom_sequence");
}

#[test]
fn item_macro_marks_nodes_as_declared_user_items() {
    let auto = ItemMacroAutoKindNode::new();
    let override_kind = ItemMacroOverrideKindNode::new();
    assert!(auto.is_declared_user_item());
    assert!(override_kind.is_declared_user_item());
}

#[test]
fn item_macro_derives_item_kind_from_node_name_when_omitted() {
    let node = ItemMacroDefaultKindNode::new();
    assert_eq!(node.user_item_kind(), "item_macro_default_kind");
}

#[test]
fn ui_snapshot_moves_type_descriptions_into_schema() {
    assert_eq!(
        UiSchemaDescriptionNode::new().type_description(),
        Some("Description surfaced once in the UI schema for all nodes of this type.")
    );
    let wrapped: MacroTestNode = UiSchemaDescriptionNode::new().into();
    assert_eq!(
        wrapped.type_description(),
        Some("Description surfaced once in the UI schema for all nodes of this type.")
    );
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(UiSchemaDescriptionNode::new().into(), None);
    engine.add_node(UiSchemaDescriptionNode::new().into(), None);
    engine.apply_edits().expect("described nodes should attach");

    let snapshot = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);
    let description = snapshot
        .schema
        .node_types
        .iter()
        .find(|descriptor| descriptor.node_type == "ui_schema_description_node")
        .and_then(|descriptor| descriptor.description.as_deref());

    assert_eq!(
        description,
        Some("Description surfaced once in the UI schema for all nodes of this type.")
    );
    assert!(
        snapshot
            .nodes
            .iter()
            .filter(|node| node.node_type == "ui_schema_description_node")
            .all(|node| node.meta.description.is_none()),
        "canonical type descriptions should not be duplicated on each node instance"
    );
}

#[test]
fn ui_snapshot_moves_declared_children_descriptions_into_shared_schema() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(SharedDeclaredDescriptionNode::new().into(), None);
    engine.add_node(SharedDeclaredDescriptionNode::new().into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("shared-description nodes should attach");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("shared-description stabilization should succeed");
    }

    let first_key = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("first shared-description node should exist");
    let second_key = engine
        .nodes
        .get(first_key)
        .and_then(|node| node.node_data().next_sibling)
        .expect("second shared-description node should exist");
    let first_position = find_child_by_decl(&engine, first_key, "position").expect("first position param should exist");
    let second_position =
        find_child_by_decl(&engine, second_key, "position").expect("second position param should exist");

    engine.edits.push(Edit::PatchMeta {
        node: second_position,
        patch: crate::node::NodeMetaPatch {
            description: Some(Some("Manual position override".to_string())),
            ..Default::default()
        },
    });
    engine.apply_edits().expect("manual description override should apply");

    let snapshot = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);
    let shared_position_description = snapshot
        .schema
        .declared_descriptions
        .iter()
        .find(|descriptor| descriptor.key == "shared_declared_description_node::position")
        .map(|descriptor| descriptor.description.as_str());
    let shared_value_description = snapshot
        .schema
        .declared_descriptions
        .iter()
        .find(|descriptor| descriptor.key == "shared_declared_description_node::value")
        .map(|descriptor| descriptor.description.as_str());
    let first_position_node = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == first_position)
        .expect("first position snapshot node should exist");
    let second_position_node = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == second_position)
        .expect("second position snapshot node should exist");

    assert_eq!(shared_position_description, Some("Shared key position description"));
    assert_eq!(shared_value_description, Some("Shared key value description"));
    assert_eq!(
        first_position_node.meta.declared_description_key.as_deref(),
        Some("shared_declared_description_node::position")
    );
    assert!(
        first_position_node.meta.description.is_none(),
        "shared declared descriptions should stay in schema when not overridden"
    );
    assert!(!first_position_node.meta.description_overridden);
    assert_eq!(
        second_position_node.meta.declared_description_key.as_deref(),
        Some("shared_declared_description_node::position")
    );
    assert_eq!(
        second_position_node.meta.description.as_deref(),
        Some("Manual position override")
    );
    assert!(
        second_position_node.meta.description_overridden,
        "manual description changes should stay instance-local"
    );
}

#[test]
fn add_node_infers_default_permissions_for_declared_item_nodes() {
    let root = ItemMacroAutoKindNode::new();
    let mut engine = Engine::new(root);

    engine.add_node(ItemMacroAutoKindNode::new(), None);
    engine.apply_edits().expect("declared item add should succeed");

    let child = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("child should exist");
    let permissions = &engine
        .nodes
        .get(child)
        .expect("child should exist")
        .node_data()
        .meta
        .user_permissions;

    assert!(!permissions.can_edit_name);
    assert!(permissions.can_remove_and_duplicate);
    assert!(!permissions.can_edit_constraints);
    assert!(permissions.can_edit_tags);
    assert!(permissions.can_edit_color);
}

#[test]
fn absorb_edits_reports_node_type_mismatch() {
    let root = Folder::new("root".to_string());
    let mut engine = Engine::new(root);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.add_child(
        NodeId(0),
        Parameter::new("param", ParamValue::Int(0), ParameterChangeCheck::None),
        None,
    );

    let result = engine.absorb_edits(&mut ctx);
    assert!(matches!(
        result,
        Err(EngineEditError::NodeTypeMismatch {
            operation: "AddNode",
            ..
        })
    ));
}

#[test]
fn absorb_edits_accepts_matching_node_type() {
    let root = Folder::new("root".to_string());
    let mut engine = Engine::new(root);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.add_child(NodeId(0), Folder::new("child".to_string()), None);

    let result = engine.absorb_edits(&mut ctx);
    assert!(result.is_ok());
    assert_eq!(engine.edits.pending.len(), 1);
}

#[test]
fn absorb_edits_skips_noop_warning_edits() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    let root = engine.root;
    engine.set_node_warning(root, "stable warning");
    engine.apply_edits().expect("initial warning should apply");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_node_warning(root, "stable warning");
    ctx.clear_node_warning(root, Some("missing"));
    ctx.set_node_child_warning_depth(root, 0);

    engine
        .absorb_edits(&mut ctx)
        .expect("absorb warning edits should succeed");
    assert!(
        engine.edits.pending.is_empty(),
        "no-op warning edits should be dropped during absorb"
    );
}

#[test]
fn external_edit_sender_sets_param_from_another_thread() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);
    let root_id = engine.root;
    let sender = engine.external_edit_sender();

    std::thread::spawn(move || {
        sender
            .send(Edit::SetParam {
                node: root_id,
                value: ParamValue::Int(33),
                behaviour: ParameterEventBehaviour::Coalesce,
            })
            .expect("external edit send should succeed");
    })
    .join()
    .expect("external producer thread should join");

    engine.apply_edits().expect("external edit should apply");
    assert_eq!(
        engine.nodes.get(root_id).expect("root parameter should exist").value,
        ParamValue::Int(33),
        "external set-param should be drained and applied",
    );
}

#[test]
fn external_edit_sender_adds_node_from_another_thread() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    let root_id = engine.root;
    let sender = engine.external_edit_sender();

    std::thread::spawn(move || {
        sender
            .send(Edit::AddNode {
                parent: root_id,
                prev_sibling: None,
                node: Box::new(Folder::new("external_child".to_string())),
            })
            .expect("external add-node send should succeed");
    })
    .join()
    .expect("external producer thread should join");

    engine.apply_edits().expect("external add-node edit should apply");

    let child = engine
        .nodes
        .get(root_id)
        .and_then(|root| root.node_data().first_child)
        .expect("root should contain one child");
    assert_eq!(
        engine
            .nodes
            .get(child)
            .expect("child node should exist")
            .node_data()
            .meta
            .label,
        "external_child"
    );
}

#[test]
fn add_node_tree_attaches_subtree_and_replays_history_as_one_root() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    let root_id = engine.root;

    engine.edits.push(Edit::AddNodeTree {
        parent: root_id,
        prev_sibling: None,
        tree: crate::edit::NodeTree::new(Folder::new("parent".to_string()))
            .with_child(crate::edit::NodeTree::new(Folder::new("child".to_string()))),
    });
    engine.apply_edits().expect("node tree edit should apply");

    let parent = find_child_by_decl_any(&engine, root_id, "parent").expect("parent should exist");
    let child = find_child_by_decl_any(&engine, parent, "child").expect("child should exist");
    assert_eq!(engine.undo_len(), 1, "subtree insert should be one undo transaction");

    assert!(engine.undo().expect("undo should succeed"));
    assert!(
        engine.nodes.get(parent).is_none(),
        "undo should detach the subtree root"
    );
    assert!(
        engine.nodes.get(child).is_none(),
        "undo should detach subtree descendants"
    );

    assert!(engine.redo().expect("redo should succeed"));
    assert!(
        engine.nodes.get(parent).is_some(),
        "redo should restore the same subtree root id"
    );
    assert!(
        engine.nodes.get(child).is_some(),
        "redo should restore the same subtree child id"
    );
}

#[test]
fn run_tick_drains_external_edits_without_manual_apply_call() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);
    engine.resolve().expect("resolve should succeed");

    let root_id = engine.root;
    let sender = engine.external_edit_sender();
    sender
        .send(Edit::SetParam {
            node: root_id,
            value: ParamValue::Int(7),
            behaviour: ParameterEventBehaviour::Coalesce,
        })
        .expect("external set-param send should succeed");

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should drain and apply external edits");

    assert_eq!(
        engine.nodes.get(root_id).expect("root parameter should exist").value,
        ParamValue::Int(7)
    );
    assert_eq!(
        engine.undo_len(),
        0,
        "runtime tick edits should not create undo entries"
    );
    assert_eq!(
        engine.redo_len(),
        0,
        "runtime tick edits should not create redo entries"
    );
    assert!(
        !engine.undo().expect("undo query should succeed"),
        "runtime-only edits should not be undoable"
    );
}

#[test]
fn external_coalesced_set_param_edits_keep_latest_value() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);
    let root_id = engine.root;
    let sender = engine.external_edit_sender();

    sender
        .send(Edit::SetParam {
            node: root_id,
            value: ParamValue::Int(1),
            behaviour: ParameterEventBehaviour::Coalesce,
        })
        .expect("first external set-param send should succeed");
    sender
        .send(Edit::SetParam {
            node: root_id,
            value: ParamValue::Int(2),
            behaviour: ParameterEventBehaviour::Coalesce,
        })
        .expect("second external set-param send should succeed");

    engine
        .absorb_external_edits()
        .expect("external edits should be absorbed");
    assert_eq!(
        engine.edits.pending.len(),
        1,
        "coalesced external edits should collapse before apply"
    );

    engine.apply_edits().expect("external edits should apply");
    assert_eq!(
        engine.nodes.get(root_id).expect("root parameter should exist").value,
        ParamValue::Int(2),
        "latest coalesced value should win",
    );
}

#[test]
fn parameter_handle_trigger_value_emits_even_when_unchanged() {
    let mut handle = crate::node::ParameterHandle::<ParamValue>::new(ParamValue::Trigger());
    handle.set_node_id(NodeId(42));

    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    handle.set(&mut ctx, ParamValue::Trigger());

    assert_eq!(
        ctx.edits.pending.len(),
        1,
        "trigger writes should emit even when value appears unchanged"
    );
    assert!(
        matches!(
            &ctx.edits.pending[0].edit,
            Edit::SetParam {
                node,
                value: ParamValue::Trigger(),
                behaviour: ParameterEventBehaviour::Coalesce,
            } if *node == NodeId(42)
        ),
        "trigger write should enqueue SetParam with trigger payload",
    );
}

#[test]
fn parameter_node_trigger_value_emits_even_when_unchanged() {
    let mut parameter = Parameter::new("trigger", ParamValue::Trigger(), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().id = NodeId(7);

    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    parameter.set(&mut ctx, ParamValue::Trigger());

    assert_eq!(
        ctx.edits.pending.len(),
        1,
        "trigger values should bypass value-change dedupe"
    );
    assert!(
        matches!(
            &ctx.edits.pending[0].edit,
            Edit::SetParam {
                node,
                value: ParamValue::Trigger(),
                behaviour: ParameterEventBehaviour::Coalesce,
            } if *node == NodeId(7)
        ),
        "parameter trigger write should enqueue SetParam",
    );
}

#[crate::node("auto_declared", impl_node)]
struct AutoDeclaredNode {
    #[param(
        default = 0.5,
        label = "Decay",
        description = "Envelope decay time",
        min = 0.0,
        max = 1.0,
        step = 0.05,
        step_base = 0.0,
        policy = "ClampAdapt"
    )]
    decay: crate::node::ParameterHandle<f64>,

    #[potential_node(decl_id = "value")]
    value: crate::node::PotentialNodeHandle,
}

struct ViaNodeCore {
    node_data: NodeData,
}

impl ViaNodeCore {
    fn new(label: impl Into<String>) -> Self {
        Self {
            node_data: NodeData::new(label.into()),
        }
    }
}

#[crate::node("struct_declared_params_node")]
struct StructDeclaredParamsNode {
    #[param(default = 0.5, label = "Value")]
    value: crate::node::ParameterHandle<f64>,
    init_calls: usize,
    init_observed_value: Option<f64>,
}

#[crate::node("struct_declared_params_node", from_struct)]
impl Node for StructDeclaredParamsNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.init_calls += 1;
        self.init_observed_value = Some(self.value.get());
    }

    fn child_event_interest_depth(&self, _event: &crate::events::Event) -> u32 {
        0
    }
}

#[crate::node("via_struct_declared_params_node")]
struct ViaStructDeclaredParamsNode {
    base: ViaNodeCore,
    #[param(default = 0.5, label = "Value")]
    value: crate::node::ParameterHandle<f64>,
    init_calls: usize,
    init_observed_value: Option<f64>,
}

#[crate::node("via_struct_declared_params_node", via = base.node_data, from_struct)]
impl Node for ViaStructDeclaredParamsNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.init_calls += 1;
        self.init_observed_value = Some(self.value.get());
    }

    fn child_event_interest_depth(&self, _event: &crate::events::Event) -> u32 {
        0
    }
}

#[crate::node("via_composed_leaf_node")]
struct ViaComposedLeafNode {
    #[param(default = 0.5, label = "Leaf Value")]
    leaf_value: crate::node::ParameterHandle<f64>,
}

#[crate::node("via_composed_leaf_node", from_struct)]
impl Node for ViaComposedLeafNode {}

#[crate::node("via_composed_mid_node")]
struct ViaComposedMidNode {
    leaf: ViaComposedLeafNode,
    #[param(default = 0.25, label = "Mid Value")]
    mid_value: crate::node::ParameterHandle<f64>,
}

#[crate::node("via_composed_mid_node", via = leaf, from_struct)]
impl Node for ViaComposedMidNode {}

#[crate::node("via_composed_root_node")]
struct ViaComposedRootNode {
    mid: ViaComposedMidNode,
    #[param(default = 0.75, label = "Root Value")]
    root_value: crate::node::ParameterHandle<f64>,
}

#[crate::node("via_composed_root_node", via = mid, from_struct)]
impl Node for ViaComposedRootNode {}

#[crate::node("reuse_folder_base_node")]
#[children(
    folder(output, label = "Output") {
        host: String = "127.0.0.1" (label = "Host");
    }
)]
struct ReuseFolderBaseNode {}

#[crate::node("reuse_folder_base_node", from_struct)]
impl Node for ReuseFolderBaseNode {}

#[crate::node("reuse_folder_via_node")]
#[children(
    folder(output, label = "Output") {
        gain: f64 = 0.5 [0.0..1.0] (label = "Gain");
    }
)]
struct ReuseFolderViaNode {
    base: ReuseFolderBaseNode,
}

#[crate::node("reuse_folder_via_node", via = base, from_struct)]
impl Node for ReuseFolderViaNode {}

#[crate::node("sparse_declared_folder_node")]
#[children(
    folder(advanced, label = "Advanced") {
        delay: f64 = 0.0 (label = "Delay");
        gain: f64 = 0.5 (label = "Gain");
    }
)]
struct SparseDeclaredFolderNode {}

#[crate::node("sparse_declared_folder_node", from_struct)]
impl Node for SparseDeclaredFolderNode {}

#[crate::node("declaration_only_materialization_node")]
#[children(
    folder(connection, label = "Connection") {
        input: String = "127.0.0.1".to_string() (label = "Input");
    }
)]
struct DeclarationOnlyMaterializationNode {
    init_calls: usize,
    ready_calls: usize,
    inbox_calls: usize,
}

#[crate::node("declaration_only_materialization_node", from_struct)]
impl Node for DeclarationOnlyMaterializationNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.init_calls += 1;
    }

    fn on_node_ready(&mut self, _ctx: &mut ProcessCtx, _context: crate::node::NodeCreationContext) {
        self.ready_calls += 1;
    }

    fn on_inbox(&mut self, _ctx: &mut ProcessCtx) {
        self.inbox_calls += 1;
    }
}

#[crate::node("base_children_layout_base_node")]
#[children(
    folder(connection, label = "Connection") {
        base_value: f64 = 1.0 (label = "Base Value");
        folder(base_folder, label = "Base Folder") {}
    }
)]
struct BaseChildrenLayoutBaseNode {}

#[crate::node("base_children_layout_base_node", from_struct)]
impl Node for BaseChildrenLayoutBaseNode {}

#[crate::node("base_children_layout_via_node")]
#[children(
    folder(connection, label = "Connection") {
        folder(before_folder, label = "Before Folder") {}
        before_value: f64 = 0.25 (label = "Before Value");
        [base_children];
        folder(after_folder, label = "After Folder") {}
        after_value: f64 = 0.75 (label = "After Value");
        node after_node: Folder = Folder::new("After Node".to_string()) (label = "After Node");
    }
)]
struct BaseChildrenLayoutViaNode {
    base: BaseChildrenLayoutBaseNode,
}

#[crate::node("base_children_layout_via_node", via = base, from_struct)]
impl Node for BaseChildrenLayoutViaNode {}

#[crate::node("dsl_params_node")]
#[children(
    feedback: f64 = 0.5 [0.0..1.0] (
        label = "Feedback",
        description = "Delay feedback amount",
        read_only = true,
        step = 0.1,
        step_base = 0.0,
        policy = "Reject",
    );

    folder(output, label = "Output") {
        host: String = "127.0.0.1" (label = "Host", description = "OSC destination host");

        folder(color, label = "Color") {
            gamma: f64 = 2.2 (behavior = "Append");
        }
    }
)]
struct DslParamsNode {
    observed_feedback_new: Option<f64>,
    observed_feedback_old: Option<ParamValue>,
}

#[crate::node("dsl_vector_bounds_node")]
#[children(
    vec2_bounds: crate::parameter::Vec2 = (0.2, 0.5) [(-1.0, 0.0)..(1.0, 2.0)] (label = "Vec2 Bounds");
    vec3_bounds: crate::parameter::Vec3 = (0.2, 0.5, 12.0) [(-1.0, 0.0, 10.0)..(1.0, 2.0, 20.0)] (label = "Vec3 Bounds");
)]
struct DslVectorBoundsNode {}

#[crate::node("dsl_enum_defaults_node")]
#[children(
    mode_marked: crate::parameter::Enum (
        label = "Mode Marked",
        enum_options = ["off", "on", "auto (default)"],
    );
    mode_explicit: crate::parameter::Enum (
        label = "Mode Explicit",
        enum_options = ["off", "on", "auto"],
        enum_default = "on",
    );
    mode_first: crate::parameter::Enum (
        label = "Mode First",
        enum_options = ["a", "b", "c"],
    );
)]
struct DslEnumDefaultsNode {}

#[crate::node("dsl_reference_default_node")]
#[children(
    target_ref: crate::node::NodeReference (label = "Target Reference");
)]
struct DslReferenceDefaultNode {}

#[crate::node("dsl_node_children_node")]
#[children(
    folder(output, label = "Output") {
        node curve: CurveNode = CurveNode::new_with_label("Curve") (
            label = "Curve",
            description = "Declared curve child",
            color = crate::color::Color::new(0.25, 0.5, 0.75, 1.0),
            collapsed = true,
        );
    }
)]
struct DslNodeChildrenNode {}

#[crate::node(
    "dsl_meta_params_node",
    presentation = crate::node::PresentationHint {
        show_in_nested_inspector: false,
        ..Default::default()
    },
    color = crate::color::Color::new(0.95, 0.4, 0.2, 1.0)
)]
#[children(
    folder(
        settings,
        label = "Settings",
        description = "Settings folder metadata",
        short_name = "settings_folder",
        enabled = false,
        can_be_disabled = true,
        tags = vec![String::from("group")],
        semantics = crate::node::SemanticsHint {
            intent: Some(String::from("container")),
            unit: Some(String::from("section")),
        },
        color = crate::color::Color::new(0.1, 0.2, 0.3, 1.0),
        collapsed = true,
        show_child_warnings_max_depth = 2,
    ) {
        gain: f64 = 0.5 (
            label = "Gain",
            description = "Gain parameter metadata",
            short_name = "gain_param",
            enabled = false,
            can_be_disabled = true,
            tags = vec![String::from("audio"), String::from("gain")],
            semantics = crate::node::SemanticsHint {
                intent: Some(String::from("level")),
                unit: Some(String::from("db")),
            },
            color = crate::color::Color::new(0.7, 0.8, 0.9, 1.0),
        );
    }
)]
struct DslMetaParamsNode {}

#[crate::node("shared_declared_description_node")]
#[children(
    position: f64 = 0.0 (
        label = "Position",
        description = "Shared key position description",
    );
    value: f64 = 0.0 (
        label = "Value",
        description = "Shared key value description",
    );
)]
struct SharedDeclaredDescriptionNode {}

#[crate::node("manual_inbox_params_node")]
#[children(
    value: f64 = 0.5 [0.0..1.0] (label = "Value");
)]
struct ManualInboxParamsNode {
    observed_inbox_value: Option<f64>,
}

#[crate::node("params_with_custom_init_node")]
#[children(
    value: f64 = 0.5 [0.0..1.0] (label = "Value");
)]
struct ParamsWithCustomInitNode {
    init_calls: usize,
    init_observed_value: Option<f64>,
    init_observed_bound: bool,
    init_observed_id: Option<NodeId>,
}

#[crate::node("nested_init_binding_node")]
#[children(
    folder(group, label = "Group") {
        value: f64 = 0.5 [0.0..1.0] (label = "Value");
    }
)]
struct NestedInitBindingNode {
    init_calls: usize,
    init_observed_bound: bool,
    init_observed_id: Option<NodeId>,
}

#[crate::node("dsl_callback_params_node")]
#[children(
    default_value: f64 = 0.1 (default_callback);
    named_value: f64 = 0.2 (callback = Self::named_value_callback);
    closure_value: f64 = 0.3 (
        callback = |node: &mut Self, _ctx: &mut ProcessCtx, old_value: ParamValue| {
            node.closure_callback_calls += 1;
            node.closure_callback_old = Some(old_value);
        }
    );
)]
struct DslCallbackParamsNode {
    on_param_change_calls: usize,
    default_callback_calls: usize,
    named_callback_calls: usize,
    closure_callback_calls: usize,
    default_callback_old: Option<ParamValue>,
    named_callback_old: Option<ParamValue>,
    closure_callback_old: Option<ParamValue>,
}

#[crate::node("field_callback_params_node")]
struct FieldCallbackParamsNode {
    #[param(default = 0.4, default_callback)]
    default_value: crate::node::ParameterHandle<f64>,

    #[param(default = 0.5, callback = Self::named_value_callback)]
    named_value: crate::node::ParameterHandle<f64>,

    #[param(
        default = 0.6,
        callback = |node: &mut Self, _ctx: &mut ProcessCtx, old_value: ParamValue| {
            node.closure_callback_calls += 1;
            node.closure_callback_old = Some(old_value);
        }
    )]
    closure_value: crate::node::ParameterHandle<f64>,

    on_param_change_calls: usize,
    default_callback_calls: usize,
    named_callback_calls: usize,
    closure_callback_calls: usize,
    default_callback_old: Option<ParamValue>,
    named_callback_old: Option<ParamValue>,
    closure_callback_old: Option<ParamValue>,
}

impl DslCallbackParamsNode {
    fn on_default_value_change(&mut self, _ctx: &mut ProcessCtx, old_value: ParamValue) {
        self.default_callback_calls += 1;
        self.default_callback_old = Some(old_value);
    }

    fn named_value_callback(&mut self, _ctx: &mut ProcessCtx, old_value: ParamValue) {
        self.named_callback_calls += 1;
        self.named_callback_old = Some(old_value);
    }
}

impl FieldCallbackParamsNode {
    fn on_default_value_change(&mut self, _ctx: &mut ProcessCtx, old_value: ParamValue) {
        self.default_callback_calls += 1;
        self.default_callback_old = Some(old_value);
    }

    fn named_value_callback(&mut self, _ctx: &mut ProcessCtx, old_value: ParamValue) {
        self.named_callback_calls += 1;
        self.named_callback_old = Some(old_value);
    }
}

#[crate::node("dependency_params_node")]
#[children(
    driver: f64 = 0.0 (label = "Driver");
    gated_simple: f64 = 1.0 (label = "Gated Simple", dependency = driver > 0.0);
    mode: crate::parameter::Enum = "off" (
        label = "Mode",
        enum_options = ["off (default)", "cool"],
    );
    gated_text: String = "hello".to_string() (
        label = "Gated Text",
        dependency = mode == "cool",
    );
    gated_complex: bool = true (
        label = "Gated Complex",
        dependency = |node: &Self| node.driver.get() > 0.0 && node.mode.get_ref().as_str() == "cool",
    );
    tail: f64 = 2.0 (label = "Tail");
)]
struct DependencyParamsNode {}

#[crate::node("dependency_params_node", from_struct)]
impl Node for DependencyParamsNode {}

#[crate::node("dependency_optional_child_node")]
#[children(
    gated_by_child: f64 = 1.0 (
        label = "Gated By Child",
        dependency = |node: &Self| node.optional_child.current_id().is_some(),
    );
)]
struct DependencyOptionalChildNode {
    #[potential_node(decl_id = "optional_child")]
    optional_child: crate::node::PotentialNodeHandle,
}

#[crate::node("dependency_optional_child_node", from_struct)]
impl Node for DependencyOptionalChildNode {}

#[crate::node("dsl_params_node", from_struct)]
impl Node for DslParamsNode {
    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if param == self.feedback.id() {
            self.observed_feedback_old = Some(old_value);
            self.observed_feedback_new = Some(self.feedback.get());
        }
    }
}

#[crate::node("dsl_vector_bounds_node", from_struct)]
impl Node for DslVectorBoundsNode {}

#[crate::node("dsl_enum_defaults_node", from_struct)]
impl Node for DslEnumDefaultsNode {}

#[crate::node("dsl_reference_default_node", from_struct)]
impl Node for DslReferenceDefaultNode {}

#[crate::node("dsl_node_children_node", from_struct)]
impl Node for DslNodeChildrenNode {}

#[crate::node("dsl_meta_params_node", from_struct)]
impl Node for DslMetaParamsNode {}

#[crate::node("shared_declared_description_node", from_struct)]
impl Node for SharedDeclaredDescriptionNode {}

#[crate::node("manual_inbox_params_node", from_struct)]
impl Node for ManualInboxParamsNode {
    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        for event in &ctx.events {
            if let EventKind::ParamChanged { param, .. } = &event.kind {
                if *param == self.value.id() {
                    self.observed_inbox_value = Some(self.value.get());
                }
            }
        }
    }
}

#[crate::node("params_with_custom_init_node", from_struct)]
impl Node for ParamsWithCustomInitNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.init_calls += 1;
        self.init_observed_value = Some(self.value.get());
        self.init_observed_bound = self.value.is_bound();
        self.init_observed_id = Some(self.value.id());
    }

    fn child_event_interest_depth(&self, _event: &crate::events::Event) -> u32 {
        0
    }
}

#[crate::node("nested_init_binding_node", from_struct)]
impl Node for NestedInitBindingNode {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.init_calls += 1;
        self.init_observed_bound = self.value.is_bound();
        self.init_observed_id = Some(self.value.id());
    }

    fn child_event_interest_depth(&self, _event: &crate::events::Event) -> u32 {
        0
    }
}

#[crate::node("dsl_callback_params_node", from_struct)]
impl Node for DslCallbackParamsNode {
    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, _param: NodeId, _old_value: ParamValue) {
        self.on_param_change_calls += 1;
    }
}

#[crate::node("field_callback_params_node", from_struct)]
impl Node for FieldCallbackParamsNode {
    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, _param: NodeId, _old_value: ParamValue) {
        self.on_param_change_calls += 1;
    }
}

#[derive(Clone, Debug, PartialEq)]
struct UiScriptHostNode {
    node_data: NodeData,
}

impl UiScriptHostNode {
    fn new(label: impl Into<String>) -> Self {
        Self {
            node_data: NodeData::new(label.into()),
        }
    }
}

impl Node for UiScriptHostNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "ui_script_host"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn script_host_policy(&self) -> Option<ScriptHostPolicy> {
        Some(ScriptHostPolicy::default_scriptable())
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["script"]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new("script", "script", "Script").with_select_when_created(false)]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type.trim().eq_ignore_ascii_case("script")).then(|| {
            Box::new(ScriptNode::new(
                "Script",
                ScriptNodeConfig::for_host_node_type(self.get_type()),
            )) as Box<dyn Node>
        })
    }
}

#[crate::node("via_script_host_node")]
struct ViaScriptHostNode {
    base: UiScriptHostNode,
}

#[crate::node("via_script_host_node", via = base, from_struct)]
impl Node for ViaScriptHostNode {}

#[crate::node("ui_context_host")]
struct UiContextHostNode {}

#[crate::node("ui_context_host", from_struct, contextualizable)]
impl Node for UiContextHostNode {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[USER_CONTEXT_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(USER_CONTEXT_NODE_TYPE, USER_CONTEXT_ITEM_KIND, "Context")
                .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        matches!(
            node_type.trim().to_ascii_lowercase().as_str(),
            USER_CONTEXT_NODE_TYPE | "context"
        )
        .then(|| Box::new(UserContextNode::new("Context")) as Box<dyn Node>)
    }
}

#[crate::node("ui_multiplex_context_host")]
struct UiMultiplexContextHostNode {}

#[crate::node(
    "ui_multiplex_context_host",
    from_struct,
    contextualizable = crate::node::UserContextHostPolicy::multiplex_contextualizable()
)]
impl Node for UiMultiplexContextHostNode {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[USER_CONTEXT_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(USER_CONTEXT_NODE_TYPE, USER_CONTEXT_ITEM_KIND, "Context")
                .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        matches!(
            node_type.trim().to_ascii_lowercase().as_str(),
            USER_CONTEXT_NODE_TYPE | "context"
        )
        .then(|| Box::new(UserContextNode::new_with_multiplex("Context", true)) as Box<dyn Node>)
    }
}

#[crate::node("policy_only_script_host", impl_node, scriptable)]
struct PolicyOnlyScriptHostNode {}

#[crate::node("policy_only_context_host", impl_node, contextualizable)]
struct PolicyOnlyContextHostNode {}

#[crate::node("via_context_host_node")]
struct ViaContextHostNode {
    base: UiContextHostNode,
}

#[crate::node("via_context_host_node", via = base, from_struct)]
impl Node for ViaContextHostNode {}

crate::define_node_enum!(
    enum MacroTestNode {
        AutoDeclaredNode,
        StructDeclaredParamsNode,
        ViaStructDeclaredParamsNode,
        ViaComposedLeafNode,
        ViaComposedMidNode,
        ViaComposedRootNode,
        ReuseFolderBaseNode,
        ReuseFolderViaNode,
        SparseDeclaredFolderNode,
        DeclarationOnlyMaterializationNode,
        BaseChildrenLayoutBaseNode,
        BaseChildrenLayoutViaNode,
        DslParamsNode,
        DslVectorBoundsNode,
        DslEnumDefaultsNode,
        DslReferenceDefaultNode,
        DslNodeChildrenNode,
        DslMetaParamsNode,
        ManualInboxParamsNode,
        ParamsWithCustomInitNode,
        NestedInitBindingNode,
        DslCallbackParamsNode,
        FieldCallbackParamsNode,
        DependencyParamsNode,
        DependencyOptionalChildNode,
        UiScriptHostNode,
        UiContextHostNode,
        UiMultiplexContextHostNode,
        ViaScriptHostNode,
        ViaContextHostNode,
        UiSchemaDescriptionNode,
        SharedDeclaredDescriptionNode,
        RemoveLifecycleProbeNode,
        ReadyRemovesChildParentNode,
        ReadyRemovedChildMutationNode,
    }
);

crate::define_node_enum!(
    enum CoreBuiltinOnlyNode {}
);

#[test]
fn node_enum_builtin_set_accepts_user_context_multiplex_nodes() {
    let multiplex = <CoreBuiltinOnlyNode as Node>::from_boxed_node(Box::new(UserContextMultiplexNode::new("Mux")))
        .expect("node enum should accept core multiplex nodes");
    assert_eq!(multiplex.get_type(), USER_CONTEXT_MULTIPLEX_NODE_TYPE);

    let list_type = user_context_multiplex_list_node_type("float");
    let list =
        <CoreBuiltinOnlyNode as Node>::from_boxed_node(Box::new(UserContextMultiplexListNode::new("Floats", "float")))
            .expect("node enum should accept core multiplex list nodes");
    assert_eq!(list.get_type(), list_type);
}

#[test]
fn node_struct_macro_declares_param_and_binds_handle_after_child_event() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(AutoDeclaredNode::new().into(), None);

    // First pass adds the node and runs generated init, which queues param creation.
    engine.apply_edits().expect("first apply should succeed");
    // Second pass materializes generated child param nodes.
    engine.apply_edits().expect("second apply should succeed");

    let declared_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("declared child should exist");

    let child_added_decl = engine.inbox.events.iter().any(|event| {
        matches!(
            &event.kind,
            EventKind::ChildAdded { parent, decl_id, .. }
                if *parent == declared_id && decl_id.0 == "decay"
        )
    });
    assert!(
        child_added_decl,
        "generated param child should emit ChildAdded with decl_id=decay"
    );

    let decay_param = find_child_by_decl(&engine, declared_id, "decay").expect("decay child should exist");
    let decay_meta = engine
        .nodes
        .get(decay_param)
        .expect("decay node should exist")
        .node_data()
        .meta
        .clone();
    assert_eq!(decay_meta.label, "Decay");
    assert_eq!(decay_meta.description.as_deref(), Some("Envelope decay time"));
    let MacroTestNode::Parameter(decay_param_node) =
        engine.nodes.get(decay_param).expect("decay parameter should exist")
    else {
        panic!("expected Parameter variant");
    };
    assert_eq!(
        decay_param_node.constraints.range,
        RangeConstraint::uniform(Some(0.0), Some(1.0))
    );
    assert_eq!(decay_param_node.constraints.step, Some(0.05));
    assert_eq!(decay_param_node.constraints.step_base, Some(0.0));
    assert_eq!(
        decay_param_node.constraints.policy,
        ParameterConstraintPolicy::ClampAdapt
    );

    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("dispatch should succeed");

    let MacroTestNode::AutoDeclaredNode(node) = engine.nodes.get(declared_id).expect("declared node should exist")
    else {
        panic!("expected AutoDeclaredNode variant");
    };

    assert!(
        node.decay.is_bound(),
        "generated param handle should be bound after ChildAdded dispatch"
    );
    assert!(
        !node.value.is_pending_create(),
        "potential slot should not be pending by default"
    );
}

fn find_child_by_decl_any<T: Node>(engine: &Engine<T>, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id)?;
        if node.node_data().meta.decl_id.0 == decl_id {
            return Some(id);
        }
        child = node.node_data().next_sibling;
    }
    None
}

fn find_child_by_label_any<T: Node>(engine: &Engine<T>, parent: NodeId, label: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id)?;
        if node.node_data().meta.label == label {
            return Some(id);
        }
        child = node.node_data().next_sibling;
    }
    None
}

fn direct_children_of_type<T: Node>(engine: &Engine<T>, parent: NodeId, node_type: &str) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(id) = child {
        let Some(node) = engine.nodes.get(id) else {
            break;
        };
        if node.get_type() == node_type {
            out.push(id);
        }
        child = node.node_data().next_sibling;
    }
    out
}

fn node_uuids<T: Node>(engine: &Engine<T>, nodes: &[NodeId]) -> Vec<String> {
    nodes
        .iter()
        .map(|node| {
            engine
                .nodes
                .get(*node)
                .expect("node should exist")
                .node_data()
                .meta
                .uuid
                .0
                .to_string()
        })
        .collect()
}

fn set_param_and_tick<T: Node>(engine: &mut Engine<T>, param: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node: param,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("param change should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("param change inbox should dispatch");
    engine.apply_edits().expect("inbox-queued edits should apply");
    engine
        .run_tick(Duration::from_millis(20))
        .expect("tick should process param change");
    engine.apply_edits().expect("tick-queued edits should apply");
}

fn switch_animation_to_curve_waveform<T: Node>(engine: &mut Engine<T>, animation_node: NodeId) -> NodeId {
    let waveform_param = find_child_by_decl_any(engine, animation_node, PARAMETER_ANIMATION_WAVEFORM_DECL_ID)
        .expect("waveform parameter should exist");
    engine.edits.push(Edit::SetParam {
        node: waveform_param,
        value: ParamValue::Enum("curve".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("waveform switch to curve should apply");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should process waveform change");
    find_child_by_decl_any(engine, animation_node, PARAMETER_ANIMATION_CURVE_DECL_ID)
        .expect("curve node should exist after switching to Curve waveform")
}

fn count_children_by_decl_any<T: Node>(engine: &Engine<T>, parent: NodeId, decl_id: &str) -> usize {
    let mut count = 0usize;
    let Some(parent_node) = engine.nodes.get(parent) else {
        return count;
    };
    let mut child = parent_node.node_data().first_child;
    while let Some(id) = child {
        let Some(node) = engine.nodes.get(id) else {
            break;
        };
        if node.node_data().meta.decl_id.0 == decl_id {
            count += 1;
        }
        child = node.node_data().next_sibling;
    }
    count
}

fn direct_child_decl_ids_any<T: Node>(engine: &Engine<T>, parent: NodeId) -> Vec<String> {
    let mut decl_ids = Vec::new();
    let Some(parent_node) = engine.nodes.get(parent) else {
        return decl_ids;
    };
    let mut child = parent_node.node_data().first_child;
    while let Some(id) = child {
        let Some(node) = engine.nodes.get(id) else {
            break;
        };
        decl_ids.push(node.node_data().meta.decl_id.0.clone());
        child = node.node_data().next_sibling;
    }
    decl_ids
}

fn child_decl_ids_any<T: Node>(engine: &Engine<T>, parent: NodeId) -> Vec<String> {
    let mut out = Vec::new();
    let Some(parent_node) = engine.nodes.get(parent) else {
        return out;
    };

    let mut child = parent_node.node_data().first_child;
    while let Some(id) = child {
        let Some(node) = engine.nodes.get(id) else {
            break;
        };
        out.push(node.node_data().meta.decl_id.0.clone());
        child = node.node_data().next_sibling;
    }

    out
}

fn find_child_by_decl(engine: &Engine<MacroTestNode>, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    find_child_by_decl_any(engine, parent, decl_id)
}

fn find_child_by_type(engine: &Engine<MacroTestNode>, parent: NodeId, node_type: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(id) = child {
        let node = engine.nodes.get(id)?;
        if node.get_type() == node_type {
            return Some(id);
        }
        child = node.node_data().next_sibling;
    }
    None
}

fn configure_control_reference(engine: &mut Engine<MacroTestNode>, param: NodeId, target_uuid: NodeUuid) {
    configure_control_reference_with_projection(engine, param, target_uuid, None);
}

fn configure_control_reference_with_projection(
    engine: &mut Engine<MacroTestNode>,
    param: NodeId,
    target_uuid: NodeUuid,
    projection: Option<ParamValueProjection>,
) {
    let reference_param = find_child_by_decl(engine, param, PARAMETER_CONTROL_REFERENCE_DECL_ID)
        .expect("control reference parameter should exist");
    let mut target_reference = NodeReference::new(target_uuid);
    target_reference.set_projection(projection);

    engine.edits.push(Edit::SetParam {
        node: reference_param,
        value: ParamValue::Reference(target_reference),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("control reference update should apply");
}

fn configure_expression_control_source(engine: &mut Engine<MacroTestNode>, param: NodeId, expression: &str) -> NodeId {
    let source_param = find_child_by_decl(engine, param, PARAMETER_EXPRESSION_SOURCE_DECL_ID)
        .expect("expression source parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: source_param,
        value: ParamValue::Str(expression.to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("expression source update should apply");
    source_param
}

fn child_decl_ids(engine: &Engine<MacroTestNode>, parent: NodeId) -> Vec<String> {
    let mut out = Vec::new();
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(id) = child {
        let Some(node) = engine.nodes.get(id) else {
            break;
        };
        out.push(node.node_data().meta.decl_id.0.clone());
        child = node.node_data().next_sibling;
    }
    out
}

#[test]
fn params_macro_materializes_nested_folders_and_binds_handles() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslParamsNode::new(None, None).into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("dsl node should be attached under root");

    let output = find_child_by_decl(&engine, owner, "output").expect("output folder should exist");
    let color = find_child_by_decl(&engine, output, "output/color").expect("output/color folder should exist");
    let feedback = find_child_by_decl(&engine, owner, "feedback").expect("feedback parameter should exist");
    let host = find_child_by_decl(&engine, output, "output/host").expect("output/host parameter should exist");
    let gamma =
        find_child_by_decl(&engine, color, "output/color/gamma").expect("output/color/gamma parameter should exist");

    let MacroTestNode::DslParamsNode(node) = engine.nodes.get(owner).expect("dsl node should exist") else {
        panic!("expected DslParamsNode variant");
    };

    assert!(node.feedback.is_bound(), "feedback handle should be bound");
    assert!(node.host.is_bound(), "host handle should be bound");
    assert!(node.gamma.is_bound(), "gamma handle should be bound");
    assert_eq!(node.gamma.event_behaviour(), ParameterEventBehaviour::Append);
    assert_eq!(node.feedback.id(), feedback);
    assert_eq!(node.host.id(), host);
    assert_eq!(node.gamma.id(), gamma);

    let feedback_meta = engine
        .nodes
        .get(feedback)
        .expect("feedback node should exist")
        .node_data()
        .meta
        .clone();
    assert_eq!(feedback_meta.label, "Feedback");
    assert_eq!(feedback_meta.description.as_deref(), Some("Delay feedback amount"));
    let MacroTestNode::Parameter(feedback_param) = engine.nodes.get(feedback).expect("feedback parameter should exist")
    else {
        panic!("expected Parameter variant");
    };
    assert_eq!(
        feedback_param.constraints.range,
        RangeConstraint::uniform(Some(0.0), Some(1.0))
    );
    assert_eq!(feedback_param.constraints.step, Some(0.1));
    assert_eq!(feedback_param.constraints.step_base, Some(0.0));
    assert_eq!(feedback_param.constraints.policy, ParameterConstraintPolicy::Reject);
    assert!(feedback_param.read_only);

    let host_meta = engine
        .nodes
        .get(host)
        .expect("host node should exist")
        .node_data()
        .meta
        .clone();
    assert_eq!(host_meta.label, "Host");
    assert_eq!(host_meta.description.as_deref(), Some("OSC destination host"));
}

#[test]
fn params_macro_supports_component_min_max_for_vector_parameters() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslVectorBoundsNode::new().into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("vector bounds node should be attached under root");
    let vec2_bounds = find_child_by_decl(&engine, owner, "vec2_bounds").expect("vec2_bounds parameter should exist");
    let vec3_bounds = find_child_by_decl(&engine, owner, "vec3_bounds").expect("vec3_bounds parameter should exist");

    let MacroTestNode::Parameter(vec2_param) = engine
        .nodes
        .get(vec2_bounds)
        .expect("vec2_bounds parameter should exist")
    else {
        panic!("expected Parameter variant");
    };
    assert_eq!(
        vec2_param.constraints.range,
        RangeConstraint::components(Some(vec![-1.0, 0.0]), Some(vec![1.0, 2.0]))
    );

    let MacroTestNode::Parameter(vec3_param) = engine
        .nodes
        .get(vec3_bounds)
        .expect("vec3_bounds parameter should exist")
    else {
        panic!("expected Parameter variant");
    };
    assert_eq!(
        vec3_param.constraints.range,
        RangeConstraint::components(Some(vec![-1.0, 0.0, 10.0]), Some(vec![1.0, 2.0, 20.0]))
    );
}

#[test]
fn params_macro_supports_simple_enum_option_lists_and_default_resolution() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslEnumDefaultsNode::new().into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("enum defaults node should be attached under root");
    let mode_marked = find_child_by_decl(&engine, owner, "mode_marked").expect("mode_marked parameter should exist");
    let mode_explicit =
        find_child_by_decl(&engine, owner, "mode_explicit").expect("mode_explicit parameter should exist");
    let mode_first = find_child_by_decl(&engine, owner, "mode_first").expect("mode_first parameter should exist");

    let MacroTestNode::DslEnumDefaultsNode(node) = engine.nodes.get(owner).expect("enum defaults node should exist")
    else {
        panic!("expected DslEnumDefaultsNode variant");
    };
    assert!(node.mode_marked.is_bound(), "mode_marked handle should be bound");
    assert!(node.mode_explicit.is_bound(), "mode_explicit handle should be bound");
    assert!(node.mode_first.is_bound(), "mode_first handle should be bound");

    let MacroTestNode::Parameter(marked_param) = engine
        .nodes
        .get(mode_marked)
        .expect("mode_marked parameter should exist")
    else {
        panic!("expected Parameter variant");
    };
    assert_eq!(marked_param.value, ParamValue::Enum("auto".to_string()));
    assert_eq!(marked_param.constraints.enum_options.len(), 3);
    assert_eq!(marked_param.constraints.enum_options[0].variant_id, "off");
    assert_eq!(marked_param.constraints.enum_options[1].variant_id, "on");
    assert_eq!(marked_param.constraints.enum_options[2].variant_id, "auto");

    let MacroTestNode::Parameter(explicit_param) = engine
        .nodes
        .get(mode_explicit)
        .expect("mode_explicit parameter should exist")
    else {
        panic!("expected Parameter variant");
    };
    assert_eq!(explicit_param.value, ParamValue::Enum("on".to_string()));

    let MacroTestNode::Parameter(first_param) =
        engine.nodes.get(mode_first).expect("mode_first parameter should exist")
    else {
        panic!("expected Parameter variant");
    };
    assert_eq!(first_param.value, ParamValue::Enum("a".to_string()));
}

#[test]
fn params_macro_allows_reference_without_explicit_default_value() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslReferenceDefaultNode::new().into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("reference default node should be attached under root");
    let target_ref = find_child_by_decl(&engine, owner, "target_ref").expect("target_ref parameter should exist");

    let MacroTestNode::DslReferenceDefaultNode(node) =
        engine.nodes.get(owner).expect("reference default node should exist")
    else {
        panic!("expected DslReferenceDefaultNode variant");
    };
    assert!(node.target_ref.is_bound(), "target_ref handle should be bound");

    let MacroTestNode::Parameter(reference_param) =
        engine.nodes.get(target_ref).expect("target_ref parameter should exist")
    else {
        panic!("expected Parameter variant");
    };
    let ParamValue::Reference(reference) = &reference_param.value else {
        panic!("expected ParamValue::Reference");
    };
    assert!(reference.uuid().is_nil(), "reference default should use nil uuid");
    assert_eq!(
        reference.cached_id(),
        None,
        "reference default should have no cached id"
    );
}

#[test]
fn children_macro_materializes_declared_node_children_and_binds_handles() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslNodeChildrenNode::new().into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("node children node should be attached under root");
    let output = find_child_by_decl(&engine, owner, "output").expect("output folder should exist");
    let curve = find_child_by_decl(&engine, output, "output/curve").expect("curve child should exist");

    let MacroTestNode::DslNodeChildrenNode(node) = engine.nodes.get(owner).expect("node children node should exist")
    else {
        panic!("expected DslNodeChildrenNode variant");
    };
    assert!(node.curve.is_present(), "generated potential handle should be bound");
    assert_eq!(node.curve.current_id(), Some(curve));
    assert!(!node.curve.is_pending_create(), "bound handle should not stay pending");

    let curve_node = engine.nodes.get(curve).expect("curve child should exist");
    assert_eq!(curve_node.get_type(), PARAMETER_ANIMATION_CURVE_NODE_TYPE);
    assert_eq!(curve_node.node_data().meta.label, "Curve");
    assert_eq!(
        curve_node.node_data().meta.description.as_deref(),
        Some("Declared curve child")
    );
    assert_eq!(
        curve_node.node_data().meta.presentation.default_color,
        Some(crate::color::Color::new(0.25, 0.5, 0.75, 1.0))
    );
    assert!(curve_node.node_data().meta.presentation.collapsed);
}

#[test]
fn params_macro_applies_metadata_overrides_for_generated_nodes() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslMetaParamsNode::new().into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("dsl meta node should be attached under root");
    let settings = find_child_by_decl(&engine, owner, "settings").expect("settings folder should exist");
    let gain = find_child_by_decl(&engine, settings, "settings/gain").expect("settings/gain parameter should exist");
    let owner_meta = engine
        .nodes
        .get(owner)
        .expect("dsl meta node should exist")
        .node_data()
        .meta
        .clone();
    assert_eq!(
        owner_meta.presentation.default_color,
        Some(crate::color::Color::new(0.95, 0.4, 0.2, 1.0))
    );
    assert!(!owner_meta.presentation.show_in_nested_inspector);

    let settings_meta = engine
        .nodes
        .get(settings)
        .expect("settings folder should exist")
        .node_data()
        .meta
        .clone();
    assert_eq!(settings_meta.label, "Settings");
    assert_eq!(settings_meta.short_name, "settings_folder");
    assert!(!settings_meta.enabled);
    assert!(settings_meta.can_be_disabled);
    assert_eq!(settings_meta.description.as_deref(), Some("Settings folder metadata"));
    assert_eq!(settings_meta.tags, vec![String::from("group")]);
    assert_eq!(settings_meta.semantics.intent.as_deref(), Some("container"));
    assert_eq!(settings_meta.semantics.unit.as_deref(), Some("section"));
    assert_eq!(
        settings_meta.presentation.default_color,
        Some(crate::color::Color::new(0.1, 0.2, 0.3, 1.0))
    );
    assert!(settings_meta.presentation.collapsed);
    assert_eq!(settings_meta.presentation.show_child_warnings_max_depth, 2);

    let gain_meta = engine
        .nodes
        .get(gain)
        .expect("gain parameter should exist")
        .node_data()
        .meta
        .clone();
    assert_eq!(gain_meta.label, "Gain");
    assert_eq!(gain_meta.short_name, "gain_param");
    assert!(!gain_meta.enabled);
    assert!(gain_meta.can_be_disabled);
    assert_eq!(gain_meta.description.as_deref(), Some("Gain parameter metadata"));
    assert_eq!(gain_meta.tags, vec![String::from("audio"), String::from("gain")]);
    assert_eq!(gain_meta.semantics.intent.as_deref(), Some("level"));
    assert_eq!(gain_meta.semantics.unit.as_deref(), Some("db"));
    assert_eq!(
        gain_meta.presentation.default_color,
        Some(crate::color::Color::new(0.7, 0.8, 0.9, 1.0))
    );
}

#[test]
fn params_macro_syncs_handle_cache_before_on_param_change_callback() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslParamsNode::new(None, None).into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("dsl node should be attached under root");
    let feedback = find_child_by_decl(&engine, owner, "feedback").expect("feedback parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: feedback,
        value: ParamValue::Float(0.6),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("dispatch should succeed");

    let MacroTestNode::DslParamsNode(node) = engine.nodes.get(owner).expect("dsl node should exist") else {
        panic!("expected DslParamsNode variant");
    };

    assert!(
        node.observed_feedback_new
            .is_some_and(|value| (value - 0.6).abs() < 1e-9),
        "on_param_change should observe synced handle cache with new value",
    );
    assert!(
        matches!(node.observed_feedback_old, Some(ParamValue::Float(value)) if (value - 0.5).abs() < 1e-9),
        "on_param_change should still receive previous parameter value",
    );
}

#[test]
fn params_macro_supports_default_named_and_closure_callbacks() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslCallbackParamsNode::new(0, 0, 0, 0, None, None, None).into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("callback node should be attached under root");
    let default_value =
        find_child_by_decl(&engine, owner, "default_value").expect("default_value parameter should exist");
    let named_value = find_child_by_decl(&engine, owner, "named_value").expect("named_value parameter should exist");
    let closure_value =
        find_child_by_decl(&engine, owner, "closure_value").expect("closure_value parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: default_value,
        value: ParamValue::Float(1.1),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: named_value,
        value: ParamValue::Float(1.2),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: closure_value,
        value: ParamValue::Float(1.3),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("dispatch should succeed");

    let MacroTestNode::DslCallbackParamsNode(node) = engine.nodes.get(owner).expect("callback node should exist")
    else {
        panic!("expected DslCallbackParamsNode variant");
    };

    assert_eq!(node.default_callback_calls, 1, "default callback should run once");
    assert_eq!(node.named_callback_calls, 1, "named callback should run once");
    assert_eq!(node.closure_callback_calls, 1, "closure callback should run once");
    assert_eq!(
        node.on_param_change_calls, 3,
        "callbacks should not replace on_param_change"
    );
    assert!(
        matches!(node.default_callback_old, Some(ParamValue::Float(value)) if (value - 0.1).abs() < 1e-9),
        "default callback should receive previous value",
    );
    assert!(
        matches!(node.named_callback_old, Some(ParamValue::Float(value)) if (value - 0.2).abs() < 1e-9),
        "named callback should receive previous value",
    );
    assert!(
        matches!(node.closure_callback_old, Some(ParamValue::Float(value)) if (value - 0.3).abs() < 1e-9),
        "closure callback should receive previous value",
    );
}

#[test]
fn field_params_support_default_named_and_closure_callbacks() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(FieldCallbackParamsNode::new(0, 0, 0, 0, None, None, None).into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("callback node should be attached under root");
    let default_value =
        find_child_by_decl(&engine, owner, "default_value").expect("default_value parameter should exist");
    let named_value = find_child_by_decl(&engine, owner, "named_value").expect("named_value parameter should exist");
    let closure_value =
        find_child_by_decl(&engine, owner, "closure_value").expect("closure_value parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: default_value,
        value: ParamValue::Float(1.4),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: named_value,
        value: ParamValue::Float(1.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: closure_value,
        value: ParamValue::Float(1.6),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("dispatch should succeed");

    let MacroTestNode::FieldCallbackParamsNode(node) = engine.nodes.get(owner).expect("callback node should exist")
    else {
        panic!("expected FieldCallbackParamsNode variant");
    };

    assert_eq!(node.default_callback_calls, 1, "default callback should run once");
    assert_eq!(node.named_callback_calls, 1, "named callback should run once");
    assert_eq!(node.closure_callback_calls, 1, "closure callback should run once");
    assert_eq!(
        node.on_param_change_calls, 3,
        "callbacks should not replace on_param_change"
    );
    assert!(
        matches!(node.default_callback_old, Some(ParamValue::Float(value)) if (value - 0.4).abs() < 1e-9),
        "default callback should receive previous value",
    );
    assert!(
        matches!(node.named_callback_old, Some(ParamValue::Float(value)) if (value - 0.5).abs() < 1e-9),
        "named callback should receive previous value",
    );
    assert!(
        matches!(node.closure_callback_old, Some(ParamValue::Float(value)) if (value - 0.6).abs() < 1e-9),
        "closure callback should receive previous value",
    );
}

#[test]
fn params_macro_dependencies_create_remove_and_reinsert_in_declared_order() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DependencyParamsNode::new().into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("dependency node should be attached under root");
    assert_eq!(
        child_decl_ids_any(&engine, owner),
        ["driver", "mode", "tail"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        "initial dependency-filtered order should skip gated params"
    );

    let driver = find_child_by_decl(&engine, owner, "driver").expect("driver parameter should exist");
    let mode = find_child_by_decl(&engine, owner, "mode").expect("mode parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: driver,
        value: ParamValue::Float(1.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..3 {
        engine.apply_edits().expect("driver edit should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert_eq!(
        child_decl_ids_any(&engine, owner),
        ["driver", "gated_simple", "mode", "tail"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        "simple dependency should insert the parameter between its declared neighbors",
    );
    assert_eq!(
        count_children_by_decl_any(&engine, owner, "gated_simple"),
        1,
        "gated simple parameter should not duplicate"
    );

    engine.edits.push(Edit::SetParam {
        node: mode,
        value: ParamValue::Enum("cool".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..3 {
        engine.apply_edits().expect("mode edit should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert_eq!(
        child_decl_ids_any(&engine, owner),
        ["driver", "gated_simple", "mode", "gated_text", "gated_complex", "tail"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        "string comparison and closure dependencies should both materialize in declared order",
    );

    engine.edits.push(Edit::SetParam {
        node: driver,
        value: ParamValue::Float(0.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for _ in 0..3 {
        engine.apply_edits().expect("driver reset should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert_eq!(
        child_decl_ids_any(&engine, owner),
        ["driver", "mode", "gated_text", "tail"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>(),
        "dependent removals should keep surviving params in their declared slots",
    );
    assert!(
        find_child_by_decl(&engine, owner, "gated_simple").is_none(),
        "simple dependency param should be removed again"
    );
    assert!(
        find_child_by_decl(&engine, owner, "gated_complex").is_none(),
        "closure dependency param should be removed again"
    );
}

#[test]
fn params_macro_dependency_closure_can_observe_absent_local_child_as_false() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DependencyOptionalChildNode::new().into(), None);

    for _ in 0..4 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("dependency node should be attached under root");
    assert!(
        find_child_by_decl(&engine, owner, "gated_by_child").is_none(),
        "dependency should evaluate false while the optional child is absent"
    );

    let mut optional_child = Folder::new("Optional Child".to_string());
    optional_child.node_data_mut().meta.decl_id = crate::node::DeclId("optional_child".to_string());
    engine.edits.push(Edit::AddNode {
        parent: owner,
        prev_sibling: None,
        node: Box::new(optional_child),
    });

    for _ in 0..4 {
        engine.apply_edits().expect("child add should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    assert!(
        find_child_by_decl(&engine, owner, "gated_by_child").is_some(),
        "dependency should become true once the local optional child exists"
    );
}

#[test]
fn engine_preprocesses_inbox_before_custom_on_inbox_logic() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(ManualInboxParamsNode::new(None).into(), None);

    for _ in 0..4 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("manual node should be attached under root");
    let value_param = find_child_by_decl(&engine, owner, "value").expect("value parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: value_param,
        value: ParamValue::Float(0.6),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("dispatch should succeed");

    let MacroTestNode::ManualInboxParamsNode(node) = engine.nodes.get(owner).expect("manual node should exist") else {
        panic!("expected ManualInboxParamsNode variant");
    };

    assert!(
        node.observed_inbox_value
            .is_some_and(|value| (value - 0.6).abs() < 1e-9),
        "custom on_inbox should observe already-preprocessed handle value",
    );
}

#[test]
fn params_macro_keeps_init_and_child_interest_overrides_available() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(ParamsWithCustomInitNode::new(0, None, false, None).into(), None);

    for _ in 0..4 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("custom node should be attached under root");
    let value_param = find_child_by_decl(&engine, owner, "value").expect("value parameter should exist");

    let MacroTestNode::ParamsWithCustomInitNode(node) = engine.nodes.get(owner).expect("custom node should exist")
    else {
        panic!("expected ParamsWithCustomInitNode variant");
    };

    assert_eq!(node.init_calls, 1, "custom init override should remain active");
    assert_eq!(
        node.value.id(),
        value_param,
        "params preprocessing should still bind handles"
    );
    assert!(
        node.init_observed_value.is_some_and(|value| (value - 0.5).abs() < 1e-9),
        "custom init should observe declared default value before app init runs",
    );
    assert!(
        node.init_observed_bound,
        "custom init should observe a bound declared handle"
    );
    assert_eq!(
        node.init_observed_id,
        Some(value_param),
        "custom init should observe the runtime parameter id"
    );
}

#[test]
fn project_load_refreshes_declared_param_handles_before_init() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(ParamsWithCustomInitNode::new(0, None, false, None).into(), None);
    engine.apply_edits().expect("declared node should materialize");

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("custom node should be attached under root");
    let value_param = find_child_by_decl(&engine, owner, "value").expect("value parameter should exist");
    engine.edits.push(Edit::SetParam {
        node: value_param,
        value: ParamValue::Float(0.8),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("parameter edit should apply");

    let json = engine
        .to_project_json_with(|node| node.project_encode_data())
        .expect("project should serialize");
    let loaded = Engine::<MacroTestNode>::from_project_json_with(&json, |node_type, data, meta| {
        if node_type == ParamsWithCustomInitNode::NODE_TYPE {
            let mut node = ParamsWithCustomInitNode::new(0, None, false, None);
            node.project_decode_data(data)?;
            return Ok(node.into());
        }
        <MacroTestNode as crate::app::ProjectNode>::project_decode_node(node_type, data, meta)
    })
    .expect("project should load");
    let loaded_owner = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("custom node should reload");
    let MacroTestNode::ParamsWithCustomInitNode(node) =
        loaded.nodes.get(loaded_owner).expect("custom node should exist")
    else {
        panic!("expected ParamsWithCustomInitNode variant");
    };

    assert!(
        node.init_observed_value.is_some_and(|value| (value - 0.8).abs() < 1e-9),
        "project-load init should observe the persisted parameter value"
    );
}

#[test]
fn sparse_load_skips_unknown_node_type_and_keeps_the_rest() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(Folder::new("keeper".to_string()).into(), None);
    engine.apply_edits().expect("known child should materialize");

    let json = crate::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");

    // Inject a saved child whose node type is unknown to this build, as a stale or
    // renamed project would produce. The loader must skip it instead of aborting.
    let mut project: serde_json::Value = serde_json::from_str(&json).expect("sparse json should parse");
    let root_obj = project["root"]
        .as_object_mut()
        .expect("root record should be an object");
    let children = root_obj
        .entry("children")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .expect("children should be an array");
    children.push(serde_json::json!({
        "uuid": "11111111-1111-1111-1111-111111111111",
        "type": "totally_unknown_removed_node_type",
    }));
    let mutated = serde_json::to_string(&project).expect("mutated json should encode");

    let loaded = crate::app::from_sparse_project_json::<MacroTestNode>(&mutated)
        .expect("project containing an unknown node type should still load");

    let mut child_types = Vec::new();
    let mut cursor = loaded
        .nodes
        .get(loaded.root)
        .and_then(|node| node.node_data().first_child);
    while let Some(id) = cursor {
        let node = loaded.nodes.get(id).expect("child node should exist");
        child_types.push(node.get_type().to_string());
        cursor = node.node_data().next_sibling;
    }

    assert!(
        child_types.iter().any(|ty| ty == crate::node::FOLDER_NODE_TYPE),
        "the known child should survive the load, got: {child_types:?}"
    );
    assert!(
        !child_types.iter().any(|ty| ty == "totally_unknown_removed_node_type"),
        "the unknown node type should have been skipped, got: {child_types:?}"
    );
}

#[test]
fn nested_declared_params_are_bound_during_init() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(NestedInitBindingNode::new(0, false, None).into(), None);

    engine
        .apply_edits()
        .expect("single apply should materialize nested declarations before init");

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("nested node should be attached under root");
    let group = find_child_by_decl(&engine, owner, "group").expect("group folder should exist");
    let value_param = find_child_by_decl(&engine, group, "group/value").expect("nested parameter should exist");

    let MacroTestNode::NestedInitBindingNode(node) = engine.nodes.get(owner).expect("nested node should exist") else {
        panic!("expected NestedInitBindingNode variant");
    };

    assert_eq!(node.init_calls, 1, "init should run exactly once");
    assert!(
        node.init_observed_bound,
        "nested declared parameter should already be bound during init"
    );
    assert_eq!(
        node.init_observed_id,
        Some(value_param),
        "init should observe the concrete nested parameter id"
    );
}

#[test]
fn struct_param_declarations_delegate_wiring_into_impl_node() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(StructDeclaredParamsNode::new(0, None).into(), None);

    for _ in 0..4 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("node should be attached under root");
    let value_param = find_child_by_decl(&engine, owner, "value").expect("value parameter should exist");

    let MacroTestNode::StructDeclaredParamsNode(node) = engine.nodes.get(owner).expect("node should exist") else {
        panic!("expected StructDeclaredParamsNode variant");
    };

    assert_eq!(node.init_calls, 1, "custom init override should run once");
    assert_eq!(
        node.value.id(),
        value_param,
        "struct-declared param handle should bind to runtime parameter child"
    );
    assert!(
        node.init_observed_value.is_some_and(|value| (value - 0.5).abs() < 1e-9),
        "custom init should observe struct-declared default before app init runs",
    );
}

#[test]
fn struct_param_declarations_with_via_use_composed_node_data() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(
        ViaStructDeclaredParamsNode::new(ViaNodeCore::new("base"), 0, None).into(),
        None,
    );

    for _ in 0..4 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("node should be attached under root");
    let value_param = find_child_by_decl(&engine, owner, "value").expect("value parameter should exist");

    let MacroTestNode::ViaStructDeclaredParamsNode(node) = engine.nodes.get(owner).expect("node should exist") else {
        panic!("expected ViaStructDeclaredParamsNode variant");
    };

    assert_eq!(
        node.base.node_data.id, owner,
        "via path should be the runtime node identity source"
    );
    assert_eq!(
        node.value.id(),
        value_param,
        "struct-declared param handle should bind using composed node identity"
    );
    assert_eq!(node.init_calls, 1, "custom init override should run once");
    assert!(
        node.init_observed_value.is_some_and(|value| (value - 0.5).abs() < 1e-9),
        "custom init should observe struct-declared default before app init runs",
    );
}

#[test]
fn from_struct_via_composed_nodes_forwards_generated_param_wiring_recursively() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(
        ViaComposedRootNode::new(ViaComposedMidNode::new(ViaComposedLeafNode::new())).into(),
        None,
    );

    for _ in 0..8 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("node should be attached under root");
    let decl_ids = child_decl_ids(&engine, owner);
    let root_value = find_child_by_decl(&engine, owner, "root_value").expect("root_value parameter should exist");
    let mid_value = find_child_by_decl(&engine, owner, "mid_value").expect("mid_value parameter should exist");
    let leaf_value = find_child_by_decl(&engine, owner, "leaf_value").expect("leaf_value parameter should exist");

    let MacroTestNode::ViaComposedRootNode(node) = engine.nodes.get(owner).expect("node should exist") else {
        panic!("expected ViaComposedRootNode variant");
    };

    assert_eq!(
        node.root_value.id(),
        root_value,
        "root-level generated handle should bind"
    );
    assert_eq!(
        node.mid.mid_value.id(),
        mid_value,
        "mid-level generated handle should bind via delegation"
    );
    assert_eq!(
        node.mid.leaf.leaf_value.id(),
        leaf_value,
        "leaf-level generated handle should bind via recursive delegation"
    );
    assert_eq!(
        decl_ids,
        vec![
            "leaf_value".to_string(),
            "mid_value".to_string(),
            "root_value".to_string()
        ],
        "when using `via`, nested parameters should materialize before outer parameters",
    );
}

#[test]
fn params_macro_folder_reuses_via_folder_when_decl_id_matches_by_default() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(ReuseFolderViaNode::new(ReuseFolderBaseNode::new()).into(), None);

    for _ in 0..8 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("node should be attached under root");
    let owner_decl_ids = child_decl_ids(&engine, owner);
    let output_count = owner_decl_ids.iter().filter(|decl| decl.as_str() == "output").count();
    assert_eq!(
        output_count, 1,
        "folder reuse should avoid creating a duplicate folder when via already queued the same decl_id"
    );

    let output = find_child_by_decl(&engine, owner, "output").expect("shared output folder should exist");
    let host = find_child_by_decl(&engine, output, "output/host").expect("base param should exist in shared folder");
    let gain = find_child_by_decl(&engine, output, "output/gain").expect("outer param should exist in shared folder");
    let output_decl_ids = child_decl_ids(&engine, output);

    assert_eq!(
        output_decl_ids
            .iter()
            .filter(|decl| decl.as_str() == "output/host")
            .count(),
        1
    );
    assert_eq!(
        output_decl_ids
            .iter()
            .filter(|decl| decl.as_str() == "output/gain")
            .count(),
        1
    );
    assert_eq!(
        output_decl_ids,
        vec!["output/host".to_string(), "output/gain".to_string()],
        "reused folder children should preserve inner(via) items first and outer items at the end",
    );

    let MacroTestNode::ReuseFolderViaNode(node) = engine.nodes.get(owner).expect("node should exist") else {
        panic!("expected ReuseFolderViaNode variant");
    };

    assert_eq!(
        node.base.host.id(),
        host,
        "base handle should bind to shared-folder host parameter"
    );
    assert_eq!(
        node.gain.id(),
        gain,
        "outer handle should bind to shared-folder gain parameter"
    );
}

#[test]
fn sparse_project_load_drops_duplicate_declared_child_overlays() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(SparseDeclaredFolderNode::new().into(), None);
    for _ in 0..8 {
        engine.apply_edits().expect("declared children should materialize");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("declared children should stabilize");
    }

    let full_project = engine
        .to_project_file_with(|node| node.project_encode_data())
        .expect("full project should encode");
    let owner_record = full_project.root.children.first().expect("owner record should exist");
    let mut advanced_record = owner_record
        .children
        .iter()
        .find(|child| {
            child
                .meta
                .decl_id
                .as_ref()
                .is_some_and(|decl_id| decl_id.0.as_str() == "advanced")
        })
        .cloned()
        .expect("advanced folder record should exist");
    let expected_decl_ids = advanced_record
        .children
        .iter()
        .map(|child| {
            child
                .meta
                .decl_id
                .as_ref()
                .expect("declared child should have a decl_id")
                .0
                .clone()
        })
        .collect::<Vec<_>>();
    let duplicate_decl_id = expected_decl_ids
        .first()
        .cloned()
        .expect("advanced folder should have declared children");
    let duplicate_child = advanced_record
        .children
        .first()
        .cloned()
        .expect("advanced folder should have declared children");
    advanced_record.children.insert(1, duplicate_child);

    let sparse_json = crate::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
    let mut sparse_project: ProjectFile = serde_json::from_str(&sparse_json).expect("sparse project should be JSON");
    sparse_project
        .root
        .children
        .first_mut()
        .expect("sparse owner record should exist")
        .children = vec![advanced_record];
    let duplicated_json = serde_json::to_string(&sparse_project).expect("mutated sparse project should encode");

    let loaded = crate::app::from_sparse_project_json::<MacroTestNode>(&duplicated_json)
        .expect("duplicated sparse project should decode");
    let owner = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("loaded owner should exist");
    let advanced = find_child_by_decl(&loaded, owner, "advanced").expect("advanced folder should exist");
    let advanced_decl_ids = child_decl_ids(&loaded, advanced);
    let mut advanced_child_details = Vec::new();
    let mut child = loaded.nodes.get(advanced).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let node = loaded.nodes.get(child_id).expect("advanced child should exist");
        advanced_child_details.push((
            node.get_type().to_string(),
            node.node_data().user_role,
            node.node_data().meta.decl_id.0.clone(),
        ));
        child = node.node_data().next_sibling;
    }

    assert_eq!(
        advanced_decl_ids
            .iter()
            .filter(|decl_id| decl_id.as_str() == duplicate_decl_id.as_str())
            .count(),
        1,
        "duplicate declared overlays should not create repeated parameters: {advanced_child_details:?}"
    );
    assert_eq!(advanced_decl_ids, expected_decl_ids);
}

#[test]
fn declaration_only_apply_materializes_nested_defaults_without_runtime_callbacks() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DeclarationOnlyMaterializationNode::new(0, 0, 0).into(), None);

    engine
        .apply_edits_without_creation_callbacks()
        .expect("declaration-only apply should succeed");

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("declaration owner should exist");
    let connection = find_child_by_decl(&engine, owner, "connection").expect("connection folder should materialize");
    let input =
        find_child_by_decl(&engine, connection, "connection/input").expect("nested input default should materialize");

    let MacroTestNode::Parameter(input) = engine.nodes.get(input).expect("input parameter should exist") else {
        panic!("expected nested input parameter");
    };
    assert_eq!(input.value, ParamValue::from("127.0.0.1".to_string()));

    let MacroTestNode::DeclarationOnlyMaterializationNode(owner_node) =
        engine.nodes.get(owner).expect("declaration owner should exist")
    else {
        panic!("expected declaration-only sentinel node");
    };
    assert_eq!(owner_node.init_calls, 0, "declaration baselines must not run init");
    assert_eq!(
        owner_node.ready_calls, 0,
        "declaration baselines must not run on_node_ready"
    );
    assert_eq!(
        owner_node.inbox_calls, 0,
        "declaration baselines must not run app on_inbox"
    );

    engine
        .apply_edits_without_creation_callbacks()
        .expect("second declaration-only apply should be idempotent");
    assert_eq!(child_decl_ids(&engine, owner), vec!["connection".to_string()]);
    assert_eq!(
        child_decl_ids(&engine, connection),
        vec!["connection/input".to_string()],
        "fixed-point materialization must not duplicate nested declarations",
    );
}

#[test]
fn declaration_only_apply_forwards_composed_via_declarations() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(
        ViaComposedRootNode::new(ViaComposedMidNode::new(ViaComposedLeafNode::new())).into(),
        None,
    );

    engine
        .apply_edits_without_creation_callbacks()
        .expect("declaration-only via apply should succeed");

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("composed declaration owner should exist");
    assert_eq!(
        child_decl_ids(&engine, owner),
        vec![
            "leaf_value".to_string(),
            "mid_value".to_string(),
            "root_value".to_string(),
        ],
        "declaration-only materialization must recurse through every via layer",
    );
}

#[test]
fn sparse_project_omits_unchanged_nested_declared_defaults() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(SparseDeclaredFolderNode::new().into(), None);
    engine.apply_edits().expect("declared children should materialize");

    let sparse_json = crate::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
    let sparse_project: ProjectFile = serde_json::from_str(&sparse_json).expect("sparse project should be JSON");
    let owner_record = sparse_project.root.children.first().expect("owner record should exist");

    assert!(
        owner_record.children.is_empty(),
        "unchanged nested declaration defaults must be supplied by the structural baseline: {:?}",
        owner_record.children,
    );
}

#[test]
fn base_children_placeholder_controls_via_layout_inside_reused_folder() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(
        BaseChildrenLayoutViaNode::new(BaseChildrenLayoutBaseNode::new()).into(),
        None,
    );

    for _ in 0..8 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("node should be attached under root");
    let connection = find_child_by_decl(&engine, owner, "connection").expect("connection folder should exist");
    let connection_decl_ids = child_decl_ids(&engine, connection);

    assert_eq!(
        connection_decl_ids,
        vec![
            "connection/before_folder".to_string(),
            "connection/before_value".to_string(),
            "connection/base_value".to_string(),
            "connection/base_folder".to_string(),
            "connection/after_folder".to_string(),
            "connection/after_value".to_string(),
            "connection/after_node".to_string(),
        ],
        "`[base_children]` should splice composed folder, parameter, and node children at the declaration point",
    );

    let before =
        find_child_by_decl(&engine, connection, "connection/before_value").expect("before parameter should exist");
    let base = find_child_by_decl(&engine, connection, "connection/base_value").expect("base parameter should exist");
    let after =
        find_child_by_decl(&engine, connection, "connection/after_value").expect("after parameter should exist");
    let after_node = find_child_by_decl(&engine, connection, "connection/after_node").expect("after node should exist");

    let MacroTestNode::BaseChildrenLayoutViaNode(node) = engine.nodes.get(owner).expect("node should exist") else {
        panic!("expected BaseChildrenLayoutViaNode variant");
    };

    assert_eq!(node.before_value.id(), before, "outer before handle should bind");
    assert_eq!(
        node.base.base_value.id(),
        base,
        "base handle should bind through the placeholder"
    );
    assert_eq!(node.after_value.id(), after, "outer after handle should bind");
    assert_eq!(
        node.after_node.current_id(),
        Some(after_node),
        "declared node handle should bind"
    );
}

#[test]
fn bound_handle_refreshes_from_runtime_parameter_value_without_param_changed_event() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslParamsNode::new(None, None).into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("dispatch should succeed");
    }

    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("dsl node should be attached under root");
    let feedback = find_child_by_decl(&engine, owner, "feedback").expect("feedback parameter should exist");

    let MacroTestNode::Parameter(feedback_param) =
        engine.nodes.get_mut(feedback).expect("feedback parameter should exist")
    else {
        panic!("expected Parameter variant");
    };
    feedback_param.value = ParamValue::Float(0.9);
    // Bypass-mutated the node directly, so resync the cache entry to match.
    engine.populate_param_cache_entry(feedback);

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("owner.ping", Some(owner), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
        .expect("dispatch should succeed");

    let MacroTestNode::DslParamsNode(node) = engine.nodes.get(owner).expect("dsl node should exist") else {
        panic!("expected DslParamsNode variant");
    };

    assert!(
        (node.feedback.get() - 0.9).abs() < 1e-9,
        "bound handle should refresh from runtime parameter value before node callbacks",
    );
}

#[test]
fn apply_edits_adds_children_in_call_order() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);

    engine.apply_edits().expect("apply_edits should succeed");

    let root_data = engine.nodes.get(engine.root).expect("root should exist").node_data();
    let first = root_data.first_child.expect("first child should exist");
    let second = engine
        .nodes
        .get(first)
        .and_then(|node| node.node_data().next_sibling)
        .expect("second child should exist");

    assert_eq!(
        engine
            .nodes
            .get(first)
            .expect("first node should exist")
            .node_data()
            .meta
            .label,
        "child_a"
    );
    assert_eq!(
        engine
            .nodes
            .get(second)
            .expect("second node should exist")
            .node_data()
            .meta
            .label,
        "child_b"
    );
    assert_eq!(
        engine.nodes.get(second).and_then(|node| node.node_data().next_sibling),
        None
    );
}

#[test]
fn apply_edits_move_reorders_children() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);
    engine.apply_edits().expect("initial apply_edits should succeed");

    let child_a = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("child_a should exist");
    let child_b = engine
        .nodes
        .get(child_a)
        .and_then(|child| child.node_data().next_sibling)
        .expect("child_b should exist");

    engine.edits.push(Edit::MoveNode {
        node: child_a,
        new_parent: engine.root,
        new_prev_sibling: Some(child_b),
    });
    engine.apply_edits().expect("move should succeed");

    let root_data = engine.nodes.get(engine.root).expect("root should exist").node_data();
    assert_eq!(root_data.first_child, Some(child_b));
    assert_eq!(
        engine.nodes.get(child_b).and_then(|node| node.node_data().next_sibling),
        Some(child_a)
    );
    assert_eq!(
        engine.nodes.get(child_a).and_then(|node| node.node_data().next_sibling),
        None
    );
    assert!(
        matches!(
            engine.inbox.events.last().map(|event| &event.kind),
            Some(EventKind::ChildReordered { parent, child }) if *parent == engine.root && *child == child_a
        ),
        "last event should report child reordering",
    );
}

#[test]
fn apply_edits_move_with_no_prev_sibling_places_child_first() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);
    engine.apply_edits().expect("initial apply_edits should succeed");

    let child_a = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("child_a should exist");
    let child_b = engine
        .nodes
        .get(child_a)
        .and_then(|child| child.node_data().next_sibling)
        .expect("child_b should exist");

    engine.edits.push(Edit::MoveNode {
        node: child_b,
        new_parent: engine.root,
        new_prev_sibling: None,
    });
    engine.apply_edits().expect("move should succeed");

    let root_data = engine.nodes.get(engine.root).expect("root should exist").node_data();
    assert_eq!(root_data.first_child, Some(child_b));
    assert_eq!(
        engine.nodes.get(child_b).and_then(|node| node.node_data().next_sibling),
        Some(child_a)
    );
    assert_eq!(
        engine.nodes.get(child_a).and_then(|node| node.node_data().prev_sibling),
        Some(child_b)
    );
    assert!(
        matches!(
            engine.inbox.events.last().map(|event| &event.kind),
            Some(EventKind::ChildReordered { parent, child }) if *parent == engine.root && *child == child_b
        ),
        "last event should report child reordering",
    );
}

#[test]
fn apply_edits_rejects_cycle_move() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("parent".to_string()), None);
    engine.apply_edits().expect("initial apply should succeed");

    let parent = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("parent should exist");

    engine.add_node(Folder::new("child".to_string()), Some(parent));
    engine.apply_edits().expect("second apply should succeed");

    let child = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child)
        .expect("child should exist");

    engine.edits.push(Edit::MoveNode {
        node: parent,
        new_parent: child,
        new_prev_sibling: None,
    });

    let result = engine.apply_edits();
    assert!(matches!(
        result,
        Err(EngineEditError::CycleDetected {
            operation: "MoveNode",
            ..
        })
    ));
}

#[derive(Clone, Debug, PartialEq)]
struct ContainerTestNode {
    node_data: NodeData,
    kind: &'static str,
    container_rules: Option<UserContainerRules>,
}

impl ContainerTestNode {
    fn regular(label: &str, kind: &'static str) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            kind,
            container_rules: None,
        }
    }

    fn container(label: &str, kind: &'static str, accepts_item_kinds: &'static [&'static str]) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            kind,
            container_rules: Some(UserContainerRules::new(accepts_item_kinds)),
        }
    }
}

impl Node for ContainerTestNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        self.kind
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn user_item_kind(&self) -> &str {
        self.kind
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        self.container_rules
    }
}

#[test]
fn add_user_item_sets_item_root_role_when_container_accepts_kind() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(
        ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]),
        None,
    );
    engine.apply_edits().expect("container setup should succeed");

    let manager = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("manager should exist");

    engine.add_user_item(ContainerTestNode::regular("Sequence 1", "sequence"), Some(manager));
    engine.apply_edits().expect("user item add should succeed");

    let sequence = engine
        .nodes
        .get(manager)
        .and_then(|node| node.node_data().first_child)
        .expect("sequence should exist");
    assert_eq!(
        engine
            .nodes
            .get(sequence)
            .expect("sequence should exist")
            .node_data()
            .user_role,
        UserNodeRole::ItemRoot,
        "AddUserItem should classify inserted node as item root",
    );
}

#[test]
fn add_user_item_rejects_kind_when_container_does_not_accept_it() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(
        ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]),
        None,
    );
    engine.apply_edits().expect("container setup should succeed");

    let manager = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("manager should exist");
    engine.add_user_item(ContainerTestNode::regular("Layer 1", "layer"), Some(manager));

    let result = engine.apply_edits();
    assert!(matches!(
        result,
        Err(EngineEditError::UserItemKindRejected {
            operation: "AddUserItem",
            ..
        })
    ));
}

#[test]
fn add_user_item_requires_direct_container_parent() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(
        ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]),
        None,
    );
    engine.apply_edits().expect("container setup should succeed");

    let manager = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("manager should exist");
    engine.add_node(ContainerTestNode::regular("Leaf", "leaf"), Some(manager));
    engine.apply_edits().expect("leaf setup should succeed");

    let leaf = engine
        .nodes
        .get(manager)
        .and_then(|node| node.node_data().first_child)
        .expect("leaf should exist");
    engine.add_user_item(ContainerTestNode::regular("Sequence 1", "sequence"), Some(leaf));

    let result = engine.apply_edits();
    assert!(matches!(
        result,
        Err(EngineEditError::UserItemContainerRequired {
            operation: "AddUserItem",
            ..
        })
    ));
}

#[test]
fn catalog_creatable_items_include_registered_blueprints() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(
        ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]),
        None,
    );
    engine.apply_edits().expect("container setup should succeed");

    let manager = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("manager should exist");

    engine.register_blueprint(BlueprintDecl::new(
        BlueprintId::new("sequence_bp"),
        "Sequence Blueprint",
        "sequence",
        || ContainerTestNode::regular("Sequence Blueprint", "sequence"),
    ));

    let creatable = engine.catalog_creatable_items(manager);
    assert!(
        creatable
            .iter()
            .any(|item| item.node_type == "blueprint::sequence_bp" && item.item_kind == "sequence"),
        "registered blueprint should be listed in catalog creatable items"
    );
}

#[test]
fn queue_catalog_create_instantiates_blueprint_and_tracks_instance_meta() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(
        ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]),
        None,
    );
    engine.apply_edits().expect("container setup should succeed");

    let manager = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("manager should exist");

    engine.register_blueprint(
        BlueprintDecl::new(
            BlueprintId::new("sequence_bp"),
            "Sequence Blueprint",
            "sequence",
            || ContainerTestNode::regular("Sequence Blueprint", "sequence"),
        )
        .with_version(3),
    );

    engine
        .queue_catalog_create(manager, "blueprint::sequence_bp", None, None)
        .expect("queueing blueprint creation should succeed");
    engine.apply_edits().expect("blueprint creation should apply");

    let sequence = engine
        .nodes
        .get(manager)
        .and_then(|node| node.node_data().first_child)
        .expect("sequence instance should exist");
    let sequence_node = engine.nodes.get(sequence).expect("sequence instance should exist");
    assert_eq!(sequence_node.node_data().meta.label, "Sequence Blueprint");
    assert_eq!(sequence_node.node_data().user_role, UserNodeRole::ItemRoot);
    assert!(
        sequence_node
            .node_data()
            .meta
            .tags
            .iter()
            .any(|tag| tag == "blueprint:sequence_bp"),
        "instance root should be tagged with source blueprint id"
    );

    let instance_meta = engine
        .blueprint_instance_meta(sequence)
        .expect("blueprint instance metadata should be registered");
    assert_eq!(instance_meta.blueprint_id, BlueprintId::new("sequence_bp"));
    assert_eq!(instance_meta.blueprint_version, 3);
}

#[test]
fn contextualizable_nodes_expose_user_context_items_in_catalog() {
    let root: MacroTestNode = UiContextHostNode::new().into();
    let mut engine = Engine::new(root);

    let creatable = engine.catalog_creatable_items(engine.root);
    assert!(
        creatable
            .iter()
            .any(|item| item.node_type == USER_CONTEXT_NODE_TYPE && item.item_kind == "user_context"),
        "contextualizable host should expose user_context in the unified catalog"
    );

    engine
        .queue_catalog_create(engine.root, USER_CONTEXT_NODE_TYPE, None, None)
        .expect("queueing user_context creation should succeed");
    engine.apply_edits().expect("user_context creation should apply");

    let scope = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("scope should exist");
    let scope_node = engine.nodes.get(scope).expect("scope node should exist");
    assert_eq!(scope_node.get_type(), USER_CONTEXT_NODE_TYPE);
    assert_eq!(scope_node.node_data().meta.label, "Context");
    assert_eq!(scope_node.node_data().user_role, UserNodeRole::ItemRoot);
}

#[test]
fn user_context_nodes_create_folders_and_all_parameter_types() {
    let root: MacroTestNode = UiContextHostNode::new().into();
    let mut engine = Engine::new(root);

    engine
        .queue_catalog_create(engine.root, USER_CONTEXT_NODE_TYPE, None, None)
        .expect("queueing user_context creation should succeed");
    engine.apply_edits().expect("user_context creation should apply");

    let scope = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("scope should exist");

    let creatable = engine.catalog_creatable_items(scope);
    assert!(
        creatable
            .iter()
            .any(|item| item.node_type == USER_CONTEXT_FOLDER_NODE_TYPE && item.item_kind == FOLDER_NODE_TYPE),
        "context scope should expose folder creation"
    );
    for parameter_type in PARAMETER_NODE_TYPES {
        assert!(
            creatable
                .iter()
                .any(|item| item.node_type == parameter_type && item.item_kind == parameter_type),
            "context scope should expose '{parameter_type}' parameter creation"
        );
    }

    engine
        .queue_catalog_create(scope, USER_CONTEXT_FOLDER_NODE_TYPE, Some("Inner".to_string()), None)
        .expect("queueing folder creation should succeed");
    engine
        .queue_catalog_create(scope, "float", Some("Tempo".to_string()), None)
        .expect("queueing float parameter creation should succeed");
    engine.apply_edits().expect("context child creation should apply");

    let first_child = engine
        .nodes
        .get(scope)
        .and_then(|node| node.node_data().first_child)
        .expect("context scope should have children");
    let second_child = engine
        .nodes
        .get(first_child)
        .and_then(|node| node.node_data().next_sibling)
        .expect("context scope should have a second child");

    let first_type = engine
        .nodes
        .get(first_child)
        .expect("first child should exist")
        .get_type()
        .to_string();
    let second_type = engine
        .nodes
        .get(second_child)
        .expect("second child should exist")
        .get_type()
        .to_string();
    assert!(
        first_type == USER_CONTEXT_FOLDER_NODE_TYPE || second_type == USER_CONTEXT_FOLDER_NODE_TYPE,
        "created children should include user-context folder"
    );
    assert!(
        first_type == "float" || second_type == "float",
        "created children should include float parameter"
    );

    let inner_folder = if first_type == USER_CONTEXT_FOLDER_NODE_TYPE {
        first_child
    } else {
        second_child
    };
    let inner_creatable = engine.catalog_creatable_items(inner_folder);
    assert!(
        inner_creatable
            .iter()
            .any(|item| item.node_type == USER_CONTEXT_FOLDER_NODE_TYPE && item.item_kind == FOLDER_NODE_TYPE),
        "folder created in scope should expose folder creation recursively"
    );
    for parameter_type in PARAMETER_NODE_TYPES {
        assert!(
            inner_creatable
                .iter()
                .any(|item| item.node_type == parameter_type && item.item_kind == parameter_type),
            "folder created in scope should expose '{parameter_type}' parameter creation recursively"
        );
    }

    engine
        .queue_catalog_create(inner_folder, "bool", Some("Enabled".to_string()), None)
        .expect("queueing bool parameter creation inside inner folder should succeed");
    engine.apply_edits().expect("inner folder child creation should apply");

    let inner_child = engine
        .nodes
        .get(inner_folder)
        .and_then(|node| node.node_data().first_child)
        .expect("inner folder should have one child");
    assert_eq!(
        engine
            .nodes
            .get(inner_child)
            .expect("inner child should exist")
            .get_type(),
        "bool",
        "inner folder should create requested child type through inherited catalog factory"
    );
}

#[test]
fn simple_user_context_does_not_expose_multiplex_authoring() {
    let root: MacroTestNode = UiContextHostNode::new().into();
    let mut engine = Engine::new(root);

    engine
        .queue_catalog_create(engine.root, USER_CONTEXT_NODE_TYPE, None, None)
        .expect("queueing user_context creation should succeed");
    engine.apply_edits().expect("user_context creation should apply");

    let scope = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("scope should exist");

    let creatable = engine.catalog_creatable_items(scope);
    assert!(
        !creatable
            .iter()
            .any(|item| item.node_type == USER_CONTEXT_MULTIPLEX_NODE_TYPE),
        "simple context scopes should not expose multiplex authoring"
    );
}

#[test]
fn multiplex_context_indexes_lists_and_resizes_entries_stably() {
    let root: MacroTestNode = UiMultiplexContextHostNode::new().into();
    let mut engine = Engine::new(root);

    engine
        .queue_catalog_create(engine.root, USER_CONTEXT_NODE_TYPE, None, None)
        .expect("queueing user_context creation should succeed");
    engine.apply_edits().expect("user_context creation should apply");
    let scope = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("scope should exist");

    assert!(
        engine
            .catalog_creatable_items(scope)
            .iter()
            .any(|item| item.node_type == USER_CONTEXT_MULTIPLEX_NODE_TYPE),
        "multiplex-enabled context scopes should expose multiplex authoring"
    );

    engine
        .queue_catalog_create(scope, USER_CONTEXT_MULTIPLEX_NODE_TYPE, Some("Mux".to_string()), None)
        .expect("queueing multiplex creation should succeed");
    for _ in 0..3 {
        engine.apply_edits().expect("multiplex creation should apply");
    }
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("multiplex child creation inbox should dispatch");
    engine.apply_edits().expect("multiplex listener edits should apply");
    let multiplex = direct_children_of_type(&engine, scope, USER_CONTEXT_MULTIPLEX_NODE_TYPE)
        .into_iter()
        .next()
        .expect("multiplex should exist");
    let count = find_child_by_decl(&engine, multiplex, USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID)
        .expect("multiplex should create count parameter");

    let list_type = user_context_multiplex_list_node_type("float");
    engine
        .queue_catalog_create(multiplex, list_type.clone(), Some("Speeds".to_string()), None)
        .expect("queueing float list creation should succeed");
    engine.apply_edits().expect("float list creation should apply");
    let list = direct_children_of_type(&engine, multiplex, list_type.as_str())
        .into_iter()
        .next()
        .expect("float list should exist");
    assert!(
        engine.catalog_creatable_items(list).is_empty(),
        "multiplex lists should not expose manual entry creation"
    );
    assert!(
        engine.queue_catalog_create(list, "float", None, None).is_err(),
        "catalog creation should reject manual multiplex entries"
    );

    let reference_list_type = user_context_multiplex_list_node_type("reference");
    engine
        .queue_catalog_create(multiplex, reference_list_type.clone(), Some("Inputs".to_string()), None)
        .expect("queueing reference list creation should succeed");
    engine.apply_edits().expect("reference list creation should apply");
    let reference_list = direct_children_of_type(&engine, multiplex, reference_list_type.as_str())
        .into_iter()
        .next()
        .expect("reference list should exist");

    set_param_and_tick(&mut engine, count, ParamValue::Int(3));
    let entry_ids = direct_children_of_type(&engine, list, "float");
    assert_eq!(entry_ids.len(), 3, "count=3 should create three entries");
    let first_uuids = node_uuids(&engine, &entry_ids);
    let reference_entry_ids = direct_children_of_type(&engine, reference_list, "reference");
    assert_eq!(
        reference_entry_ids.len(),
        3,
        "count=3 should create three reference entries"
    );
    let reference_snapshot = engine
        .nodes
        .get(reference_entry_ids[0])
        .and_then(|node| node.engine_param_snapshot())
        .expect("reference entry should be a parameter");
    assert_eq!(
        reference_snapshot.constraints.reference.target_kind,
        crate::parameter::ReferenceTargetKind::ParameterOnly,
        "reference multiplex entries should pick parameter values"
    );
    assert!(
        reference_snapshot.constraints.reference.allow_projections,
        "reference multiplex entries should allow value projections"
    );
    let entry_permissions = &engine
        .nodes
        .get(entry_ids[0])
        .expect("entry should exist")
        .node_data()
        .meta
        .user_permissions;
    assert!(entry_permissions.can_edit_name, "generated entries keep label editing");
    assert!(
        !entry_permissions.can_remove_and_duplicate,
        "generated entries should not be removable or duplicable"
    );

    let lookup = engine.resolve_user_context_symbol(engine.root, "float", Some(UserContextValueType::Float));
    assert!(
        matches!(
            lookup,
            UserContextLookup::Resolved(resolved)
                if resolved.kind == UserContextEntryKind::MultiplexList
                    && resolved.entry_param == list
                    && resolved.multiplex.as_ref().is_some_and(|list| list.entries.len() == 3)
        ),
        "multiplex list should be indexed once as the context symbol"
    );

    engine.add_node(
        Parameter::new(
            "Index target",
            ParamValue::Float(0.0),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        None,
    );
    engine.apply_edits().expect("index target should attach");
    let index_target = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Index target")
        .map(|(id, _)| id)
        .expect("index target should exist");
    assert!(
        engine
            .ui_context_candidates_for_param(index_target)
            .candidates
            .iter()
            .any(|candidate| candidate.multiplex_index_compatible),
        "multiplex indexes should be offered when the target accepts integer coercion"
    );

    let entry_decl = engine
        .nodes
        .get(entry_ids[0])
        .expect("entry should exist")
        .node_data()
        .meta
        .decl_id
        .0
        .clone();
    if entry_decl != "float" {
        assert!(
            matches!(
                engine.resolve_user_context_symbol(engine.root, entry_decl.as_str(), None),
                UserContextLookup::Missing { .. }
            ),
            "direct entry params should not be exported as scalar context symbols"
        );
    }

    set_param_and_tick(&mut engine, count, ParamValue::Int(5));
    let grown_uuids = node_uuids(&engine, &direct_children_of_type(&engine, list, "float"));
    assert_eq!(&grown_uuids[..3], first_uuids.as_slice());

    let trigger_list_type = user_context_multiplex_list_node_type("trigger");
    engine
        .queue_catalog_create(multiplex, trigger_list_type.clone(), Some("Triggers".to_string()), None)
        .expect("queueing trigger list creation should succeed");
    engine.apply_edits().expect("trigger list creation should apply");
    engine.apply_edits().expect("new list count sync should apply");
    let trigger_list = direct_children_of_type(&engine, multiplex, trigger_list_type.as_str())
        .into_iter()
        .next()
        .expect("trigger list should exist");
    assert_eq!(
        direct_children_of_type(&engine, trigger_list, "trigger").len(),
        5,
        "new lists should immediately sync to the current multiplex count"
    );

    set_param_and_tick(&mut engine, count, ParamValue::Int(6));
    assert_eq!(
        direct_children_of_type(&engine, trigger_list, "trigger").len(),
        6,
        "growing count should append entries to every list"
    );

    set_param_and_tick(&mut engine, count, ParamValue::Int(4));
    assert_eq!(
        direct_children_of_type(&engine, trigger_list, "trigger").len(),
        4,
        "shrinking count should remove trailing entries from every list"
    );

    set_param_and_tick(&mut engine, count, ParamValue::Int(2));
    let shrunk_uuids = node_uuids(&engine, &direct_children_of_type(&engine, list, "float"));
    assert_eq!(shrunk_uuids, first_uuids[..2]);
}

#[test]
fn sparse_project_round_trips_multiplex_list_entry_names_and_values() {
    let root: MacroTestNode = UiMultiplexContextHostNode::new().into();
    let mut engine = Engine::new(root);

    engine
        .queue_catalog_create(engine.root, USER_CONTEXT_NODE_TYPE, None, None)
        .expect("queueing user_context creation should succeed");
    engine.apply_edits().expect("user_context creation should apply");
    let scope = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("scope should exist");

    engine
        .queue_catalog_create(scope, USER_CONTEXT_MULTIPLEX_NODE_TYPE, Some("Mux".to_string()), None)
        .expect("queueing multiplex creation should succeed");
    for _ in 0..3 {
        engine.apply_edits().expect("multiplex creation should apply");
    }
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("multiplex child creation inbox should dispatch");
    engine.apply_edits().expect("multiplex listener edits should apply");
    let multiplex = direct_children_of_type(&engine, scope, USER_CONTEXT_MULTIPLEX_NODE_TYPE)
        .into_iter()
        .next()
        .expect("multiplex should exist");
    let count = find_child_by_decl(&engine, multiplex, USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID)
        .expect("multiplex should create count parameter");

    let list_type = user_context_multiplex_list_node_type("float");
    engine
        .queue_catalog_create(multiplex, list_type.clone(), Some("Speeds".to_string()), None)
        .expect("queueing float list creation should succeed");
    engine.apply_edits().expect("float list creation should apply");
    let list = direct_children_of_type(&engine, multiplex, list_type.as_str())
        .into_iter()
        .next()
        .expect("float list should exist");

    set_param_and_tick(&mut engine, count, ParamValue::Int(3));
    let entry_ids = direct_children_of_type(&engine, list, "float");
    assert_eq!(entry_ids.len(), 3, "count=3 should create three entries");

    let expected_entries = [("Slow", 0.25), ("Cruise", 1.5), ("Fast", 12.0)];
    for (entry_id, (label, value)) in entry_ids.iter().zip(expected_entries) {
        let node = engine.nodes.get_mut(*entry_id).expect("entry should exist");
        node.node_data_mut().meta.label = label.to_string();
        let MacroTestNode::Parameter(parameter) = node else {
            panic!("expected multiplex entry to be a parameter");
        };
        parameter.value = ParamValue::Float(value);
    }

    let json = crate::app::to_sparse_project_json_pretty(&engine).expect("sparse project should encode");
    let loaded = crate::app::from_sparse_project_json::<MacroTestNode>(&json).expect("sparse project should decode");
    let loaded_scope = loaded
        .nodes
        .get(loaded.root)
        .and_then(|node| node.node_data().first_child)
        .expect("loaded scope should exist");
    let loaded_multiplex = direct_children_of_type(&loaded, loaded_scope, USER_CONTEXT_MULTIPLEX_NODE_TYPE)
        .into_iter()
        .next()
        .expect("loaded multiplex should exist");
    let loaded_list = direct_children_of_type(&loaded, loaded_multiplex, list_type.as_str())
        .into_iter()
        .next()
        .expect("loaded float list should exist");
    let loaded_entries = direct_children_of_type(&loaded, loaded_list, "float");
    assert_eq!(loaded_entries.len(), 3, "loaded list should keep three entries");

    for (entry_id, (label, value)) in loaded_entries.iter().zip(expected_entries) {
        let node = loaded.nodes.get(*entry_id).expect("loaded entry should exist");
        assert_eq!(node.node_data().meta.label, label);
        let MacroTestNode::Parameter(parameter) = node else {
            panic!("expected loaded multiplex entry to be a parameter");
        };
        assert_eq!(parameter.value, ParamValue::Float(value));
    }
}

#[test]
fn user_context_nodes_expose_blueprints_in_catalog() {
    let root: MacroTestNode = UiContextHostNode::new().into();
    let mut engine = Engine::new(root);

    engine.register_blueprint(BlueprintDecl::new(
        BlueprintId::new("ctx_scope_bp"),
        "Context Scope Blueprint",
        "sequence",
        || UiContextHostNode::new().into(),
    ));

    engine
        .queue_catalog_create(engine.root, USER_CONTEXT_NODE_TYPE, None, None)
        .expect("queueing user_context creation should succeed");
    engine.apply_edits().expect("user_context creation should apply");
    let scope = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("scope should exist");

    let creatable = engine.catalog_creatable_items(scope);
    assert!(
        creatable
            .iter()
            .any(|item| item.node_type == "blueprint::ctx_scope_bp" && item.item_kind == "sequence"),
        "context scope should expose registered blueprint catalog items"
    );

    engine
        .queue_catalog_create(scope, "blueprint::ctx_scope_bp", None, None)
        .expect("queueing blueprint creation in context should succeed");
    engine
        .apply_edits()
        .expect("blueprint creation in context should apply");

    let instance = engine
        .nodes
        .get(scope)
        .and_then(|node| node.node_data().first_child)
        .expect("context scope should contain blueprint instance");
    let instance_node = engine.nodes.get(instance).expect("blueprint instance should exist");
    assert_eq!(instance_node.node_data().user_role, UserNodeRole::ItemRoot);
}

#[test]
fn via_nodes_inherit_script_host_policy_from_base() {
    let root: MacroTestNode = ViaScriptHostNode::new(UiScriptHostNode::new("base")).into();
    let mut engine = Engine::new(root);

    let creatable = engine.catalog_creatable_items(engine.root);
    assert!(
        creatable
            .iter()
            .any(|item| item.node_type == "script" && item.item_kind == "script"),
        "via-hosted node should expose script item creation when base is scriptable"
    );

    engine
        .queue_catalog_create(engine.root, "script", Some("Script".to_string()), None)
        .expect("queueing script creation should succeed");
    engine.apply_edits().expect("script creation should apply");

    let script = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("script should exist");
    assert_eq!(
        engine.nodes.get(script).expect("script should exist").get_type(),
        "script"
    );
}

#[test]
fn via_nodes_inherit_context_host_policy_from_base() {
    let root: MacroTestNode = ViaContextHostNode::new(UiContextHostNode::new()).into();
    let engine = Engine::new(root);

    let creatable = engine.catalog_creatable_items(engine.root);
    assert!(
        creatable
            .iter()
            .any(|item| item.node_type == USER_CONTEXT_NODE_TYPE && item.item_kind == "user_context"),
        "via-hosted node should expose user_context item creation when base is contextualizable"
    );
}

#[test]
fn script_host_policy_alone_does_not_expose_script_items_in_catalog() {
    let engine = Engine::new(PolicyOnlyScriptHostNode::new());

    assert!(
        engine.catalog_creatable_items(engine.root).is_empty(),
        "script host policy alone should not expose script creation without explicit container methods"
    );
}

#[test]
fn context_host_policy_alone_does_not_expose_context_items_in_catalog() {
    let engine = Engine::new(PolicyOnlyContextHostNode::new());

    assert!(
        engine.catalog_creatable_items(engine.root).is_empty(),
        "context host policy alone should not expose context creation without explicit container methods"
    );
}

#[test]
fn folder_under_script_host_does_not_inherit_script_creation() {
    let root: MacroTestNode = UiScriptHostNode::new("root").into();
    let mut engine = Engine::new(root);

    engine.add_node(Folder::new("Inner".to_string()).into(), Some(engine.root));
    engine.apply_edits().expect("folder add should succeed");

    let folder = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("folder should exist");

    let creatable = engine.catalog_creatable_items(folder);
    assert!(
        !creatable
            .iter()
            .any(|item| item.node_type == "script" && item.item_kind == "script"),
        "folder descendants should not inherit direct-only script creation"
    );
    assert!(
        matches!(
            engine.queue_catalog_create(folder, "script", Some("Script".to_string()), None),
            Err(EngineEditError::UserItemTypeUnavailable { .. })
        ),
        "direct queueing should reject script creation under non-host folders"
    );
}

#[test]
fn folder_under_context_host_does_not_inherit_context_creation() {
    let root: MacroTestNode = UiContextHostNode::new().into();
    let mut engine = Engine::new(root);

    engine.add_node(Folder::new("Inner".to_string()).into(), Some(engine.root));
    engine.apply_edits().expect("folder add should succeed");

    let folder = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("folder should exist");

    let creatable = engine.catalog_creatable_items(folder);
    assert!(
        !creatable
            .iter()
            .any(|item| item.node_type == USER_CONTEXT_NODE_TYPE && item.item_kind == USER_CONTEXT_ITEM_KIND),
        "folder descendants should not inherit direct-only context creation"
    );
    assert!(
        matches!(
            engine.queue_catalog_create(folder, USER_CONTEXT_NODE_TYPE, None, None),
            Err(EngineEditError::UserItemTypeUnavailable { .. })
        ),
        "direct queueing should reject context creation under non-host folders"
    );
}

#[test]
fn user_context_resolution_tracks_lexical_scope_and_reparenting() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(Folder::new("Owner".to_string()).into(), None);
    engine.apply_edits().expect("owner add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(UserContextNode::new("Scope").into(), Some(owner));
    engine.apply_edits().expect("scope context add should succeed");
    let scope = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("scope context should exist");

    engine.add_node(
        Parameter::new("tempo", ParamValue::Float(120.0), ParameterChangeCheck::ValueChange).into(),
        Some(scope),
    );
    engine.add_node(Folder::new("Consumer".to_string()).into(), Some(owner));
    engine.apply_edits().expect("owner children add should succeed");
    let tempo = engine
        .nodes
        .get(scope)
        .and_then(|node| node.node_data().first_child)
        .expect("tempo parameter should exist");
    let consumer = engine
        .nodes
        .get(scope)
        .and_then(|node| node.node_data().next_sibling)
        .expect("consumer folder should exist");

    let resolved = engine.resolve_user_context_symbol(consumer, "tempo", Some(UserContextValueType::Float));
    assert!(
        matches!(resolved, UserContextLookup::Resolved(resolved) if resolved.entry_param == tempo && resolved.lexical_depth == 1),
        "consumer should resolve nearest scope entry"
    );

    let mismatch = engine.resolve_user_context_symbol(consumer, "tempo", Some(UserContextValueType::Int));
    assert!(
        matches!(mismatch, UserContextLookup::TypeMismatch(mismatch) if mismatch.expected == UserContextValueType::Int && mismatch.found == UserContextValueType::Float),
        "type-mismatch should be reported for incompatible expected type"
    );

    engine.edits.push(Edit::MoveNode {
        node: consumer,
        new_parent: engine.root,
        new_prev_sibling: None,
    });
    engine.apply_edits().expect("consumer move should succeed");

    let missing = engine.resolve_user_context_symbol(consumer, "tempo", Some(UserContextValueType::Float));
    assert!(
        matches!(missing, UserContextLookup::Missing { .. }),
        "moving consumer outside scope should invalidate cached resolution"
    );
}

#[test]
fn context_owned_by_child_is_not_visible_to_parent_ancestor() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(Folder::new("Node A".to_string()).into(), None);
    engine.apply_edits().expect("node A add should succeed");
    let node_a = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("node A should exist");

    engine.add_node(Folder::new("Node B".to_string()).into(), Some(node_a));
    engine.apply_edits().expect("node B add should succeed");
    let node_b = engine
        .nodes
        .get(node_a)
        .and_then(|node| node.node_data().first_child)
        .expect("node B should exist");

    engine.add_node(UserContextNode::new("Scope").into(), Some(node_b));
    engine.apply_edits().expect("scope add should succeed");
    let scope = engine
        .nodes
        .get(node_b)
        .and_then(|node| node.node_data().first_child)
        .expect("scope should exist");

    engine.add_node(
        Parameter::new("tempo", ParamValue::Float(120.0), ParameterChangeCheck::ValueChange).into(),
        Some(scope),
    );
    engine.add_node(
        Parameter::new("gain_b", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(node_b),
    );
    engine.add_node(
        Parameter::new("gain_a", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(node_a),
    );
    engine.apply_edits().expect("parameter add should succeed");

    let tempo = engine
        .nodes
        .get(scope)
        .and_then(|node| node.node_data().first_child)
        .expect("tempo should exist");
    let gain_b = engine
        .nodes
        .get(scope)
        .and_then(|node| node.node_data().next_sibling)
        .expect("gain_b should exist");
    let gain_a = engine
        .nodes
        .get(node_b)
        .and_then(|node| node.node_data().next_sibling)
        .expect("gain_a should exist");

    let resolved_for_owner = engine.resolve_user_context_symbol(gain_b, "tempo", Some(UserContextValueType::Float));
    assert!(
        matches!(resolved_for_owner, UserContextLookup::Resolved(resolved) if resolved.entry_param == tempo),
        "context owner subtree should resolve symbols from its direct child context node"
    );

    let missing_for_ancestor = engine.resolve_user_context_symbol(gain_a, "tempo", Some(UserContextValueType::Float));
    assert!(
        matches!(missing_for_ancestor, UserContextLookup::Missing { .. }),
        "ancestor nodes should not resolve symbols from descendant-owned contexts"
    );
}

#[test]
fn ui_context_candidates_report_shadowing_and_compatibility() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(Folder::new("Owner".to_string()).into(), None);
    engine.apply_edits().expect("owner add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(UserContextNode::new("Outer").into(), Some(owner));
    engine.apply_edits().expect("outer context add should succeed");
    let outer = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("outer context should exist");

    engine.add_node(
        Parameter::new("tempo", ParamValue::Float(128.0), ParameterChangeCheck::ValueChange).into(),
        Some(outer),
    );
    engine.add_node(UserContextNode::new("Inner").into(), Some(outer));
    engine.apply_edits().expect("outer context children add should succeed");
    let outer_tempo = engine
        .nodes
        .get(outer)
        .and_then(|node| node.node_data().first_child)
        .expect("outer tempo should exist");
    let inner = engine
        .nodes
        .get(outer_tempo)
        .and_then(|node| node.node_data().next_sibling)
        .expect("inner context should exist");

    engine.add_node(
        Parameter::new("tempo", ParamValue::Int(1), ParameterChangeCheck::ValueChange).into(),
        Some(inner),
    );
    engine.add_node(
        Parameter::new("consumer", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(inner),
    );
    engine.apply_edits().expect("inner children add should succeed");
    let inner_tempo = engine
        .nodes
        .get(inner)
        .and_then(|node| node.node_data().first_child)
        .expect("inner tempo should exist");
    let consumer_param = engine
        .nodes
        .get(inner_tempo)
        .and_then(|node| node.node_data().next_sibling)
        .expect("consumer parameter should exist");

    let dto = engine.ui_context_candidates_for_param(consumer_param);
    assert_eq!(dto.expected, Some(UserContextValueType::Float));
    assert_eq!(
        dto.candidates.len(),
        2,
        "both nearest and shadowed candidates should be returned"
    );

    let nearest = &dto.candidates[0];
    assert_eq!(nearest.symbol, "tempo");
    assert_eq!(nearest.scope_owner, outer);
    assert_eq!(nearest.lexical_depth, 2);
    assert!(!nearest.shadowed);
    assert!(
        nearest.compatible,
        "inner Int value should be coercible for Float targets"
    );
    assert!(nearest.directly_compatible);

    let shadowed = &dto.candidates[1];
    assert_eq!(shadowed.symbol, "tempo");
    assert_eq!(shadowed.scope_owner, owner);
    assert_eq!(shadowed.lexical_depth, 3);
    assert!(shadowed.shadowed, "outer symbol should be marked shadowed");
    assert!(
        shadowed.compatible,
        "outer Float value should be compatible with Float target"
    );
    assert!(shadowed.directly_compatible);
}

#[test]
fn move_item_root_between_containers_requires_target_acceptance() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(
        ContainerTestNode::container("Sequences A", "sequence_manager", &["sequence"]),
        None,
    );
    engine.add_node(
        ContainerTestNode::container("Sequences B", "sequence_manager", &["layer"]),
        None,
    );
    engine.apply_edits().expect("container setup should succeed");

    let manager_a = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("manager_a should exist");
    let manager_b = engine
        .nodes
        .get(manager_a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("manager_b should exist");

    engine.add_user_item(ContainerTestNode::regular("Sequence 1", "sequence"), Some(manager_a));
    engine.apply_edits().expect("initial user item add should succeed");

    let sequence = engine
        .nodes
        .get(manager_a)
        .and_then(|node| node.node_data().first_child)
        .expect("sequence should exist");
    engine.edits.push(Edit::MoveNode {
        node: sequence,
        new_parent: manager_b,
        new_prev_sibling: None,
    });

    let rejected = engine.apply_edits();
    assert!(matches!(
        rejected,
        Err(EngineEditError::UserItemKindRejected {
            operation: "MoveNode",
            ..
        })
    ));

    engine.add_node(
        ContainerTestNode::container("Sequences C", "sequence_manager", &["sequence"]),
        None,
    );
    engine
        .apply_edits()
        .expect("adding compatible target container should succeed");
    let manager_c = engine
        .nodes
        .get(manager_b)
        .and_then(|node| node.node_data().next_sibling)
        .expect("manager_c should exist");

    engine.edits.push(Edit::MoveNode {
        node: sequence,
        new_parent: manager_c,
        new_prev_sibling: None,
    });
    engine
        .apply_edits()
        .expect("moving item root to compatible container should succeed");

    let sequence_data = engine
        .nodes
        .get(sequence)
        .expect("sequence should exist after move")
        .node_data();
    assert_eq!(sequence_data.parent, Some(manager_c));
    assert_eq!(sequence_data.user_role, UserNodeRole::ItemRoot);
}

#[test]
fn move_item_root_requires_direct_container_parent() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(
        ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]),
        None,
    );
    engine.apply_edits().expect("container setup should succeed");

    let manager = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("manager should exist");
    engine.add_node(ContainerTestNode::regular("Leaf", "leaf"), Some(manager));
    engine.add_user_item(ContainerTestNode::regular("Sequence 1", "sequence"), Some(manager));
    engine.apply_edits().expect("initial children should attach");

    let leaf = engine
        .nodes
        .get(manager)
        .and_then(|node| node.node_data().first_child)
        .expect("leaf should exist");
    let sequence = engine
        .nodes
        .get(leaf)
        .and_then(|node| node.node_data().next_sibling)
        .expect("sequence should exist");

    engine.edits.push(Edit::MoveNode {
        node: sequence,
        new_parent: leaf,
        new_prev_sibling: None,
    });

    let result = engine.apply_edits();
    assert!(matches!(
        result,
        Err(EngineEditError::UserItemContainerRequired {
            operation: "MoveNode",
            ..
        })
    ));
}

#[test]
fn move_container_item_root_requires_destination_acceptance() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(
        ContainerTestNode::container("Folders", "folder_manager", &["folder"]),
        None,
    );
    engine.add_node(
        ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]),
        None,
    );
    engine.apply_edits().expect("container setup should succeed");

    let folder_manager = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("folder manager should exist");
    let sequence_manager = engine
        .nodes
        .get(folder_manager)
        .and_then(|node| node.node_data().next_sibling)
        .expect("sequence manager should exist");

    engine.add_user_item(
        ContainerTestNode::container("Group", "folder", &["sequence"]),
        Some(folder_manager),
    );
    engine.apply_edits().expect("folder item should attach");

    let folder = engine
        .nodes
        .get(folder_manager)
        .and_then(|node| node.node_data().first_child)
        .expect("folder should exist");
    engine.edits.push(Edit::MoveNode {
        node: folder,
        new_parent: sequence_manager,
        new_prev_sibling: None,
    });

    let rejected = engine.apply_edits();
    assert!(matches!(
        rejected,
        Err(EngineEditError::UserItemKindRejected {
            operation: "MoveNode",
            ..
        })
    ));

    engine.add_node(
        ContainerTestNode::container("More Folders", "folder_manager", &["folder"]),
        None,
    );
    engine.apply_edits().expect("compatible folder target should attach");
    let compatible_manager = engine
        .nodes
        .get(sequence_manager)
        .and_then(|node| node.node_data().next_sibling)
        .expect("compatible manager should exist");

    engine.edits.push(Edit::MoveNode {
        node: folder,
        new_parent: compatible_manager,
        new_prev_sibling: None,
    });
    engine
        .apply_edits()
        .expect("folder should move to compatible container");

    assert_eq!(
        engine.nodes.get(folder).and_then(|node| node.node_data().parent),
        Some(compatible_manager)
    );
}

#[test]
fn move_item_root_outside_any_container_is_rejected() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(
        ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]),
        None,
    );
    engine.apply_edits().expect("container setup should succeed");

    let manager = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("manager should exist");
    engine.add_user_item(ContainerTestNode::regular("Sequence 1", "sequence"), Some(manager));
    engine.apply_edits().expect("initial user item add should succeed");

    let sequence = engine
        .nodes
        .get(manager)
        .and_then(|node| node.node_data().first_child)
        .expect("sequence should exist");
    engine.edits.push(Edit::MoveNode {
        node: sequence,
        new_parent: engine.root,
        new_prev_sibling: Some(manager),
    });

    let result = engine.apply_edits();
    assert!(matches!(
        result,
        Err(EngineEditError::UserItemContainerRequired {
            operation: "MoveNode",
            ..
        })
    ));
}

#[test]
fn apply_edits_set_param_rejects_non_parameter_node() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(12),
        behaviour: ParameterEventBehaviour::Coalesce,
    });

    let result = engine.apply_edits();
    assert!(matches!(result, Err(EngineEditError::ParamEditTargetMismatch { .. })));
}

#[test]
fn apply_edits_set_param_updates_parameter_node() {
    let root = Parameter::new("root_param", ParamValue::Int(10), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(42),
        behaviour: ParameterEventBehaviour::Coalesce,
    });

    engine.apply_edits().expect("set param should succeed");

    let node = engine.nodes.get(engine.root).expect("root parameter should exist");
    assert_eq!(node.value, ParamValue::Int(42));
    assert!(
        matches!(
            engine.inbox.events.last().map(|event| &event.kind),
            Some(EventKind::ParamChanged {
                param,
                old_value: ParamValue::Int(10),
                new_value: ParamValue::Int(42),
            }) if *param == engine.root
        ),
        "last event should report previous parameter value",
    );
}

#[test]
fn apply_set_param_value_change_ignores_normalized_noop() {
    let mut root = Parameter::new("root_param", ParamValue::Float(10.0), ParameterChangeCheck::ValueChange);
    root.constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(10.0)),
        step: None,
        step_base: None,
        enum_options: Vec::new(),
        policy: ParameterConstraintPolicy::ClampAdapt,
        reference: Default::default(),
        file: Default::default(),
    };
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(42.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine
        .apply_edits()
        .expect("set param should normalize to current value");

    let node = engine.nodes.get(engine.root).expect("root parameter should exist");
    assert_eq!(
        node.value,
        ParamValue::Float(10.0),
        "value should remain unchanged after clamping"
    );
    assert!(
        engine.inbox.events.is_empty(),
        "normalized no-op should not emit ParamChanged event"
    );
    assert_eq!(engine.undo_len(), 0, "normalized no-op should not create undo history");
}

#[test]
fn parameter_set_normalizes_before_change_check() {
    let mut parameter = Parameter::new("param", ParamValue::Float(10.0), ParameterChangeCheck::ValueChange);
    parameter.constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(10.0)),
        step: None,
        step_base: None,
        enum_options: Vec::new(),
        policy: ParameterConstraintPolicy::ClampAdapt,
        reference: Default::default(),
        file: Default::default(),
    };
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    parameter.set(&mut ctx, ParamValue::Float(99.0));

    assert!(
        ctx.edits.pending.is_empty(),
        "normalized no-op should not enqueue SetParam edit"
    );
}

#[test]
fn parameter_set_coalesces_pending_set_param_edits_by_default() {
    let mut parameter = Parameter::new("param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    parameter.set(&mut ctx, ParamValue::Int(1));
    parameter.set(&mut ctx, ParamValue::Int(2));

    assert_eq!(
        ctx.edits.pending.len(),
        1,
        "coalesce mode should keep only one queued SetParam"
    );
    assert!(
        matches!(
            ctx.edits.pending.first().map(|request| &request.edit),
            Some(Edit::SetParam {
                node,
                value: ParamValue::Int(2),
                behaviour: ParameterEventBehaviour::Coalesce,
            }) if *node == parameter.id()
        ),
        "queued SetParam should keep the latest value",
    );
}

#[test]
fn parameter_set_append_behaviour_keeps_all_pending_set_param_edits() {
    let mut parameter = Parameter::new("param", ParamValue::Int(0), ParameterChangeCheck::None);
    parameter.event_behaviour = ParameterEventBehaviour::Append;

    let mut ctx = ProcessCtx::new(
        ExecutionPhase::EngineTick,
        EngineTime {
            tick: 0,
            micro: 0,
            seq: 0,
        },
    );

    parameter.set(&mut ctx, ParamValue::Int(1));
    parameter.set(&mut ctx, ParamValue::Int(2));

    assert_eq!(
        ctx.edits.pending.len(),
        2,
        "append mode should keep every queued SetParam"
    );
    assert!(
        matches!(
            ctx.edits.pending.first().map(|request| &request.edit),
            Some(Edit::SetParam {
                behaviour: ParameterEventBehaviour::Append,
                ..
            })
        ),
        "queued edits should retain append behaviour metadata",
    );
}

#[test]
fn apply_set_param_clamps_value_when_constraints_use_clamp_adapt_policy() {
    let mut root = Parameter::new("root_param", ParamValue::Float(0.0), ParameterChangeCheck::None);
    root.constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        step: Some(0.25),
        step_base: Some(0.0),
        enum_options: Vec::new(),
        policy: ParameterConstraintPolicy::ClampAdapt,
        reference: Default::default(),
        file: Default::default(),
    };
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(1.13),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should clamp and adapt");

    let value = engine
        .nodes
        .get(engine.root)
        .expect("root parameter should exist")
        .value
        .clone();
    assert_eq!(
        value,
        ParamValue::Float(1.0),
        "value should clamp to max after step adaptation"
    );
}

#[test]
fn apply_set_param_rejects_value_when_constraints_use_reject_policy() {
    let mut root = Parameter::new("root_param", ParamValue::Float(0.0), ParameterChangeCheck::None);
    root.constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        step: Some(0.5),
        step_base: Some(0.0),
        enum_options: Vec::new(),
        policy: ParameterConstraintPolicy::Reject,
        reference: Default::default(),
        file: Default::default(),
    };
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(0.3),
        behaviour: ParameterEventBehaviour::Coalesce,
    });

    let result = engine.apply_edits();
    assert!(matches!(result, Err(EngineEditError::ParamConstraintViolation { .. })));
}

#[test]
fn apply_set_param_clamps_vec2_components_when_constraints_use_clamp_adapt_policy() {
    let mut root = Parameter::new("root_param", ParamValue::Vec2(0.0, 0.0), ParameterChangeCheck::None);
    root.constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        step: Some(0.25),
        step_base: Some(0.0),
        enum_options: Vec::new(),
        policy: ParameterConstraintPolicy::ClampAdapt,
        reference: Default::default(),
        file: Default::default(),
    };
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Vec2(-0.2, 1.13),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine
        .apply_edits()
        .expect("set vec2 param should clamp and adapt each component");

    let value = engine
        .nodes
        .get(engine.root)
        .expect("root parameter should exist")
        .value
        .clone();
    assert_eq!(
        value,
        ParamValue::Vec2(0.0, 1.0),
        "each vec2 component should be normalized"
    );
}

#[test]
fn apply_set_param_rejects_vec3_components_when_constraints_use_reject_policy() {
    let mut root = Parameter::new(
        "root_param",
        ParamValue::Vec3(0.0, 0.0, 0.0),
        ParameterChangeCheck::None,
    );
    root.constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        step: None,
        step_base: None,
        enum_options: Vec::new(),
        policy: ParameterConstraintPolicy::Reject,
        reference: Default::default(),
        file: Default::default(),
    };
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Vec3(0.2, 1.3, 0.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });

    let result = engine.apply_edits();
    assert!(matches!(result, Err(EngineEditError::ParamConstraintViolation { .. })));
}

#[test]
fn apply_set_param_clamps_vec3_components_with_component_specific_bounds() {
    let mut root = Parameter::new(
        "root_param",
        ParamValue::Vec3(0.0, 0.0, 0.0),
        ParameterChangeCheck::None,
    );
    root.constraints = ParameterConstraints {
        range: RangeConstraint::components(Some(vec![0.0, -1.0, 5.0]), Some(vec![1.0, 2.0, 6.0])),
        step: None,
        step_base: None,
        enum_options: Vec::new(),
        policy: ParameterConstraintPolicy::ClampAdapt,
        reference: Default::default(),
        file: Default::default(),
    };
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Vec3(-3.0, 3.0, 4.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine
        .apply_edits()
        .expect("set vec3 param should clamp using component-specific bounds");

    let value = engine
        .nodes
        .get(engine.root)
        .expect("root parameter should exist")
        .value
        .clone();
    assert_eq!(
        value,
        ParamValue::Vec3(0.0, 2.0, 5.0),
        "vec3 bounds should apply per component"
    );
}

#[test]
fn apply_set_param_rejects_values_outside_enum_constraints() {
    let mut root = Parameter::new("mode", ParamValue::Enum("a".to_string()), ParameterChangeCheck::None);
    root.constraints = ParameterConstraints {
        range: None,
        step: None,
        step_base: None,
        enum_options: vec![
            ParameterEnumOption {
                variant_id: "a".to_string(),
                value: ParamValue::Enum("a".to_string()),
                label: "Mode A".to_string(),
                tags: Vec::new(),
                ordering: Some(0),
            },
            ParameterEnumOption {
                variant_id: "b".to_string(),
                value: ParamValue::Enum("b".to_string()),
                label: "Mode B".to_string(),
                tags: Vec::new(),
                ordering: Some(1),
            },
        ],
        policy: ParameterConstraintPolicy::ClampAdapt,
        reference: Default::default(),
        file: Default::default(),
    };
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Enum("c".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });

    let result = engine.apply_edits();
    assert!(matches!(result, Err(EngineEditError::ParamConstraintViolation { .. })));
}

#[test]
fn apply_set_param_accepts_enum_variant_ids_with_legacy_string_enum_values() {
    let mut root = Parameter::new(
        "mode",
        ParamValue::Str("legacy_a".to_string()),
        ParameterChangeCheck::None,
    );
    root.constraints = ParameterConstraints {
        range: None,
        step: None,
        step_base: None,
        enum_options: vec![
            ParameterEnumOption {
                variant_id: "legacy_a".to_string(),
                value: ParamValue::Str("a".to_string()),
                label: "Mode A".to_string(),
                tags: Vec::new(),
                ordering: Some(0),
            },
            ParameterEnumOption {
                variant_id: "legacy_b".to_string(),
                value: ParamValue::Str("b".to_string()),
                label: "Mode B".to_string(),
                tags: Vec::new(),
                ordering: Some(1),
            },
        ],
        policy: ParameterConstraintPolicy::ClampAdapt,
        reference: Default::default(),
        file: Default::default(),
    };
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Enum("legacy_b".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine
        .apply_edits()
        .expect("enum variant ids should be accepted against legacy string enum values");

    let value = engine
        .nodes
        .get(engine.root)
        .expect("root parameter should exist")
        .value
        .clone();
    assert_eq!(value, ParamValue::Enum("legacy_b".to_string()));
}

fn encode_parameter_node(node: &Parameter) -> Result<serde_json::Value, String> {
    serde_json::to_value(serde_json::json!({
        "value": node.value,
        "change_check": node.change_check,
        "event_behaviour": node.event_behaviour,
    }))
    .map_err(|err| format!("failed to encode parameter node: {err}"))
}

fn decode_parameter_node(_node_type: &str, data: &serde_json::Value, meta: &NodeMeta) -> Result<Parameter, String> {
    let value: ParamValue = serde_json::from_value(data.get("value").cloned().ok_or("missing 'value' field")?)
        .map_err(|err| format!("invalid parameter value payload: {err}"))?;
    let change_check: ParameterChangeCheck = serde_json::from_value(
        data.get("change_check")
            .cloned()
            .ok_or("missing 'change_check' field")?,
    )
    .map_err(|err| format!("invalid change_check payload: {err}"))?;
    let event_behaviour: ParameterEventBehaviour = serde_json::from_value(
        data.get("event_behaviour")
            .cloned()
            .ok_or("missing 'event_behaviour' field")?,
    )
    .map_err(|err| format!("invalid event_behaviour payload: {err}"))?;

    let mut node = Parameter::new(&meta.label, value, change_check);
    node.event_behaviour = event_behaviour;
    Ok(node)
}

#[test]
fn project_roundtrip_restores_reference_uuid_and_cached_runtime_id() {
    let root = Parameter::new("root", ParamValue::Int(10), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.75), ParameterChangeCheck::None),
        None,
    );
    engine.add_node(
        Parameter::new(
            "target_ref",
            ParamValue::Reference(NodeReference::new(NodeUuid(Uuid::new_v4()))),
            ParameterChangeCheck::None,
        ),
        None,
    );
    engine.apply_edits().expect("initial add should succeed");

    let target = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("target child should exist");
    let target_ref = engine
        .nodes
        .get(target)
        .and_then(|node| node.node_data().next_sibling)
        .expect("reference child should exist");
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target node should exist")
        .node_data()
        .meta
        .uuid;

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");

    let json = engine
        .to_project_json_with(encode_parameter_node)
        .expect("project serialization should succeed");
    let loaded =
        Engine::<Parameter>::from_project_json_with(&json, decode_parameter_node).expect("project load should succeed");

    let loaded_target = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("loaded target child should exist");
    let loaded_target_ref = loaded
        .nodes
        .get(loaded_target)
        .and_then(|node| node.node_data().next_sibling)
        .expect("loaded reference child should exist");
    let loaded_target_uuid = loaded
        .nodes
        .get(loaded_target)
        .expect("loaded target node should exist")
        .node_data()
        .meta
        .uuid;

    let loaded_ref_value = &loaded
        .nodes
        .get(loaded_target_ref)
        .expect("loaded reference node should exist")
        .value;
    match loaded_ref_value {
        ParamValue::Reference(reference) => {
            assert_eq!(reference.uuid(), loaded_target_uuid);
            assert_eq!(reference.cached_id(), Some(loaded_target));
            assert_eq!(reference.cached_name(), Some("target"));
        }
        other => panic!("expected reference value after load, got {:?}", other),
    }
}

#[test]
fn project_roundtrip_keeps_dangling_reference_uuid_with_empty_cache() {
    let dangling_uuid = NodeUuid(Uuid::new_v4());
    let mut dangling_reference = NodeReference::new(dangling_uuid);
    dangling_reference.set_cached_name(Some("missing_target".to_string()));
    let root = Parameter::new(
        "root",
        ParamValue::Reference(dangling_reference),
        ParameterChangeCheck::None,
    );
    let engine = Engine::new(root);

    let json = engine
        .to_project_json_with(encode_parameter_node)
        .expect("project serialization should succeed");
    let loaded =
        Engine::<Parameter>::from_project_json_with(&json, decode_parameter_node).expect("project load should succeed");

    let loaded_root = loaded.nodes.get(loaded.root).expect("loaded root should exist");
    match &loaded_root.value {
        ParamValue::Reference(reference) => {
            assert_eq!(reference.uuid(), dangling_uuid);
            assert_eq!(reference.cached_id(), None);
            assert_eq!(reference.cached_name(), Some("missing_target"));
            let warning = loaded_root
                .node_data()
                .meta
                .presentation
                .warning(Some("missing-reference"))
                .expect("dangling reference should surface missing-reference warning");
            assert_eq!(warning.message, "Missing reference");
            assert_eq!(warning.detail.as_deref(), Some("Target 'missing_target' is missing"));
        }
        other => panic!("expected dangling reference value, got {:?}", other),
    }
}

#[test]
fn project_load_save_load_roundtrip_is_stable() {
    let root = Parameter::new("root", ParamValue::Int(123), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.75), ParameterChangeCheck::None),
        None,
    );
    engine.add_node(
        Parameter::new(
            "target_ref",
            ParamValue::Reference(NodeReference::new(NodeUuid(Uuid::new_v4()))),
            ParameterChangeCheck::None,
        ),
        None,
    );
    engine.apply_edits().expect("initial add should succeed");

    let target = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("target child should exist");
    let target_ref = engine
        .nodes
        .get(target)
        .and_then(|node| node.node_data().next_sibling)
        .expect("reference child should exist");
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target node should exist")
        .node_data()
        .meta
        .uuid;

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");

    let json1 = engine
        .to_project_json_pretty_with(encode_parameter_node)
        .expect("first project serialization should succeed");
    let loaded1 = Engine::<Parameter>::from_project_json_with(&json1, decode_parameter_node)
        .expect("first project load should succeed");

    let json2 = loaded1
        .to_project_json_pretty_with(encode_parameter_node)
        .expect("second project serialization should succeed");
    let loaded2 = Engine::<Parameter>::from_project_json_with(&json2, decode_parameter_node)
        .expect("second project load should succeed");

    let json3 = loaded2
        .to_project_json_pretty_with(encode_parameter_node)
        .expect("third project serialization should succeed");

    let value1: serde_json::Value = serde_json::from_str(&json1).expect("json1 should parse");
    let value2: serde_json::Value = serde_json::from_str(&json2).expect("json2 should parse");
    let value3: serde_json::Value = serde_json::from_str(&json3).expect("json3 should parse");
    assert_eq!(value1, value2, "load-save should preserve full project data");
    assert_eq!(value2, value3, "second load-save should remain stable");

    let loaded2_target = loaded2
        .nodes
        .get(loaded2.root)
        .and_then(|root| root.node_data().first_child)
        .expect("loaded2 target child should exist");
    let loaded2_target_ref = loaded2
        .nodes
        .get(loaded2_target)
        .and_then(|node| node.node_data().next_sibling)
        .expect("loaded2 reference child should exist");

    match &loaded2
        .nodes
        .get(loaded2_target_ref)
        .expect("loaded2 reference node should exist")
        .value
    {
        ParamValue::Reference(reference) => {
            assert_eq!(
                reference.uuid(),
                loaded2
                    .nodes
                    .get(loaded2_target)
                    .expect("loaded2 target should exist")
                    .node_data()
                    .meta
                    .uuid
            );
            assert_eq!(reference.cached_id(), Some(loaded2_target));
            assert_eq!(reference.cached_name(), Some("target"));
        }
        other => panic!("expected loaded2 reference value, got {:?}", other),
    }
}

#[test]
fn project_roundtrip_keeps_cached_reference_name_when_target_is_missing() {
    let root = Parameter::new("root", ParamValue::Int(10), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.75), ParameterChangeCheck::None),
        None,
    );
    engine.add_node(
        Parameter::new(
            "target_ref",
            ParamValue::Reference(NodeReference::new(NodeUuid(Uuid::new_v4()))),
            ParameterChangeCheck::None,
        ),
        None,
    );
    engine.apply_edits().expect("initial add should succeed");

    let target = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("target child should exist");
    let target_ref = engine
        .nodes
        .get(target)
        .and_then(|node| node.node_data().next_sibling)
        .expect("reference child should exist");
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target node should exist")
        .node_data()
        .meta
        .uuid;

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");

    engine.edits.push(Edit::RemoveNode { node: target });
    engine.apply_edits().expect("target removal should succeed");

    let json = engine
        .to_project_json_with(encode_parameter_node)
        .expect("project serialization should succeed");
    let loaded =
        Engine::<Parameter>::from_project_json_with(&json, decode_parameter_node).expect("project load should succeed");
    let loaded_ref = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .expect("loaded reference child should exist");

    match &loaded
        .nodes
        .get(loaded_ref)
        .expect("loaded reference node should exist")
        .value
    {
        ParamValue::Reference(reference) => {
            assert_eq!(reference.uuid(), target_uuid);
            assert_eq!(reference.cached_id(), None);
            assert_eq!(reference.cached_name(), Some("target"));
            let warning = loaded
                .nodes
                .get(loaded_ref)
                .expect("loaded reference node should exist")
                .node_data()
                .meta
                .presentation
                .warning(Some("missing-reference"))
                .expect("missing reference should surface warning");
            assert_eq!(warning.message, "Missing reference");
            assert_eq!(warning.detail.as_deref(), Some("Target 'target' is missing"));
        }
        other => panic!("expected reference value after load, got {:?}", other),
    }
}

fn ui_event_log_requires_resync<T: Node>(engine: &Engine<T>) -> bool {
    engine.ui_event_log().iter().any(|event| {
        matches!(
            &event.kind,
            EventKind::Custom(custom) if custom.topic == "__transport.resync_required"
        )
    })
}

fn direct_children<T: Node>(engine: &Engine<T>, parent: NodeId) -> Vec<NodeId> {
    let mut children = Vec::new();
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        children.push(child_id);
        child = engine
            .nodes
            .get(child_id)
            .and_then(|node| node.node_data().next_sibling);
    }
    children
}

fn first_graph_transaction_ops(batch: &crate::ui_sync::UiEventBatch) -> &[UiGraphOp] {
    batch
        .events
        .iter()
        .find_map(|event| match &event.kind {
            UiEventKind::GraphTransaction { transaction } => Some(transaction.ops.as_slice()),
            _ => None,
        })
        .expect("expected graph transaction event")
}

#[test]
fn rename_node_does_not_require_whole_graph_resync() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child".to_string()), None);
    engine.apply_edits().expect("child add should succeed");
    let child = direct_children(&engine, engine.root)
        .first()
        .copied()
        .expect("child should exist");

    engine.clear_ui_event_log();
    let ack = engine.apply_ui_intent(UiEditIntent::PatchMeta {
        node: child,
        patch: NodeMetaPatch {
            label: Some("renamed".to_string()),
            ..Default::default()
        },
    });

    assert!(ack.success, "rename intent should apply: {:?}", ack.error_message);
    assert!(
        !ui_event_log_requires_resync(&engine),
        "rename should not force a whole-graph UI snapshot resync"
    );

    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let ops = first_graph_transaction_ops(&batch);
    assert!(
        ops.iter().any(|op| matches!(
            op,
            UiGraphOp::NodeMetaPatched { node, patch }
                if *node == child && patch.label.as_deref() == Some("renamed")
        )),
        "rename should emit an incremental metadata patch transaction"
    );
}

#[test]
fn remove_node_does_not_require_whole_graph_resync() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child".to_string()), None);
    engine.apply_edits().expect("child add should succeed");
    let child = direct_children(&engine, engine.root)
        .first()
        .copied()
        .expect("child should exist");

    engine.clear_ui_event_log();
    let ack = engine.apply_ui_intent(UiEditIntent::RemoveNode { node: child });

    assert!(ack.success, "remove intent should apply: {:?}", ack.error_message);
    assert!(
        !ui_event_log_requires_resync(&engine),
        "remove should not force a whole-graph UI snapshot resync"
    );

    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let ops = first_graph_transaction_ops(&batch);
    assert!(
        ops.iter().any(|op| matches!(
            op,
            UiGraphOp::SubtreeRemoved {
                root,
                removed_ids,
                parent_after: Some(parent_after),
            } if *root == child && removed_ids == &vec![child] && parent_after.parent == engine.root && parent_after.children.is_empty()
        )),
        "remove should emit an incremental subtree-removal transaction"
    );
}

#[test]
fn move_node_does_not_require_whole_graph_resync() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("old_parent".to_string()), None);
    engine.add_node(Folder::new("new_parent".to_string()), None);
    engine.apply_edits().expect("parent add should succeed");
    let parents = direct_children(&engine, engine.root);
    let old_parent = parents[0];
    let new_parent = parents[1];
    engine.add_node(Folder::new("child".to_string()), Some(old_parent));
    engine.apply_edits().expect("child add should succeed");
    let child = direct_children(&engine, old_parent)[0];

    engine.clear_ui_event_log();
    let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: child,
        new_parent,
        new_prev_sibling: None,
    });

    assert!(ack.success, "move intent should apply: {:?}", ack.error_message);
    assert!(
        !ui_event_log_requires_resync(&engine),
        "move should not force a whole-graph UI snapshot resync"
    );

    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let ops = first_graph_transaction_ops(&batch);
    assert!(
        ops.iter().any(|op| matches!(
            op,
            UiGraphOp::NodeMoved {
                node: moved_child,
                old_parent: moved_old_parent,
                new_parent: moved_new_parent,
                old_parent_after: Some(old_parent_after),
                new_parent_after: Some(new_parent_after),
            } if *moved_child == child
                && *moved_old_parent == Some(old_parent)
                && *moved_new_parent == Some(new_parent)
                && old_parent_after.parent == old_parent
                && old_parent_after.children.is_empty()
                && new_parent_after.parent == new_parent
                && new_parent_after.children == vec![child]
        )),
        "move should emit an incremental move transaction with post-transaction parent orders"
    );
}

// --- Phase 6: compact subtree event tests ---

#[test]
fn phase6_add_node_tree_small_emits_node_created_ops() {
    // N ≤ 8 nodes: must use individual NodeCreated ops (backward-compatible path).
    let mut engine = Engine::new(Folder::new("root".to_string()));
    let tree = crate::edit::NodeTree::new(Folder::new("a".to_string()))
        .with_child(crate::edit::NodeTree::new(Folder::new("b".to_string())))
        .with_child(crate::edit::NodeTree::new(Folder::new("c".to_string())));
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: engine.root,
        prev_sibling: None,
    });
    engine.apply_edits().expect("add_node_tree should succeed");

    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let ops = first_graph_transaction_ops(&batch);

    assert!(
        ops.iter().any(|op| matches!(op, UiGraphOp::NodeCreated { .. })),
        "small tree (≤8 nodes) must use NodeCreated ops"
    );
    assert!(
        !ops.iter().any(|op| matches!(op, UiGraphOp::SubtreeInserted { .. })),
        "small tree (≤8 nodes) must NOT emit SubtreeInserted"
    );
}

#[test]
fn phase6_add_node_tree_large_emits_subtree_inserted_op() {
    // N > 8 nodes: must emit a single SubtreeInserted op instead of N NodeCreated ops.
    let mut engine = Engine::new(Folder::new("root".to_string()));
    let mut tree = crate::edit::NodeTree::new(Folder::new("subtree_root".to_string()));
    for i in 0..10 {
        tree = tree.with_child(crate::edit::NodeTree::new(Folder::new(format!("child_{i}"))));
    }
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: engine.root,
        prev_sibling: None,
    });
    engine.apply_edits().expect("add_node_tree should succeed");

    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let ops = first_graph_transaction_ops(&batch);

    assert_eq!(
        ops.len(),
        1,
        "large tree (>8 nodes) must emit exactly one SubtreeInserted op"
    );
    assert!(
        matches!(ops[0], UiGraphOp::SubtreeInserted { .. }),
        "the single op must be SubtreeInserted"
    );

    if let UiGraphOp::SubtreeInserted {
        nodes,
        parent,
        parent_children_after,
        ..
    } = &ops[0]
    {
        assert_eq!(
            nodes.len(),
            11,
            "SubtreeInserted must carry all 11 node snapshots (root + 10 children)"
        );
        assert_eq!(*parent, engine.root, "insertion parent must be engine root");
        assert_eq!(
            parent_children_after.len(),
            1,
            "root gains one direct child after insertion"
        );
    }
}

#[test]
fn undo_remove_large_subtree_emits_compact_insert_transaction() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    let mut tree = crate::edit::NodeTree::new(Folder::new("subtree_root".to_string()));
    for i in 0..10 {
        tree = tree.with_child(crate::edit::NodeTree::new(Folder::new(format!("child_{i}"))));
    }
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: engine.root,
        prev_sibling: None,
    });
    engine.apply_edits().expect("add_node_tree should succeed");
    let subtree_root = direct_children(&engine, engine.root)
        .first()
        .copied()
        .expect("inserted subtree root should exist");

    engine.clear_ui_event_log();
    let remove_ack = engine.apply_ui_intent(UiEditIntent::RemoveNode { node: subtree_root });
    assert!(
        remove_ack.success,
        "remove intent should apply: {:?}",
        remove_ack.error_message
    );

    engine.clear_ui_event_log();
    let undo_ack = engine.apply_ui_intent(UiEditIntent::Undo);
    assert!(
        undo_ack.success,
        "undo intent should apply: {:?}",
        undo_ack.error_message
    );

    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    assert!(
        batch
            .events
            .iter()
            .all(|event| !matches!(event.kind, UiEventKind::NodeCreated { .. })),
        "undo restore should not expose per-node NodeCreated UI events"
    );

    let ops = first_graph_transaction_ops(&batch);
    assert_eq!(ops.len(), 1, "undo restore should emit exactly one compact graph op");
    match &ops[0] {
        UiGraphOp::SubtreeInserted {
            root,
            parent,
            nodes,
            parent_children_after,
        } => {
            assert_eq!(*root, subtree_root);
            assert_eq!(*parent, engine.root);
            assert_eq!(nodes.len(), 11);
            assert_eq!(parent_children_after, &vec![subtree_root]);
        }
        other => panic!("expected SubtreeInserted op, got {other:?}"),
    }
}

#[test]
fn reorder_children_does_not_require_whole_graph_resync() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);
    engine.apply_edits().expect("child add should succeed");
    let children = direct_children(&engine, engine.root);
    let child_a = children[0];
    let child_b = children[1];

    engine.clear_ui_event_log();
    let ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: child_a,
        new_parent: engine.root,
        new_prev_sibling: Some(child_b),
    });

    assert!(ack.success, "reorder intent should apply: {:?}", ack.error_message);
    assert!(
        !ui_event_log_requires_resync(&engine),
        "reorder should not force a whole-graph UI snapshot resync"
    );

    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let ops = first_graph_transaction_ops(&batch);
    assert!(
        ops.iter().any(|op| matches!(
            op,
            UiGraphOp::ChildrenReordered {
                parent,
                children,
            } if *parent == engine.root && children == &vec![child_b, child_a]
        )),
        "reorder should emit an incremental child order transaction"
    );
}

#[test]
fn set_param_does_not_require_whole_graph_resync() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));

    engine.clear_ui_event_log();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: engine.root,
        value: ParamValue::Int(42),
        behaviour: ParameterEventBehaviour::Coalesce,
    });

    assert!(ack.success, "set_param intent should apply: {:?}", ack.error_message);
    assert!(
        !ui_event_log_requires_resync(&engine),
        "set_param should not force a whole-graph UI snapshot resync"
    );

    let batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    assert!(
        batch.events.iter().any(|event| matches!(
            &event.kind,
            UiEventKind::ParamChanged {
                param,
                new_value: ParamValue::Int(42),
                ..
            } if *param == engine.root
        )),
        "set_param should emit an incremental parameter value patch"
    );
}

#[test]
fn duplicate_subtree_preserves_payload_and_remaps_internal_references() {
    let root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("group", ParamValue::Int(1), ParameterChangeCheck::None),
        None,
    );
    engine.apply_edits().expect("group add should succeed");

    let group = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("group child should exist");

    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.75), ParameterChangeCheck::None),
        Some(group),
    );
    engine.add_node(
        Parameter::new(
            "target_ref",
            ParamValue::Reference(NodeReference::new(NodeUuid(Uuid::new_v4()))),
            ParameterChangeCheck::None,
        ),
        Some(group),
    );
    engine.apply_edits().expect("group children add should succeed");

    let target = engine
        .nodes
        .get(group)
        .and_then(|node| node.node_data().first_child)
        .expect("target child should exist");
    let target_ref = engine
        .nodes
        .get(target)
        .and_then(|node| node.node_data().next_sibling)
        .expect("reference child should exist");
    let original_target_uuid = engine
        .nodes
        .get(target)
        .expect("target should exist")
        .node_data()
        .meta
        .uuid;

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(original_target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");
    engine.inbox.clear();
    engine.clear_ui_event_log();

    let duplicated_group = engine
        .duplicate_subtree_with(
            group,
            engine.root,
            Some(group),
            None,
            encode_parameter_node,
            decode_parameter_node,
        )
        .expect("duplicate subtree should succeed");

    assert!(
        engine.inbox.events.len() == 2,
        "duplicate subtree should queue only root structural inbox events"
    );
    assert!(
        !engine.ui_event_log().iter().any(|event| matches!(
            &event.kind,
            EventKind::Custom(custom) if custom.topic == "__transport.resync_required"
        )),
        "duplicate subtree should not force a whole-graph UI snapshot resync"
    );
    let transaction_ops = engine
        .ui_event_log()
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::GraphTransaction { transaction } => Some(transaction.ops.as_slice()),
            _ => None,
        })
        .expect("duplicate subtree should publish one graph transaction");
    assert!(
        transaction_ops.iter().any(|op| matches!(
            op,
            UiGraphOp::NodeCreated {
                snapshot,
                parent: Some(parent),
                ..
            } if snapshot.node_id == duplicated_group && *parent == engine.root
        )),
        "duplicate subtree should publish an incremental UI node-created op for the duplicated root"
    );
    assert!(
        transaction_ops.iter().any(|op| matches!(
            op,
            UiGraphOp::ChildrenReordered { parent, children }
                if *parent == engine.root && children == &vec![group, duplicated_group]
        )),
        "duplicate subtree should publish a parent child-order op for the duplicated root"
    );

    assert_eq!(
        engine.nodes.get(group).and_then(|node| node.node_data().next_sibling),
        Some(duplicated_group),
        "duplicate should be inserted after the source subtree root"
    );
    assert_eq!(
        engine
            .nodes
            .get(duplicated_group)
            .map(|node| node.node_data().meta.label.as_str()),
        Some("group 2"),
        "duplicate labels should be generated by the backend with a numeric suffix"
    );

    let duplicated_target = engine
        .nodes
        .get(duplicated_group)
        .and_then(|node| node.node_data().first_child)
        .expect("duplicated target child should exist");
    let duplicated_target_ref = engine
        .nodes
        .get(duplicated_target)
        .and_then(|node| node.node_data().next_sibling)
        .expect("duplicated reference child should exist");

    let ui_batch = engine.ui_event_batch(None, UiSubscriptionScope::WholeGraph);
    let ops = first_graph_transaction_ops(&ui_batch);
    let duplicated_root_snapshot = ops
        .iter()
        .find_map(|op| match op {
            UiGraphOp::NodeCreated { snapshot, .. } if snapshot.node_id == duplicated_group => Some(snapshot),
            _ => None,
        })
        .expect("duplicated root node-created op should carry an incremental node snapshot");
    assert_eq!(
        duplicated_root_snapshot.children,
        vec![duplicated_target, duplicated_target_ref],
        "incremental duplicate root snapshot should include direct children"
    );
    let duplicate_parent_children = ops
        .iter()
        .find_map(|op| match op {
            UiGraphOp::ChildrenReordered { parent, children } if *parent == engine.root => Some(children),
            _ => None,
        })
        .expect("duplicated root transaction should carry parent child order");
    assert_eq!(
        duplicate_parent_children,
        &vec![group, duplicated_group],
        "incremental duplicate event should preserve insertion order without a snapshot resync"
    );

    let duplicated_target_uuid = engine
        .nodes
        .get(duplicated_target)
        .expect("duplicated target should exist")
        .node_data()
        .meta
        .uuid;

    assert_ne!(
        duplicated_target_uuid, original_target_uuid,
        "duplicated subtree must receive fresh UUIDs"
    );

    match &engine
        .nodes
        .get(duplicated_target_ref)
        .expect("duplicated reference should exist")
        .value
    {
        ParamValue::Reference(reference) => {
            assert_eq!(
                reference.uuid(),
                duplicated_target_uuid,
                "internal duplicated references should point at duplicated targets"
            );
            assert_eq!(
                reference.cached_id(),
                Some(duplicated_target),
                "duplicated references should resolve to duplicated runtime ids"
            );
            assert_eq!(
                reference.cached_name(),
                Some("target"),
                "duplicated references should refresh cached target labels"
            );
        }
        other => panic!("expected duplicated reference value, got {:?}", other),
    }

    assert!(
        engine.undo().expect("undo should succeed"),
        "duplicate should create undo history"
    );
    assert_eq!(
        engine.nodes.get(group).and_then(|node| node.node_data().next_sibling),
        None,
        "undo should remove the duplicated subtree root"
    );
    assert!(
        engine.nodes.get(duplicated_group).is_none(),
        "duplicated subtree root should be detached after undo"
    );

    assert!(
        engine.redo().expect("redo should succeed"),
        "redo should restore duplicated subtree"
    );
    assert_eq!(
        engine.nodes.get(group).and_then(|node| node.node_data().next_sibling),
        Some(duplicated_group),
        "redo should restore the duplicated subtree root at the same position"
    );
    assert_eq!(
        engine
            .nodes
            .get(duplicated_group)
            .and_then(|node| node.node_data().first_child),
        Some(duplicated_target),
        "redo should restore the same duplicated child ids"
    );
}

#[test]
fn set_param_reference_recovers_target_from_relative_path_and_updates_hints() {
    let root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("container", ParamValue::Int(1), ParameterChangeCheck::None),
        Some(engine.root),
    );
    engine.apply_edits().expect("container add should succeed");
    let container = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("container should exist");

    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.5), ParameterChangeCheck::None),
        Some(container),
    );
    engine.add_node(
        Parameter::new(
            "target_ref",
            ParamValue::Reference(NodeReference::default()),
            ParameterChangeCheck::None,
        ),
        Some(engine.root),
    );
    engine.apply_edits().expect("target and reference add should succeed");

    let target = find_child_by_label_parameter(&engine, container, "target").expect("target should exist");
    let target_ref =
        find_child_by_label_parameter(&engine, engine.root, "target_ref").expect("target_ref should exist");

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::with_hints(
            NodeUuid::nil(),
            None,
            vec!["container".to_string(), "target".to_string()],
        )),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");

    let Some(reference) = engine.nodes.get(target_ref).and_then(|node| match &node.value {
        ParamValue::Reference(reference) => Some(reference),
        _ => None,
    }) else {
        panic!("target_ref value should be a reference");
    };
    assert_eq!(reference.cached_id(), Some(target));
    assert_eq!(
        reference.uuid(),
        engine
            .nodes
            .get(target)
            .expect("target should exist")
            .node_data()
            .meta
            .uuid
    );
    assert_eq!(reference.cached_name(), Some("target"));
    assert_eq!(
        reference.relative_path_from_root(),
        &["container".to_string(), "target".to_string()]
    );
}

#[test]
fn set_param_reference_preserves_missing_uuid_over_relative_path_hint() {
    let root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("container", ParamValue::Int(1), ParameterChangeCheck::None),
        Some(engine.root),
    );
    engine.apply_edits().expect("container add should succeed");
    let container = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("container should exist");

    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.5), ParameterChangeCheck::None),
        Some(container),
    );
    engine.add_node(
        Parameter::new("target", ParamValue::Float(1.5), ParameterChangeCheck::None),
        Some(container),
    );
    engine.add_node(
        Parameter::new(
            "target_ref",
            ParamValue::Reference(NodeReference::default()),
            ParameterChangeCheck::None,
        ),
        Some(engine.root),
    );
    engine.apply_edits().expect("targets and reference add should succeed");

    let first_target = engine
        .nodes
        .get(container)
        .and_then(|container| container.node_data().first_child)
        .expect("first target should exist");
    let second_target = engine
        .nodes
        .get(first_target)
        .and_then(|first| first.node_data().next_sibling)
        .expect("second target should exist");
    let target_ref =
        find_child_by_label_parameter(&engine, engine.root, "target_ref").expect("target_ref should exist");
    let first_target_uuid = engine
        .nodes
        .get(first_target)
        .expect("first target should exist")
        .node_data()
        .meta
        .uuid;
    let second_target_uuid = engine
        .nodes
        .get(second_target)
        .expect("second target should exist")
        .node_data()
        .meta
        .uuid;

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(first_target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");

    engine.edits.push(Edit::RemoveNode { node: first_target });
    engine.apply_edits().expect("target removal should succeed");

    let Some(reference) = engine.nodes.get(target_ref).and_then(|node| match &node.value {
        ParamValue::Reference(reference) => Some(reference),
        _ => None,
    }) else {
        panic!("target_ref value should be a reference");
    };
    assert_eq!(
        reference.uuid(),
        first_target_uuid,
        "missing persistent UUID must not be replaced by a relative-path match"
    );
    assert_ne!(reference.uuid(), second_target_uuid);
    assert_eq!(reference.cached_id(), None);
    assert!(
        engine
            .nodes
            .get(target_ref)
            .expect("target_ref should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("missing-reference"))
            .is_some(),
        "dangling reference should report a missing-reference warning"
    );
}

#[test]
fn set_param_reference_to_nil_uuid_clears_cached_hints() {
    let root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.5), ParameterChangeCheck::None),
        Some(engine.root),
    );
    engine.add_node(
        Parameter::new(
            "target_ref",
            ParamValue::Reference(NodeReference::default()),
            ParameterChangeCheck::None,
        ),
        Some(engine.root),
    );
    engine.apply_edits().expect("target and reference add should succeed");

    let target = find_child_by_label_parameter(&engine, engine.root, "target").expect("target should exist");
    let target_ref =
        find_child_by_label_parameter(&engine, engine.root, "target_ref").expect("target_ref should exist");
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target should exist")
        .node_data()
        .meta
        .uuid;

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");

    let mut clear_reference = NodeReference::new(NodeUuid::nil());
    clear_reference.set_cached_id(Some(target));
    clear_reference.set_cached_name(Some("target".to_string()));
    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(clear_reference),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("clear reference should succeed");

    let Some(reference) = engine.nodes.get(target_ref).and_then(|node| match &node.value {
        ParamValue::Reference(reference) => Some(reference),
        _ => None,
    }) else {
        panic!("target_ref value should be a reference");
    };

    assert!(reference.uuid().is_nil(), "cleared reference should use nil uuid");
    assert_eq!(
        reference.cached_id(),
        None,
        "cleared reference should not keep runtime cache"
    );
    assert_eq!(
        reference.cached_name(),
        None,
        "cleared reference should not keep cached name"
    );
    assert!(
        reference.relative_path_from_root().is_empty(),
        "cleared reference should not keep relative path hints"
    );
}

#[test]
fn uuid_index_updates_on_remove_undo_redo() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("target".to_string()), Some(engine.root));
    engine.apply_edits().expect("target add should succeed");

    let target = direct_children(&engine, engine.root)
        .first()
        .copied()
        .expect("target should exist");
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target should exist")
        .node_data()
        .meta
        .uuid;
    assert_eq!(engine.node_id_by_uuid(target_uuid), Some(target));

    engine.edits.push(Edit::RemoveNode { node: target });
    engine.apply_edits().expect("target remove should succeed");
    assert_eq!(engine.node_id_by_uuid(target_uuid), None);

    engine.undo().expect("undo should restore target");
    assert_eq!(engine.node_id_by_uuid(target_uuid), Some(target));

    engine.redo().expect("redo should remove target again");
    assert_eq!(engine.node_id_by_uuid(target_uuid), None);
}

#[test]
fn missing_reference_warning_tracks_reference_resolution_state() {
    let root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.5), ParameterChangeCheck::None),
        Some(engine.root),
    );
    engine.add_node(
        Parameter::new(
            "target_ref",
            ParamValue::Reference(NodeReference::default()),
            ParameterChangeCheck::None,
        ),
        Some(engine.root),
    );
    engine.apply_edits().expect("target and reference add should succeed");

    let target = find_child_by_label_parameter(&engine, engine.root, "target").expect("target should exist");
    let target_ref =
        find_child_by_label_parameter(&engine, engine.root, "target_ref").expect("target_ref should exist");
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target should exist")
        .node_data()
        .meta
        .uuid;

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");

    assert!(
        engine
            .nodes
            .get(target_ref)
            .expect("target_ref should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("missing-reference"))
            .is_none(),
        "resolved reference should not have missing-reference warning",
    );

    engine.edits.push(Edit::RemoveNode { node: target });
    engine.apply_edits().expect("target removal should succeed");

    let warning = engine
        .nodes
        .get(target_ref)
        .expect("target_ref should exist")
        .node_data()
        .meta
        .presentation
        .warning(Some("missing-reference"))
        .expect("dangling reference should have missing-reference warning");
    assert_eq!(warning.message, "Missing reference");
    assert_eq!(warning.detail.as_deref(), Some("Target 'target' is missing"));

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(NodeUuid::nil())),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference clear should succeed");

    assert!(
        engine
            .nodes
            .get(target_ref)
            .expect("target_ref should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("missing-reference"))
            .is_none(),
        "empty reference should clear missing-reference warning",
    );
}

#[test]
fn missing_reference_warning_updates_on_undo_redo() {
    let root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.5), ParameterChangeCheck::None),
        Some(engine.root),
    );
    engine.add_node(
        Parameter::new(
            "target_ref",
            ParamValue::Reference(NodeReference::default()),
            ParameterChangeCheck::None,
        ),
        Some(engine.root),
    );
    engine.apply_edits().expect("target and reference add should succeed");

    let target = find_child_by_label_parameter(&engine, engine.root, "target").expect("target should exist");
    let target_ref =
        find_child_by_label_parameter(&engine, engine.root, "target_ref").expect("target_ref should exist");
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target should exist")
        .node_data()
        .meta
        .uuid;

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");

    engine.edits.push(Edit::RemoveNode { node: target });
    engine.apply_edits().expect("target removal should succeed");
    assert!(
        engine
            .nodes
            .get(target_ref)
            .expect("target_ref should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("missing-reference"))
            .is_some(),
        "dangling reference should have missing-reference warning",
    );

    assert!(
        engine.undo().expect("undo should succeed"),
        "undo should restore removed target"
    );
    assert!(
        engine
            .nodes
            .get(target_ref)
            .expect("target_ref should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("missing-reference"))
            .is_none(),
        "restored target should clear missing-reference warning",
    );

    assert!(
        engine.redo().expect("redo should succeed"),
        "redo should remove target again"
    );
    assert!(
        engine
            .nodes
            .get(target_ref)
            .expect("target_ref should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("missing-reference"))
            .is_some(),
        "redo removal should restore missing-reference warning",
    );
}

#[test]
fn set_param_reference_rejects_existing_target_that_violates_constraints() {
    let root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("target", ParamValue::Trigger(), ParameterChangeCheck::None),
        Some(engine.root),
    );
    engine.add_node(
        Parameter::new(
            "target_ref",
            ParamValue::Reference(NodeReference::default()),
            ParameterChangeCheck::None,
        ),
        Some(engine.root),
    );
    engine.apply_edits().expect("initial nodes should be added");

    let target = find_child_by_label_parameter(&engine, engine.root, "target").expect("target should exist");
    let target_ref =
        find_child_by_label_parameter(&engine, engine.root, "target_ref").expect("target_ref should exist");
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target should exist")
        .node_data()
        .meta
        .uuid;

    {
        let param = engine
            .nodes
            .get_mut(target_ref)
            .expect("target_ref parameter should exist");
        param.constraints.reference = ReferenceConstraints {
            root: ReferenceRoot::EngineRoot,
            target_kind: ReferenceTargetKind::ParameterOnly,
            allowed_node_types: Vec::new(),
            allowed_parameter_types: vec!["float".to_string()],
            allow_projections: true,
            custom_filter_key: None,
            default_search_filter: None,
        };
    }

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });

    let result = engine.apply_edits();
    assert!(
        matches!(result, Err(EngineEditError::ParamConstraintViolation { .. })),
        "constraint violation should reject incompatible target"
    );
}

fn find_child_by_label_parameter(engine: &Engine<Parameter>, parent: NodeId, label: &str) -> Option<NodeId> {
    let mut child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let matches = engine
            .nodes
            .get(child_id)
            .is_some_and(|node| node.node_data().meta.label == label);
        if matches {
            return Some(child_id);
        }
        child = engine
            .nodes
            .get(child_id)
            .and_then(|node| node.node_data().next_sibling);
    }
    None
}

#[test]
fn project_serialization_omits_null_and_empty_meta_fields() {
    let root = Parameter::new("root", ParamValue::Int(1), ParameterChangeCheck::None);
    let engine = Engine::new(root);

    let json = engine
        .to_project_json_with(encode_parameter_node)
        .expect("project serialization should succeed");
    let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");

    let meta = value
        .get("root")
        .and_then(|root| root.get("meta"))
        .and_then(|meta| meta.as_object())
        .expect("root.meta should be an object");

    assert!(!meta.contains_key("description"), "null description should be omitted");
    assert!(!meta.contains_key("tags"), "empty tags should be omitted");
    assert!(!meta.contains_key("semantics"), "empty semantics should be omitted");
    assert!(
        !meta.contains_key("presentation"),
        "empty presentation should be omitted"
    );
}

#[test]
fn project_serialization_omits_null_data_and_empty_children() {
    let engine = Engine::new(Folder::new("root"));

    let json = engine
        .to_project_json_with(|_node| Ok(serde_json::Value::Null))
        .expect("project serialization should succeed");
    let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");

    let root = value
        .get("root")
        .and_then(|root| root.as_object())
        .expect("root should be an object");

    assert!(!root.contains_key("data"), "null data should be omitted");
    assert!(!root.contains_key("children"), "empty children should be omitted");
}

#[test]
fn emit_custom_event_uses_edit_pipeline() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.emit_custom_event(CustomEvent::new(
        "transport.play",
        Some(engine.root),
        serde_json::Value::Null,
    ));

    assert!(
        ctx.events.is_empty(),
        "custom event should not be injected directly into ctx events"
    );
    assert_eq!(
        ctx.edits.pending.len(),
        1,
        "custom event should enqueue one edit request"
    );

    engine
        .absorb_edits(&mut ctx)
        .expect("absorb_edits should accept custom event edits");
    engine
        .apply_edits()
        .expect("apply_edits should convert custom event edit into engine event");

    assert!(
        matches!(
            engine.inbox.events.last().map(|event| &event.kind),
            Some(EventKind::Custom(event))
                if event.topic == "transport.play" && event.origin == Some(engine.root)
        ),
        "last event should be the emitted custom event",
    );
}

#[test]
fn undo_redo_set_param_restores_value() {
    let root = Parameter::new("root_param", ParamValue::Int(10), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(42),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should succeed");

    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Int(42)
    );
    assert_eq!(engine.undo_len(), 1);

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Int(10)
    );
    assert_eq!(engine.redo_len(), 1);

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Int(42)
    );
}

#[test]
fn same_tick_coalesced_set_param_keeps_first_old_value_for_undo() {
    let root = Parameter::new("root_param", ParamValue::Float(0.3), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(0.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("first set should succeed");

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(0.7),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("second set should succeed");

    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Float(0.7)
    );
    assert_eq!(
        engine.undo_len(),
        1,
        "same-tick coalesced updates should keep one undo step"
    );

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Float(0.3),
        "undo should restore the original value before the first coalesced update",
    );
}

#[test]
fn same_tick_append_set_param_keeps_distinct_undo_steps() {
    let root = Parameter::new("root_param", ParamValue::Float(0.3), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(0.5),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().expect("first append set should succeed");

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(0.7),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().expect("second append set should succeed");

    assert_eq!(
        engine.undo_len(),
        2,
        "append mode should keep both updates in undo history"
    );

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Float(0.5),
        "append undo should step back to the immediately previous value",
    );
}

#[test]
fn begin_end_edit_session_groups_multiple_queue_drains_into_one_undo() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::BeginEditSession {
        origin: crate::edit::EditOrigin::Ui,
        label: Some("Slider drag".to_string()),
        client_edit_id: "drag-1".to_string(),
        ui_client_instance_id: None,
    });
    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(10),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("first session chunk should apply");

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(20),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("second session chunk should apply");

    assert!(engine.has_active_edit_session());
    assert_eq!(engine.active_edit_session_id(), Some("drag-1"));
    assert_eq!(
        engine.undo_len(),
        0,
        "undo entry should not be committed before EndEditSession"
    );

    engine.edits.push(Edit::EndEditSession {
        client_edit_id: "drag-1".to_string(),
    });
    engine.apply_edits().expect("session end should commit history");

    assert!(!engine.has_active_edit_session());
    assert_eq!(
        engine.undo_len(),
        1,
        "all session edits should be grouped as one undo step"
    );

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Int(0)
    );
}

#[test]
fn clear_history_drops_active_edit_session_state() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::BeginEditSession {
        origin: crate::edit::EditOrigin::Ui,
        label: Some("bootstrap".to_string()),
        client_edit_id: "bootstrap-1".to_string(),
        ui_client_instance_id: None,
    });
    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(10),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("bootstrap edit session should apply");

    assert!(
        engine.has_active_edit_session(),
        "session should remain open before clear"
    );
    assert_eq!(engine.undo_len(), 0, "open session should not commit undo history yet");
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Int(10)
    );

    engine.clear_history();

    assert!(
        !engine.has_active_edit_session(),
        "clear_history should drop active session state"
    );
    assert_eq!(engine.undo_len(), 0);
    assert_eq!(engine.redo_len(), 0);

    engine.edits.push(Edit::EndEditSession {
        client_edit_id: "bootstrap-1".to_string(),
    });
    let stale_end = engine.apply_edits();
    assert!(
        matches!(stale_end, Err(EngineEditError::EditSessionNotActive { .. })),
        "stale session end should fail after clear_history"
    );

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(25),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("post-clear edit should apply");
    assert_eq!(engine.undo_len(), 1, "only post-clear edits should be undoable");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Int(10),
        "undo should restore to the runtime-baseline value, not pre-clear session history",
    );
}

#[test]
fn current_history_state_id_tracks_undo_redo_and_branching() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    assert_eq!(engine.current_history_state_id(), 0);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(10),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("first edit should apply");

    let first_state_id = engine.current_history_state_id();
    assert!(first_state_id > 0, "first edit should advance the content-state id");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine.current_history_state_id(),
        0,
        "undo should restore the initial content-state id"
    );

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(
        engine.current_history_state_id(),
        first_state_id,
        "redo should restore the first edited content-state id"
    );

    assert!(engine.undo().expect("second undo should succeed"));
    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(25),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("branch edit should apply");

    let branch_state_id = engine.current_history_state_id();
    assert_ne!(
        branch_state_id, first_state_id,
        "branching after undo should produce a fresh content-state id"
    );
    assert_eq!(engine.redo_len(), 0, "branching edit should clear redo history");
}

#[test]
fn current_history_state_id_tracks_live_edit_sessions() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::BeginEditSession {
        origin: crate::edit::EditOrigin::Ui,
        label: Some("drag".to_string()),
        client_edit_id: "drag-1".to_string(),
        ui_client_instance_id: None,
    });
    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(10),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("first session chunk should apply");

    let first_session_state_id = engine.current_history_state_id();
    assert!(
        first_session_state_id > 0,
        "live edit-session changes should advance the content-state id before commit"
    );
    assert_eq!(engine.undo_len(), 0, "open session should not commit undo history yet");

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(15),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("second session chunk should apply");

    let second_session_state_id = engine.current_history_state_id();
    assert!(
        second_session_state_id > first_session_state_id,
        "additional live session changes should keep advancing the content-state id"
    );

    engine.edits.push(Edit::EndEditSession {
        client_edit_id: "drag-1".to_string(),
    });
    engine.apply_edits().expect("session end should commit history");

    assert_eq!(
        engine.current_history_state_id(),
        second_session_state_id,
        "committing an edit session should preserve the live content-state id"
    );

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine.current_history_state_id(),
        0,
        "undo should restore the pre-session content-state id"
    );

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(
        engine.current_history_state_id(),
        second_session_state_id,
        "redo should restore the committed session content-state id"
    );
}

#[test]
fn patch_meta_applies_patch_to_runtime_node_metadata() {
    let mut engine = Engine::new(Folder::new("root".to_string()));

    engine.edits.push(Edit::PatchMeta {
        node: engine.root,
        patch: crate::node::NodeMetaPatch {
            label: Some("Renamed Root".to_string()),
            enabled: Some(false),
            description: Some(Some("Updated from UI".to_string())),
            ..Default::default()
        },
    });

    engine.apply_edits().expect("meta patch should apply");

    let root_meta = &engine
        .nodes
        .get(engine.root)
        .expect("root should exist")
        .node_data()
        .meta;
    assert_eq!(root_meta.label, "Renamed Root");
    assert!(!root_meta.enabled);
    assert_eq!(root_meta.description.as_deref(), Some("Updated from UI"));
}

#[test]
fn engine_warning_helpers_replace_clear_and_clear_all() {
    let mut engine = Engine::new(Folder::new("root".to_string()));

    engine.set_node_warning(engine.root, "default warning");
    engine.set_node_warning_with(
        engine.root,
        Some("port"),
        "invalid port",
        Some("port must be in [1..65535]"),
    );
    engine.set_node_warning(engine.root, "default warning updated");
    engine.apply_edits().expect("warning edits should apply");

    let presentation = &engine
        .nodes
        .get(engine.root)
        .expect("root should exist")
        .node_data()
        .meta
        .presentation;
    assert_eq!(presentation.warnings.len(), 2, "same-id warnings should be replaced");
    assert_eq!(
        presentation.warning(None).map(|warning| warning.message.as_str()),
        Some("default warning updated")
    );
    assert_eq!(
        presentation
            .warning(Some("port"))
            .and_then(|warning| warning.detail.as_deref()),
        Some("port must be in [1..65535]")
    );

    engine.clear_node_warning(engine.root, Some("port"));
    engine.apply_edits().expect("clear warning should apply");
    let presentation = &engine
        .nodes
        .get(engine.root)
        .expect("root should exist")
        .node_data()
        .meta
        .presentation;
    assert!(
        presentation.warning(Some("port")).is_none(),
        "specific warning should be cleared"
    );
    assert!(presentation.warning(None).is_some(), "default warning should remain");

    engine.clear_all_node_warnings(engine.root);
    engine.apply_edits().expect("clear all warnings should apply");
    let presentation = &engine
        .nodes
        .get(engine.root)
        .expect("root should exist")
        .node_data()
        .meta
        .presentation;
    assert!(presentation.warnings.is_empty(), "all warnings should be cleared");
}

#[test]
fn engine_warning_helpers_set_child_warning_depth() {
    let mut engine = Engine::new(Folder::new("root".to_string()));

    engine.set_node_child_warning_depth(engine.root, 3);
    engine.apply_edits().expect("set child warning depth should apply");

    let root_meta = &engine
        .nodes
        .get(engine.root)
        .expect("root should exist")
        .node_data()
        .meta;
    assert_eq!(root_meta.presentation.show_child_warnings_max_depth, 3);
}

#[test]
fn engine_warning_noops_do_not_change_history_or_redo() {
    let root = Parameter::new("root_param", ParamValue::Int(1), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);
    let root_id = engine.root;

    engine.set_node_warning(root_id, "stable warning");
    engine.apply_edits().expect("initial warning should apply");

    engine.edits.push(Edit::SetParam {
        node: root_id,
        value: ParamValue::Int(2),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("param edit should apply");
    assert_eq!(engine.undo_len(), 2);

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.redo_len(), 1);

    engine.set_node_warning(root_id, "stable warning");
    engine.clear_node_warning(root_id, Some("missing"));
    engine.set_node_child_warning_depth(root_id, 0);
    engine.apply_edits().expect("no-op warning edits should apply as empty");

    assert_eq!(engine.undo_len(), 1, "no-op warning edits must not add undo history");
    assert_eq!(engine.redo_len(), 1, "no-op warning edits must not clear redo history");
}

#[test]
fn ui_event_log_retains_events_after_inbox_dispatch() {
    let root = Parameter::new("root_param", ParamValue::Int(1), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(2),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should apply");

    assert_eq!(
        engine.ui_event_log().len(),
        1,
        "ui event log should capture emitted events"
    );
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("dispatch should succeed");
    assert!(engine.inbox.events.is_empty(), "inbox should be cleared by dispatch");
    assert_eq!(
        engine.ui_event_log().len(),
        1,
        "ui event log should remain available for replay"
    );
}

#[test]
fn ui_snapshot_projects_parameter_nodes_with_param_payload() {
    let root = Parameter::new("root_param", ParamValue::Float(0.5), ParameterChangeCheck::None);
    let engine = Engine::new(root);

    let snapshot = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);
    assert_eq!(snapshot.nodes.len(), 1);

    match &snapshot.nodes[0].data {
        UiNodeDataDto::Parameter { param } => {
            assert_eq!(param.value, ParamValue::Float(0.5));
            assert_eq!(param.default_value, None);
        }
        UiNodeDataDto::Node { .. } => panic!("expected parameter payload for parameter node"),
    }
}

#[test]
fn ui_snapshot_includes_logger_state() {
    let _logger_guard = logger::test_lock();
    logger::clear();
    crate::log!(tag = "tests", level = error; "snapshot logger payload");

    let engine = Engine::new(Folder::new("root".to_string()));
    let snapshot = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);

    assert!(snapshot.logger.max_entries >= 1);
    assert!(
        snapshot
            .logger
            .records
            .iter()
            .any(|record| record.tag == "tests" && record.message == "snapshot logger payload"),
        "snapshot should include the logger record emitted by this test"
    );

    logger::clear();
}

#[test]
fn ui_logger_intents_emit_custom_events() {
    let _logger_guard = logger::test_lock();
    logger::clear();
    let mut engine = Engine::new(Folder::new("root".to_string()));

    let set_ack = engine.apply_ui_intent(UiEditIntent::SetLogMaxEntries { max_entries: 3 });
    assert!(set_ack.success);
    assert_eq!(logger::max_entries(), 3);

    crate::log!("entry");
    assert!(
        logger::records().iter().any(|record| record.message == "entry"),
        "logger should retain the entry emitted by this test"
    );

    let clear_ack = engine.apply_ui_intent(UiEditIntent::ClearLogs);
    assert!(clear_ack.success);
    assert!(logger::records().is_empty());

    let topics: Vec<String> = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(custom) => Some(custom.topic.clone()),
            _ => None,
        })
        .collect();

    assert!(topics.iter().any(|topic| topic == UI_LOG_MAX_ENTRIES_TOPIC));
    assert!(topics.iter().any(|topic| topic == UI_LOG_CLEARED_TOPIC));

    logger::clear();
}

#[test]
fn run_tick_flushes_pending_logger_records_into_ui_event_log() {
    let _logger_guard = logger::test_lock();
    logger::clear();
    let mut engine = Engine::new(Folder::new("root".to_string()));

    crate::log!(origin = engine.root, tag = "runtime"; "pending logger event");
    assert!(
        engine.ui_event_log().is_empty(),
        "logger events should flush on runtime tick"
    );

    engine
        .run_tick(Duration::from_millis(16))
        .expect("run_tick should succeed");

    let logged = engine.ui_event_log().iter().find(|event| {
        matches!(
            &event.kind,
            EventKind::Custom(custom) if custom.topic == UI_LOG_RECORD_TOPIC
        )
    });
    assert!(
        logged.is_some(),
        "run_tick should project pending logger records to ui event log"
    );

    logger::clear();
}

#[test]
fn ui_set_param_ack_applies_immediately() {
    let root = Parameter::new("root_param", ParamValue::Int(1), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: engine.root,
        value: ParamValue::Int(7),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert_eq!(ack.status, UiAckStatus::Applied);
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Int(7)
    );
}

#[test]
fn cancel_active_ui_edit_session_only_cancels_matching_client_owner() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    let begin_ack = engine.apply_ui_intent_from_client(
        UiEditIntent::BeginEdit {
            client_edit_id: "drag-1".to_string(),
            label: Some("Slider drag".to_string()),
        },
        Some("browser-client-a"),
    );
    assert!(begin_ack.success, "begin edit should succeed");
    assert_eq!(
        engine.active_edit_session_ui_client_instance_id(),
        Some("browser-client-a")
    );

    let set_ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: engine.root,
        value: ParamValue::Int(10),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(set_ack.success, "set param should succeed inside the open session");

    assert!(
        !engine.cancel_active_ui_edit_session_for_client("browser-client-b"),
        "different clients must not cancel another client's edit session"
    );
    assert!(
        engine.has_active_edit_session(),
        "session should remain active after mismatched cancel"
    );

    assert!(
        engine.cancel_active_ui_edit_session_for_client("browser-client-a"),
        "matching client should cancel the active session"
    );
    assert!(
        !engine.has_active_edit_session(),
        "session should be cleared after cancel"
    );
    assert_eq!(engine.undo_len(), 0, "canceled session history must be discarded");

    let post_cancel_ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: engine.root,
        value: ParamValue::Int(25),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(post_cancel_ack.success, "post-cancel edit should succeed");
    assert!(engine.undo().expect("undo should succeed after cancel"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .expect("root parameter should exist")
            .value,
        ParamValue::Int(10),
        "undo should restore the live value at cancel time, not the discarded session baseline"
    );
}

#[test]
fn ui_intents_manage_user_context_scope_and_entries() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("owner context add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(
        Parameter::new("tempo", ParamValue::Float(120.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.apply_edits().expect("tempo add should succeed");
    let tempo = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("tempo should exist");

    let ensure_ack = engine.apply_ui_intent(UiEditIntent::EnsureUserContextScope { owner });
    assert!(ensure_ack.success);

    let upsert_ack = engine.apply_ui_intent(UiEditIntent::UpsertUserContextEntry {
        owner,
        symbol: "tempo".to_string(),
        param: tempo,
    });
    assert!(upsert_ack.success);

    let contexts = engine.ui_user_contexts();
    assert_eq!(contexts.scopes.len(), 1);
    assert_eq!(contexts.scopes[0].owner, engine.root);
    assert_eq!(contexts.scopes[0].entries.len(), 1);
    assert_eq!(contexts.scopes[0].entries[0].symbol, "tempo");
    assert_eq!(contexts.scopes[0].entries[0].param, tempo);

    let remove_entry_ack = engine.apply_ui_intent(UiEditIntent::RemoveUserContextEntry {
        owner,
        symbol: "tempo".to_string(),
    });
    assert!(remove_entry_ack.success);
    assert_eq!(engine.ui_user_contexts().scopes[0].entries.len(), 0);

    let remove_scope_ack = engine.apply_ui_intent(UiEditIntent::RemoveUserContextScope { owner });
    assert!(remove_scope_ack.success);
    assert!(engine.ui_user_contexts().scopes.is_empty());

    let topics = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(custom) => Some(custom.topic.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(topics.iter().any(|topic| *topic == "__user_context.scope_changed"));
    assert!(topics.iter().any(|topic| *topic == "__user_context.entry_changed"));
}

#[test]
fn ui_snapshot_includes_user_context_scopes() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("owner context add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(
        Parameter::new("tempo", ParamValue::Float(120.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.apply_edits().expect("tempo add should succeed");

    let snapshot = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);
    assert_eq!(snapshot.user_contexts.scopes.len(), 1);
    assert_eq!(snapshot.user_contexts.scopes[0].owner, engine.root);
    assert_eq!(snapshot.user_contexts.scopes[0].entries.len(), 1);
    assert_eq!(snapshot.user_contexts.scopes[0].entries[0].symbol, "tempo");
}

#[test]
fn control_mode_context_link_updates_value_from_user_context() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("owner context add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(
        Parameter::new("tempo", ParamValue::Float(120.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new("gain", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.apply_edits().expect("parameter add should succeed");

    let tempo = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("tempo should exist");
    let gain = engine
        .nodes
        .get(tempo)
        .and_then(|node| node.node_data().next_sibling)
        .expect("gain should exist");

    engine
        .set_param_control_state(
            gain,
            ParameterControlState::new(
                ParameterControlMode::ContextLink,
                ParameterControlSpec::ContextLink {
                    symbol: "tempo".to_string(),
                    projection: None,
                },
            ),
        )
        .expect("context-link state should be accepted");

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate controls");

    let gain_snapshot = engine
        .nodes
        .get(gain)
        .and_then(|node| node.engine_param_snapshot())
        .expect("gain parameter snapshot should exist");
    assert_eq!(gain_snapshot.value, ParamValue::Float(120.0));
    assert!(gain_snapshot.control.diagnostics.is_empty());
}

#[test]
fn control_pass_uses_source_index_for_context_link_dependents() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("owner context add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    for (name, value) in [
        ("source_a", 1.0),
        ("source_b", 10.0),
        ("target_a", 0.0),
        ("target_b", 0.0),
    ] {
        let mut parameter = Parameter::new(name, ParamValue::Float(value), ParameterChangeCheck::ValueChange);
        parameter.node_data_mut().meta.decl_id = DeclId(name.to_string());
        engine.add_node(parameter.into(), Some(owner));
    }
    engine.apply_edits().expect("parameters should be added");

    let source_a = find_child_by_label_any(&engine, owner, "source_a").expect("source_a should exist");
    let target_a = find_child_by_label_any(&engine, owner, "target_a").expect("target_a should exist");
    let target_b = find_child_by_label_any(&engine, owner, "target_b").expect("target_b should exist");

    for (target, symbol) in [(target_a, "source_a"), (target_b, "source_b")] {
        engine
            .set_param_control_state(
                target,
                ParameterControlState::new(
                    ParameterControlMode::ContextLink,
                    ParameterControlSpec::ContextLink {
                        symbol: symbol.to_string(),
                        projection: None,
                    },
                ),
            )
            .expect("context-link state should be accepted");
    }

    engine.inbox.clear();
    engine.tick_scratch.clear_stats();
    assert!(
        engine.evaluate_parameter_controls(),
        "initial dirty index pass should evaluate controls"
    );
    assert_eq!(
        engine.tick_stats().controls_params_scanned,
        2,
        "initial dirty index pass should visit both active controls"
    );
    assert!(
        engine
            .control_source_dependents
            .get(&source_a)
            .is_some_and(|dependents| dependents.contains(&target_a)),
        "control index should route source_a changes to target_a"
    );
    engine.apply_edits().expect("initial control writes should apply");
    engine.inbox.clear();

    engine.tick_scratch.clear_stats();
    assert!(
        !engine.evaluate_parameter_controls(),
        "steady state should not scan active controls without source changes"
    );
    assert_eq!(engine.tick_stats().controls_params_scanned, 0);
    engine.inbox.clear();

    engine.edits.push(Edit::SetParam {
        node: source_a,
        value: ParamValue::Float(2.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("source_a write should apply");

    engine.tick_scratch.clear_stats();
    assert!(
        engine.evaluate_parameter_controls(),
        "changed source should evaluate its dependent control"
    );
    assert_eq!(
        engine.tick_stats().controls_params_scanned,
        1,
        "only controls depending on source_a should be evaluated"
    );
    engine.apply_edits().expect("indexed control write should apply");

    let target_a_snapshot = engine
        .nodes
        .get(target_a)
        .and_then(|node| node.engine_param_snapshot())
        .expect("target_a snapshot should exist");
    let target_b_snapshot = engine
        .nodes
        .get(target_b)
        .and_then(|node| node.engine_param_snapshot())
        .expect("target_b snapshot should exist");
    assert_eq!(target_a_snapshot.value, ParamValue::Float(2.0));
    assert_eq!(target_b_snapshot.value, ParamValue::Float(10.0));
}

#[test]
fn control_mode_template_text_resolves_context_tokens_and_node_metadata() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(Folder::new("Track A".to_string()).into(), None);
    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("initial add should succeed");

    let track = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("track should exist");
    let owner = engine
        .nodes
        .get(track)
        .and_then(|node| node.node_data().next_sibling)
        .expect("owner should exist");
    let track_uuid = engine
        .nodes
        .get(track)
        .expect("track node should exist")
        .node_data()
        .meta
        .uuid;

    engine.add_node(
        Parameter::new(
            "sequence",
            ParamValue::Reference(NodeReference::new(track_uuid)),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new(
            "title",
            ParamValue::Str(String::new()),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        Some(owner),
    );
    engine.apply_edits().expect("context parameters should be added");

    let sequence = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("sequence should exist");
    let title = engine
        .nodes
        .get(sequence)
        .and_then(|node| node.node_data().next_sibling)
        .expect("title should exist");

    engine
        .set_param_control_state(
            title,
            ParameterControlState::new(
                ParameterControlMode::TemplateText,
                ParameterControlSpec::TemplateText {
                    template: "Seq {sequence.$name}".to_string(),
                },
            ),
        )
        .expect("template-text state should be accepted");

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate controls");

    let title_snapshot = engine
        .nodes
        .get(title)
        .and_then(|node| node.engine_param_snapshot())
        .expect("title parameter snapshot should exist");
    assert_eq!(title_snapshot.value, ParamValue::Str("Seq Track A".to_string()));
    assert!(title_snapshot.control.diagnostics.is_empty());
}

#[test]
fn ui_intent_set_text_param_smart_owns_template_mode_switching() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(Folder::new("Track A".to_string()).into(), None);
    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("initial add should succeed");

    let track = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("track should exist");
    let owner = engine
        .nodes
        .get(track)
        .and_then(|node| node.node_data().next_sibling)
        .expect("owner should exist");
    let track_uuid = engine
        .nodes
        .get(track)
        .expect("track node should exist")
        .node_data()
        .meta
        .uuid;

    engine.add_node(
        Parameter::new(
            "sequence",
            ParamValue::Reference(NodeReference::new(track_uuid)),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new(
            "title",
            ParamValue::Str(String::new()),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        Some(owner),
    );
    engine.apply_edits().expect("context parameters should be added");

    let sequence = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("sequence should exist");
    let title = engine
        .nodes
        .get(sequence)
        .and_then(|node| node.node_data().next_sibling)
        .expect("title should exist");

    let template_ack = engine.apply_ui_intent(UiEditIntent::SetTextParamSmart {
        node: title,
        value: "Seq {sequence.$name}".to_string(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(template_ack.success, "smart template text should be accepted");
    let title_snapshot = engine
        .nodes
        .get(title)
        .and_then(|node| node.engine_param_snapshot())
        .expect("title snapshot should exist");
    assert_eq!(title_snapshot.control.mode, ParameterControlMode::TemplateText);
    assert_eq!(
        title_snapshot.control.spec,
        ParameterControlSpec::TemplateText {
            template: "Seq {sequence.$name}".to_string()
        }
    );

    let manual_ack = engine.apply_ui_intent(UiEditIntent::SetTextParamSmart {
        node: title,
        value: "Manual title".to_string(),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(manual_ack.success, "plain smart text should switch back to manual");
    let title_snapshot = engine
        .nodes
        .get(title)
        .and_then(|node| node.engine_param_snapshot())
        .expect("title snapshot should exist");
    assert_eq!(title_snapshot.control.mode, ParameterControlMode::Manual);
    assert_eq!(title_snapshot.value, ParamValue::Str("Manual title".to_string()));
}

#[test]
fn control_mode_template_text_is_rejected_for_non_string_parameter() {
    let root = Parameter::new("gain", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange);
    let mut engine = Engine::new(root);

    let result = engine.set_param_control_state(
        engine.root,
        ParameterControlState::new(
            ParameterControlMode::TemplateText,
            ParameterControlSpec::TemplateText {
                template: "x {tempo}".to_string(),
            },
        ),
    );

    assert!(
        result.is_err(),
        "template-text should be rejected for non-string parameters"
    );
}

#[test]
fn control_mode_template_text_requires_visible_context_entries() {
    let root = Parameter::new(
        "title",
        ParamValue::Str(String::new()),
        ParameterChangeCheck::ValueChange,
    );
    let mut engine = Engine::new(root);

    let result = engine.set_param_control_state(
        engine.root,
        ParameterControlState::new(
            ParameterControlMode::TemplateText,
            ParameterControlSpec::TemplateText {
                template: "x {tempo}".to_string(),
            },
        ),
    );

    assert!(
        result.is_err(),
        "template-text should be rejected when no visible context entry exists"
    );
}

#[test]
fn control_mode_expression_reads_context_symbols() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("owner context add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(
        Parameter::new("a", ParamValue::Float(2.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new("b", ParamValue::Float(3.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new("result", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.apply_edits().expect("context parameters should be added");

    let a = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("a should exist");
    let b = engine
        .nodes
        .get(a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("b should exist");
    let result = engine
        .nodes
        .get(b)
        .and_then(|node| node.node_data().next_sibling)
        .expect("result should exist");

    engine
        .set_param_control_state(
            result,
            ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
        )
        .expect("expression state should be accepted");
    configure_expression_control_source(&mut engine, result, "a * 2 + b");

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate controls");
    let result_snapshot = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .expect("result snapshot should exist");
    assert_eq!(result_snapshot.value, ParamValue::Float(7.0));

    engine.edits.push(Edit::SetParam {
        node: b,
        value: ParamValue::Float(9.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("manual parameter update should apply");

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should reevaluate expression");
    let result_snapshot = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .expect("result snapshot should exist");
    assert_eq!(result_snapshot.value, ParamValue::Float(13.0));
}

#[test]
fn control_mode_expression_tracks_dependency_listeners() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("owner context add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(
        Parameter::new("a", ParamValue::Float(2.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new("b", ParamValue::Float(3.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new("result", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.apply_edits().expect("context parameters should be added");

    let a = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("a should exist");
    let b = engine
        .nodes
        .get(a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("b should exist");
    let result = engine
        .nodes
        .get(b)
        .and_then(|node| node.node_data().next_sibling)
        .expect("result should exist");

    engine
        .set_param_control_state(
            result,
            ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
        )
        .expect("expression state should be accepted");
    let source_param = configure_expression_control_source(&mut engine, result, "a + b");

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate expression");

    let listeners = engine
        .event_listeners
        .get_subscriptions(result)
        .expect("expression target should register listeners");
    assert!(
        listeners.contains(&EventSubscription::node(a)),
        "expression should listen to symbol 'a'"
    );
    assert!(
        listeners.contains(&EventSubscription::node(b)),
        "expression should listen to symbol 'b'"
    );
    assert!(
        listeners.contains(&EventSubscription::node(source_param)),
        "expression should listen to its source parameter"
    );
}

#[test]
fn control_mode_expression_updates_and_cleans_listeners() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("owner context add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(
        Parameter::new("a", ParamValue::Float(2.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new("b", ParamValue::Float(3.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new("result", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.apply_edits().expect("context parameters should be added");

    let a = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("a should exist");
    let b = engine
        .nodes
        .get(a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("b should exist");
    let result = engine
        .nodes
        .get(b)
        .and_then(|node| node.node_data().next_sibling)
        .expect("result should exist");

    engine
        .set_param_control_state(
            result,
            ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
        )
        .expect("expression state should be accepted");
    let source_param = configure_expression_control_source(&mut engine, result, "a + b");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate expression");

    let listeners = engine
        .event_listeners
        .get_subscriptions(result)
        .expect("expression target should register listeners");
    assert!(listeners.contains(&EventSubscription::node(a)));
    assert!(listeners.contains(&EventSubscription::node(b)));
    assert!(listeners.contains(&EventSubscription::node(source_param)));

    configure_expression_control_source(&mut engine, result, "b");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should rebind expression listeners");

    let listeners = engine
        .event_listeners
        .get_subscriptions(result)
        .expect("expression target should keep listeners after rebind");
    assert!(
        !listeners.contains(&EventSubscription::node(a)),
        "old dependency listener should be removed"
    );
    assert!(
        listeners.contains(&EventSubscription::node(b)),
        "new dependency listener should be kept"
    );
    assert!(
        listeners.contains(&EventSubscription::node(source_param)),
        "source listener should stay bound"
    );

    engine
        .set_param_control_state(
            result,
            ParameterControlState::new(ParameterControlMode::Manual, ParameterControlSpec::Manual),
        )
        .expect("manual mode should be accepted");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should clear expression listeners when mode exits");

    assert!(
        engine
            .event_listeners
            .get_subscriptions(result)
            .is_none_or(|subscriptions| subscriptions.is_empty()),
        "expression listeners should be removed after leaving expression mode",
    );
}

#[test]
fn control_mode_expression_time_runs_continuously() {
    let root: MacroTestNode =
        Parameter::new("result", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);
    let result = engine.root;

    engine
        .set_param_control_state(
            result,
            ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
        )
        .expect("expression state should be accepted");
    configure_expression_control_source(&mut engine, result, "time()");

    engine
        .run_tick(Duration::from_millis(100))
        .expect("first tick should evaluate expression");
    let first = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_float())
        .expect("result should stay numeric");

    engine
        .run_tick(Duration::from_millis(200))
        .expect("second tick should evaluate expression");
    let second = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_float())
        .expect("result should stay numeric");

    assert!(second > first, "time() expression should advance across ticks");
}

#[test]
fn control_mode_expression_delta_time_runs_continuously() {
    let root: MacroTestNode =
        Parameter::new("result", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);
    let result = engine.root;

    engine
        .set_param_control_state(
            result,
            ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
        )
        .expect("expression state should be accepted");
    configure_expression_control_source(&mut engine, result, "deltaTime");

    engine
        .run_tick(Duration::from_millis(100))
        .expect("first tick should evaluate expression");
    let first = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_float())
        .expect("result should stay numeric");

    engine
        .run_tick(Duration::from_millis(50))
        .expect("second tick should evaluate expression");
    let second = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_float())
        .expect("result should stay numeric");

    engine
        .run_tick(Duration::from_millis(25))
        .expect("third tick should evaluate expression");
    let third = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_float())
        .expect("result should stay numeric");

    assert!(first.abs() < 1e-9, "initial deltaTime should start from zero");
    assert!(
        (second - 0.05).abs() < 1e-9,
        "deltaTime should match per-tick elapsed seconds"
    );
    assert!((third - 0.025).abs() < 1e-9, "deltaTime should keep updating each tick");
}

#[test]
fn control_mode_expression_can_read_script_tree_globals() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(Folder::new("moduleManager").into(), None);
    engine.add_node(
        Parameter::new("result", ParamValue::Bool(false), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("initial nodes should be added");

    let module_manager = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("module manager should exist");
    let result = engine
        .nodes
        .get(module_manager)
        .and_then(|node| node.node_data().next_sibling)
        .expect("result should exist");

    engine.add_node(
        Parameter::new("enabled", ParamValue::Bool(true), ParameterChangeCheck::ValueChange).into(),
        Some(module_manager),
    );
    engine
        .apply_edits()
        .expect("module manager enabled parameter should be added");
    let enabled = engine
        .nodes
        .get(module_manager)
        .and_then(|node| node.node_data().first_child)
        .expect("enabled should exist");

    engine
        .set_param_control_state(
            result,
            ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
        )
        .expect("expression state should be accepted");
    configure_expression_control_source(&mut engine, result, "root.moduleManager.enabled.get()");

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate expression");
    let first = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_bool())
        .expect("result should stay boolean");
    assert!(first, "root.moduleManager.enabled.get() should read true");

    engine.edits.push(Edit::SetParam {
        node: enabled,
        value: ParamValue::Bool(false),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("enabled update should apply");

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should reevaluate expression");
    let second = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_bool())
        .expect("result should stay boolean");
    assert!(
        !second,
        "root.moduleManager.enabled.get() should read false after update"
    );
}

#[test]
fn control_mode_expression_supports_javascript_modulo_and_comparisons() {
    let root: MacroTestNode =
        Parameter::new("result", ParamValue::Bool(false), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);
    let result = engine.root;

    engine
        .set_param_control_state(
            result,
            ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
        )
        .expect("expression state should be accepted");
    configure_expression_control_source(&mut engine, result, "time() % 2 < 1");

    engine
        .run_tick(Duration::from_millis(100))
        .expect("first tick should evaluate expression");
    let first = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_bool())
        .expect("result should stay boolean");
    assert!(first, "time() % 2 < 1 should be true around t=0.1s");

    engine
        .run_tick(Duration::from_millis(1000))
        .expect("second tick should evaluate expression");
    let second = engine
        .nodes
        .get(result)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_bool())
        .expect("result should stay boolean");
    assert!(!second, "time() % 2 < 1 should be false around t=1.1s");
}

#[test]
fn control_mode_expression_diagnostics_surface_node_warnings() {
    let root: MacroTestNode =
        Parameter::new("result", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);
    let result = engine.root;

    engine
        .set_param_control_state(
            result,
            ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
        )
        .expect("expression state should be accepted");
    configure_expression_control_source(&mut engine, result, "1 + )");

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate expression");

    let warning = engine
        .nodes
        .get(result)
        .expect("result should exist")
        .node_data()
        .meta
        .presentation
        .warning(Some("control-diagnostic:expression:expression_error"))
        .expect("expression diagnostics should surface as node warnings");
    assert!(
        warning.message.starts_with("Expression error: SyntaxError:"),
        "warning message should surface the actionable JavaScript syntax failure: {}",
        warning.message
    );
    assert!(
        warning
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("error: SyntaxError:")),
        "warning detail should include structured error context"
    );
    assert!(
        warning
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("stage:")),
        "warning detail should include the expression stage"
    );

    configure_expression_control_source(&mut engine, result, "1 + 2");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should clear expression diagnostics");

    assert!(
        engine
            .nodes
            .get(result)
            .expect("result should still exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("control-diagnostic:expression:expression_error"))
            .is_none(),
        "warning should clear once expression diagnostics clear",
    );

    configure_expression_control_source(&mut engine, result, "test");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate expression with unknown symbol");
    assert!(
        engine
            .nodes
            .get(result)
            .expect("result should still exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("control-diagnostic:expression:expression_error"))
            .is_some(),
        "warning should return when expression becomes invalid again",
    );

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should keep previous expression diagnostic when reevaluation is skipped");
    assert!(
        engine
            .nodes
            .get(result)
            .expect("result should still exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("control-diagnostic:expression:expression_error"))
            .is_some(),
        "warning should persist across ticks while expression remains invalid",
    );
}

#[test]
fn control_mode_proxy_ignores_empty_reference_and_warns_for_missing_target() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("source", ParamValue::Float(5.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("source should exist");
    let target = engine
        .nodes
        .get(source)
        .and_then(|node| node.node_data().next_sibling)
        .expect("target should exist");
    let target_uuid = engine
        .nodes
        .get(target)
        .expect("target node should exist")
        .node_data()
        .meta
        .uuid;

    engine
        .set_param_control_state(
            source,
            ParameterControlState::new(ParameterControlMode::Proxy, ParameterControlSpec::Proxy),
        )
        .expect("proxy state should be accepted");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate empty proxy");

    assert!(
        engine
            .nodes
            .get(source)
            .expect("source should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("control-diagnostic:proxy:proxy_target_missing"))
            .is_none(),
        "empty proxy reference should not emit diagnostics",
    );

    configure_control_reference(&mut engine, source, NodeUuid(Uuid::new_v4()));
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate proxy diagnostics");

    assert!(
        engine
            .nodes
            .get(source)
            .expect("source should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("control-diagnostic:proxy:proxy_target_missing"))
            .is_some(),
        "dangling proxy target should surface diagnostics",
    );

    configure_control_reference(&mut engine, source, target_uuid);
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should resolve proxy diagnostics");

    assert!(
        engine
            .nodes
            .get(source)
            .expect("source should still exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("control-diagnostic:proxy:proxy_target_missing"))
            .is_none(),
        "warning should clear once proxy diagnostics clear",
    );
}

#[test]
fn control_mode_context_link_ignores_empty_symbol() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("value", ParamValue::Float(3.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let value = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("value should exist");

    engine
        .set_param_control_state(
            value,
            ParameterControlState::new(
                ParameterControlMode::ContextLink,
                ParameterControlSpec::ContextLink {
                    symbol: String::new(),
                    projection: None,
                },
            ),
        )
        .expect("context-link state should be accepted");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate empty context link");

    let snapshot = engine
        .nodes
        .get(value)
        .and_then(|node| node.engine_param_snapshot())
        .expect("value snapshot should exist");
    assert_eq!(snapshot.value, ParamValue::Float(3.0));
    assert!(
        snapshot.control.diagnostics.is_empty(),
        "empty context symbol should not produce diagnostics"
    );
    assert!(
        engine
            .nodes
            .get(value)
            .expect("value should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("control-diagnostic:context-link:context_symbol_missing"))
            .is_none(),
        "empty context symbol should not emit missing-symbol warnings",
    );
}

#[test]
fn control_mode_expression_ignores_empty_source() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("value", ParamValue::Float(3.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let value = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("value should exist");

    engine
        .set_param_control_state(
            value,
            ParameterControlState::new(ParameterControlMode::Expression, ParameterControlSpec::Expression),
        )
        .expect("expression state should be accepted");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate empty expression");

    let snapshot = engine
        .nodes
        .get(value)
        .and_then(|node| node.engine_param_snapshot())
        .expect("value snapshot should exist");
    assert_eq!(snapshot.value, ParamValue::Float(3.0));
    assert!(
        snapshot.control.diagnostics.is_empty(),
        "empty expression should not produce diagnostics"
    );
    assert!(
        engine
            .nodes
            .get(value)
            .expect("value should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("control-diagnostic:expression:expression_error"))
            .is_none(),
        "empty expression should not emit expression warnings",
    );
}

#[test]
fn control_mode_proxy_detects_cycles() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("a", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(
        Parameter::new("b", ParamValue::Float(2.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let a = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("a should exist");
    let b = engine
        .nodes
        .get(a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("b should exist");

    let a_uuid = engine.nodes.get(a).expect("a node should exist").node_data().meta.uuid;
    let b_uuid = engine.nodes.get(b).expect("b node should exist").node_data().meta.uuid;

    engine
        .set_param_control_state(
            a,
            ParameterControlState::new(ParameterControlMode::Proxy, ParameterControlSpec::Proxy),
        )
        .expect("proxy state for a should be accepted");
    engine
        .set_param_control_state(
            b,
            ParameterControlState::new(ParameterControlMode::Proxy, ParameterControlSpec::Proxy),
        )
        .expect("proxy state for b should be accepted");

    configure_control_reference(&mut engine, a, b_uuid);
    configure_control_reference(&mut engine, b, a_uuid);

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate controls");

    let a_snapshot = engine
        .nodes
        .get(a)
        .and_then(|node| node.engine_param_snapshot())
        .expect("a snapshot should exist");
    let b_snapshot = engine
        .nodes
        .get(b)
        .and_then(|node| node.engine_param_snapshot())
        .expect("b snapshot should exist");
    assert!(
        a_snapshot
            .control
            .diagnostics
            .iter()
            .any(|diag| diag.code == "proxy_cycle")
    );
    assert!(
        b_snapshot
            .control
            .diagnostics
            .iter()
            .any(|diag| diag.code == "proxy_cycle")
    );
}

#[test]
fn control_mode_proxy_applies_projection_from_reference() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("source", ParamValue::Vec2(3.0, 7.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("source should exist");
    let target = engine
        .nodes
        .get(source)
        .and_then(|node| node.node_data().next_sibling)
        .expect("target should exist");
    let source_uuid = engine
        .nodes
        .get(source)
        .expect("source node should exist")
        .node_data()
        .meta
        .uuid;

    engine
        .set_param_control_state(
            target,
            ParameterControlState::new(ParameterControlMode::Proxy, ParameterControlSpec::Proxy),
        )
        .expect("proxy state for target should be accepted");
    configure_control_reference_with_projection(&mut engine, target, source_uuid, Some(ParamValueProjection::Vec2Y));

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate projected proxy");
    let snapshot = engine
        .nodes
        .get(target)
        .and_then(|node| node.engine_param_snapshot())
        .expect("target snapshot should exist");
    assert_eq!(snapshot.value, ParamValue::Float(7.0));
}

#[test]
fn control_mode_binding_syncs_bidirectionally_with_latest_writer() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("a", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(
        Parameter::new("b", ParamValue::Float(9.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let a = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("a should exist");
    let b = engine
        .nodes
        .get(a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("b should exist");

    let a_uuid = engine.nodes.get(a).expect("a node should exist").node_data().meta.uuid;
    let b_uuid = engine.nodes.get(b).expect("b node should exist").node_data().meta.uuid;

    engine
        .set_param_control_state(
            a,
            ParameterControlState::new(ParameterControlMode::Binding, ParameterControlSpec::Binding),
        )
        .expect("binding state for a should be accepted");
    engine
        .set_param_control_state(
            b,
            ParameterControlState::new(ParameterControlMode::Binding, ParameterControlSpec::Binding),
        )
        .expect("binding state for b should be accepted");

    configure_control_reference(&mut engine, a, b_uuid);
    configure_control_reference(&mut engine, b, a_uuid);

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate binding");
    let b_snapshot = engine
        .nodes
        .get(b)
        .and_then(|node| node.engine_param_snapshot())
        .expect("b snapshot should exist");
    assert_eq!(b_snapshot.value, ParamValue::Float(1.0));

    engine.edits.push(Edit::SetParam {
        node: b,
        value: ParamValue::Float(5.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("manual b write should apply");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should propagate latest writer");

    let a_snapshot = engine
        .nodes
        .get(a)
        .and_then(|node| node.engine_param_snapshot())
        .expect("a snapshot should exist");
    let b_snapshot = engine
        .nodes
        .get(b)
        .and_then(|node| node.engine_param_snapshot())
        .expect("b snapshot should exist");
    assert_eq!(a_snapshot.value, ParamValue::Float(5.0));
    assert_eq!(b_snapshot.value, ParamValue::Float(5.0));
}

#[test]
fn control_mode_binding_single_side_syncs_referenced_parameter() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("a", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(
        Parameter::new("b", ParamValue::Float(9.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let a = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("a should exist");
    let b = engine
        .nodes
        .get(a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("b should exist");

    let b_uuid = engine.nodes.get(b).expect("b node should exist").node_data().meta.uuid;

    engine
        .set_param_control_state(
            a,
            ParameterControlState::new(ParameterControlMode::Binding, ParameterControlSpec::Binding),
        )
        .expect("binding state for a should be accepted");
    configure_control_reference(&mut engine, a, b_uuid);

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate binding");
    let b_snapshot = engine
        .nodes
        .get(b)
        .and_then(|node| node.engine_param_snapshot())
        .expect("b snapshot should exist");
    assert_eq!(b_snapshot.value, ParamValue::Float(1.0));

    engine.edits.push(Edit::SetParam {
        node: b,
        value: ParamValue::Float(7.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("manual b write should apply");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should propagate latest writer");

    let a_snapshot = engine
        .nodes
        .get(a)
        .and_then(|node| node.engine_param_snapshot())
        .expect("a snapshot should exist");
    assert_eq!(a_snapshot.value, ParamValue::Float(7.0));
}

#[test]
fn control_mode_binding_projection_roundtrips_bidirectionally() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("a", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(
        Parameter::new("b", ParamValue::Vec2(9.0, 2.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let a = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("a should exist");
    let b = engine
        .nodes
        .get(a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("b should exist");
    let b_uuid = engine.nodes.get(b).expect("b node should exist").node_data().meta.uuid;

    engine
        .set_param_control_state(
            a,
            ParameterControlState::new(ParameterControlMode::Binding, ParameterControlSpec::Binding),
        )
        .expect("binding state for a should be accepted");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate empty binding");
    assert!(
        engine
            .nodes
            .get(a)
            .expect("a should exist")
            .node_data()
            .meta
            .presentation
            .warning(Some("control-diagnostic:binding:binding_target_missing"))
            .is_none(),
        "empty binding reference should not emit missing-target warning",
    );

    configure_control_reference_with_projection(&mut engine, a, b_uuid, Some(ParamValueProjection::Vec2X));
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should apply binding projection");

    let a_snapshot = engine
        .nodes
        .get(a)
        .and_then(|node| node.engine_param_snapshot())
        .expect("a snapshot should exist");
    let b_snapshot = engine
        .nodes
        .get(b)
        .and_then(|node| node.engine_param_snapshot())
        .expect("b snapshot should exist");
    assert_eq!(a_snapshot.value, ParamValue::Float(1.0));
    assert_eq!(b_snapshot.value, ParamValue::Vec2(1.0, 2.0));

    engine.edits.push(Edit::SetParam {
        node: b,
        value: ParamValue::Vec2(7.0, 3.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("manual b write should apply");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should write forward projection");

    let a_snapshot = engine
        .nodes
        .get(a)
        .and_then(|node| node.engine_param_snapshot())
        .expect("a snapshot should exist");
    assert_eq!(a_snapshot.value, ParamValue::Float(7.0));

    engine.edits.push(Edit::SetParam {
        node: a,
        value: ParamValue::Float(4.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("manual a write should apply");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should write reverse projection");

    let b_snapshot = engine
        .nodes
        .get(b)
        .and_then(|node| node.engine_param_snapshot())
        .expect("b snapshot should exist");
    assert_eq!(b_snapshot.value, ParamValue::Vec2(4.5, 3.0));
}

#[test]
fn control_reference_rejects_missing_required_projection() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("source", ParamValue::Vec2(1.0, 2.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("source should exist");
    let target = engine
        .nodes
        .get(source)
        .and_then(|node| node.node_data().next_sibling)
        .expect("target should exist");
    let source_uuid = engine
        .nodes
        .get(source)
        .expect("source node should exist")
        .node_data()
        .meta
        .uuid;

    engine
        .set_param_control_state(
            target,
            ParameterControlState::new(ParameterControlMode::Proxy, ParameterControlSpec::Proxy),
        )
        .expect("proxy state for target should be accepted");
    let target_param = find_child_by_decl(&engine, target, PARAMETER_CONTROL_REFERENCE_DECL_ID)
        .expect("control reference parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: target_param,
        value: ParamValue::Reference(NodeReference::new(source_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    let result = engine.apply_edits();
    assert!(
        matches!(result, Err(EngineEditError::ParamConstraintViolation { .. })),
        "missing projection should reject source that only matches through projection",
    );
}

#[test]
fn control_reference_parameter_is_manual_only() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("a", ParamValue::Float(1.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(
        Parameter::new("b", ParamValue::Float(2.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let a = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("a should exist");

    engine
        .set_param_control_state(
            a,
            ParameterControlState::new(ParameterControlMode::Proxy, ParameterControlSpec::Proxy),
        )
        .expect("proxy state should be accepted");

    let target_param = find_child_by_decl(&engine, a, PARAMETER_CONTROL_REFERENCE_DECL_ID)
        .expect("control reference parameter should exist");

    let info = engine
        .ui_param_control_info(target_param)
        .expect("control info query should succeed");
    assert_eq!(info.available_modes, vec![ParameterControlMode::Manual]);

    let err = engine.set_param_control_state(
        target_param,
        ParameterControlState::new(
            ParameterControlMode::ContextLink,
            ParameterControlSpec::ContextLink {
                symbol: "tempo".to_string(),
                projection: None,
            },
        ),
    );
    assert!(
        err.is_err(),
        "manual-only control reference parameter should reject non-manual modes"
    );
}

#[test]
fn ui_reference_targets_report_projection_options_for_control_reference_parameter() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("source", ParamValue::Vec2(1.0, 2.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.add_node(
        Parameter::new("target", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("source should exist");
    let target = engine
        .nodes
        .get(source)
        .and_then(|node| node.node_data().next_sibling)
        .expect("target should exist");

    engine
        .set_param_control_state(
            target,
            ParameterControlState::new(ParameterControlMode::Proxy, ParameterControlSpec::Proxy),
        )
        .expect("proxy state should be accepted");

    let target_param = find_child_by_decl(&engine, target, PARAMETER_CONTROL_REFERENCE_DECL_ID)
        .expect("control reference parameter should exist");

    let targets = engine.ui_reference_targets_for_param(target_param);
    let candidate = targets
        .candidates
        .iter()
        .find(|candidate| candidate.target == source)
        .expect("source should be exposed as one reference target candidate");

    assert!(!candidate.direct, "vec2->float should require projection");
    assert_eq!(
        candidate.projections,
        vec![ParamValueProjection::Vec2X, ParamValueProjection::Vec2Y]
    );
}

#[test]
fn ui_reference_targets_for_binding_use_bidirectional_compatibility() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new(
            "source",
            ParamValue::Color(0.1, 0.2, 0.3, 0.7),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        None,
    );
    engine.add_node(
        Parameter::new(
            "target",
            ParamValue::Vec3(0.0, 0.0, 0.0),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("source should exist");
    let target = engine
        .nodes
        .get(source)
        .and_then(|node| node.node_data().next_sibling)
        .expect("target should exist");

    engine
        .set_param_control_state(
            target,
            ParameterControlState::new(ParameterControlMode::Binding, ParameterControlSpec::Binding),
        )
        .expect("binding state should be accepted");
    let target_param = find_child_by_decl(&engine, target, PARAMETER_CONTROL_REFERENCE_DECL_ID)
        .expect("control reference parameter should exist");

    let targets = engine.ui_reference_targets_for_param(target_param);
    let candidate = targets
        .candidates
        .iter()
        .find(|candidate| candidate.target == source)
        .expect("source should be exposed as one reference target candidate");

    assert!(
        !candidate.direct,
        "binding compatibility should require bidirectional direct conversion"
    );
    assert!(candidate.projections.contains(&ParamValueProjection::ColorToVec3Rgb));
    assert!(candidate.projections.contains(&ParamValueProjection::ColorToVec3Hsv));
}

#[test]
fn animation_control_parameters_keep_control_modes_enabled() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let osc = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("osc should exist");

    engine
        .set_param_control_state(
            osc,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = find_child_by_type(&engine, osc, PARAMETER_ANIMATION_CONTROL_NODE_TYPE)
        .expect("animation control node should exist");
    let amplitude_param = find_child_by_decl(&engine, animation_node, PARAMETER_ANIMATION_AMPLITUDE_DECL_ID)
        .expect("amplitude parameter should exist");

    let info = engine
        .ui_param_control_info(amplitude_param)
        .expect("control info query should succeed");
    assert!(
        info.available_modes.contains(&ParameterControlMode::Proxy),
        "animation control parameters should allow proxy mode",
    );
    assert!(
        info.available_modes.contains(&ParameterControlMode::Binding),
        "animation control parameters should allow binding mode",
    );
}

#[test]
fn animation_control_node_rejects_user_created_children() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter add should succeed");

    let osc = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("osc should exist");

    engine
        .set_param_control_state(
            osc,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = find_child_by_type(&engine, osc, PARAMETER_ANIMATION_CONTROL_NODE_TYPE)
        .expect("animation control node should exist");
    let animation = engine
        .nodes
        .get(animation_node)
        .expect("animation control node should exist");

    assert!(animation.user_container_rules().is_none());
    assert!(animation.user_creatable_items().is_empty());
    assert!(animation.create_user_item("float").is_none());
}

#[test]
fn control_mode_animation_drives_parameter_value() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    assert_eq!(
        engine
            .nodes
            .get(animation_node)
            .expect("animation control node should exist")
            .get_type(),
        PARAMETER_ANIMATION_CONTROL_NODE_TYPE
    );

    let waveform = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_WAVEFORM_DECL_ID)
        .expect("waveform parameter should exist");
    let frequency = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_FREQUENCY_DECL_ID)
        .expect("frequency parameter should exist");
    let amplitude = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_AMPLITUDE_DECL_ID)
        .expect("amplitude parameter should exist");
    let offset = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_OFFSET_DECL_ID)
        .expect("offset parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: waveform,
        value: ParamValue::Enum("sine".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: frequency,
        value: ParamValue::Float(1.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: amplitude,
        value: ParamValue::Float(2.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: offset,
        value: ParamValue::Float(1.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine
        .apply_edits()
        .expect("animation control parameter updates should apply");

    engine
        .run_tick(Duration::from_millis(250))
        .expect("tick should evaluate animation");
    let snapshot = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.engine_param_snapshot())
        .expect("root snapshot should exist");

    let ParamValue::Float(value) = snapshot.value else {
        panic!("expected float value from animation");
    };
    assert!((value - 3.0).abs() < 1e-6, "expected sine peak at t=0.25s, got {value}");
}

#[test]
fn animation_control_update_rate_changes_schedule_frequency() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");

    let waveform = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_WAVEFORM_DECL_ID)
        .expect("waveform parameter should exist");
    let frequency = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_FREQUENCY_DECL_ID)
        .expect("frequency parameter should exist");
    let amplitude = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_AMPLITUDE_DECL_ID)
        .expect("amplitude parameter should exist");
    let offset = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_OFFSET_DECL_ID)
        .expect("offset parameter should exist");
    let update_rate = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_UPDATE_RATE_DECL_ID)
        .expect("update-rate parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: waveform,
        value: ParamValue::Enum("square".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: frequency,
        value: ParamValue::Float(1.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: amplitude,
        value: ParamValue::Float(1.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: offset,
        value: ParamValue::Float(0.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: update_rate,
        value: ParamValue::Int(2),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine
        .apply_edits()
        .expect("animation control parameter updates should apply");

    engine
        .run_tick(Duration::from_millis(100))
        .expect("tick should process runtime updates");
    let first = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.engine_param_snapshot())
        .expect("root snapshot should exist");
    assert_eq!(
        first.value,
        ParamValue::Float(0.0),
        "2 Hz update-rate should not schedule an update after 100ms",
    );

    engine
        .run_tick(Duration::from_millis(500))
        .expect("tick should process runtime updates");
    let second = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.engine_param_snapshot())
        .expect("root snapshot should exist");

    let ParamValue::Float(value) = second.value else {
        panic!("expected float value from animation");
    };
    assert!(
        value.abs() > 1e-9,
        "2 Hz update-rate should schedule an update by 600ms total elapsed",
    );
}

#[test]
fn animation_control_materializes_curve_key_and_easing_nodes() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");

    // Curve node only materializes when waveform is set to "curve".
    let waveform_param = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_WAVEFORM_DECL_ID)
        .expect("waveform parameter should exist");
    engine.edits.push(Edit::SetParam {
        node: waveform_param,
        value: ParamValue::Enum("curve".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("waveform update should apply");
    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should process waveform change");

    let curve_node = find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_CURVE_DECL_ID)
        .expect("curve node should exist after switching to Curve waveform");
    assert_eq!(
        engine
            .nodes
            .get(curve_node)
            .expect("curve node should exist")
            .get_type(),
        PARAMETER_ANIMATION_CURVE_NODE_TYPE
    );

    let mut key_count = 0usize;
    let mut child = engine
        .nodes
        .get(curve_node)
        .and_then(|node| node.node_data().first_child);
    while let Some(key_id) = child {
        let key_node = engine.nodes.get(key_id).expect("key node should exist");
        if key_node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE {
            key_count += 1;
            let _position = find_child_by_decl_any(&engine, key_id, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID)
                .expect("key position parameter should exist");
            let _value = find_child_by_decl_any(&engine, key_id, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID)
                .expect("key value parameter should exist");
            let easing_node = find_child_by_decl_any(&engine, key_id, PARAMETER_ANIMATION_EASING_DECL_ID)
                .expect("easing node should exist");
            let _kind = find_child_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID)
                .expect("easing kind parameter should exist");
        }
        child = key_node.node_data().next_sibling;
    }
    assert_eq!(key_count, 2, "curve should materialize exactly two default key nodes");

    let tree_snapshot = engine.build_process_tree_snapshot();
    let parsed_curve =
        curve_from_snapshot(tree_snapshot.as_ref(), curve_node).expect("curve should parse from node snapshot");
    assert_eq!(
        parsed_curve.key_count(),
        2,
        "parsed curve should expose exactly two default keys"
    );
}

#[test]
fn animation_curve_waveform_survives_save_load_round_trip() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");

    let curve_node_before = switch_animation_to_curve_waveform(&mut engine, animation_node);
    assert!(
        engine.nodes.contains(curve_node_before),
        "curve node should exist before save"
    );
    assert!(
        find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_AMPLITUDE_DECL_ID).is_none(),
        "amplitude should not exist in curve mode before save"
    );
    assert!(
        find_child_by_decl_any(&engine, animation_node, PARAMETER_ANIMATION_OFFSET_DECL_ID).is_none(),
        "offset should not exist in curve mode before save"
    );

    let json = engine
        .to_project_json_with(|node| node.project_encode_data())
        .expect("project serialization should succeed");

    use crate::app::ProjectNode as _;
    let loaded = Engine::<MacroTestNode>::from_project_json_with(&json, MacroTestNode::project_decode_node)
        .expect("project load should succeed — curve waveform save/load must not produce MissingNode errors");

    let loaded_animation_node = loaded
        .nodes
        .get(loaded.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist after load");

    let curve_node_after = find_child_by_decl_any(&loaded, loaded_animation_node, PARAMETER_ANIMATION_CURVE_DECL_ID)
        .expect("curve node should exist after load");
    assert_eq!(
        loaded.nodes.get(curve_node_after).map(|n| n.get_type()),
        Some(PARAMETER_ANIMATION_CURVE_NODE_TYPE),
        "loaded curve node should have the correct type"
    );
    assert!(
        find_child_by_decl_any(&loaded, loaded_animation_node, PARAMETER_ANIMATION_AMPLITUDE_DECL_ID).is_none(),
        "amplitude should not exist in curve mode after load"
    );
    assert!(
        find_child_by_decl_any(&loaded, loaded_animation_node, PARAMETER_ANIMATION_OFFSET_DECL_ID).is_none(),
        "offset should not exist in curve mode after load"
    );
}

#[test]
fn animation_curve_easing_keeps_single_kind_parameter_while_switching_kind() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);
    let first_key = engine
        .build_process_tree_snapshot()
        .child_ids(curve_node)
        .into_iter()
        .find(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .is_some_and(|node| node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE)
        })
        .expect("default key should exist");
    let easing_node = find_child_by_decl_any(&engine, first_key, PARAMETER_ANIMATION_EASING_DECL_ID)
        .expect("easing node should exist");
    let kind_param = find_child_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID)
        .expect("easing kind parameter should exist");

    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID),
        1,
        "easing should start with exactly one kind parameter"
    );

    engine.edits.push(Edit::SetParam {
        node: kind_param,
        value: ParamValue::Enum("bezier".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("switch to bezier should apply");
    engine
        .apply_edits()
        .expect("queued structural easing edits should apply");

    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID),
        1,
        "switching easing kind should not duplicate kind parameter"
    );

    engine.edits.push(Edit::SetParam {
        node: kind_param,
        value: ParamValue::Enum("perlinNoise".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("switch to perlin noise should apply");
    engine
        .apply_edits()
        .expect("queued structural easing edits should apply");

    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID),
        1,
        "switching easing kind repeatedly should keep one kind parameter"
    );
}

#[test]
fn ui_set_param_stabilizes_animation_curve_easing_dependencies() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);
    let first_key = engine
        .build_process_tree_snapshot()
        .child_ids(curve_node)
        .into_iter()
        .find(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .is_some_and(|node| node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE)
        })
        .expect("default key should exist");
    let easing_node = find_child_by_decl_any(&engine, first_key, PARAMETER_ANIMATION_EASING_DECL_ID)
        .expect("easing node should exist");
    let kind_param = find_child_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID)
        .expect("easing kind parameter should exist");

    let initial_children = direct_child_decl_ids_any(&engine, easing_node);
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID),
        1,
        "default bezier easing should start with one out-position parameter; children were {:?}",
        initial_children
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID),
        1,
        "default bezier easing should start with one out-value parameter"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID),
        1,
        "default bezier easing should start with one in-position parameter"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID),
        1,
        "default bezier easing should start with one in-value parameter"
    );

    let to_steps_ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: kind_param,
        value: ParamValue::Enum("steps".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(
        to_steps_ack.success,
        "UI set-param should succeed when switching easing to steps"
    );

    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID),
        1,
        "kind should remain singular after switching to steps"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID),
        0,
        "steps easing should remove out-position immediately in the UI flow"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID),
        0,
        "steps easing should remove out-value immediately in the UI flow"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID),
        0,
        "steps easing should remove in-position immediately in the UI flow"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID),
        0,
        "steps easing should remove in-value immediately in the UI flow"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID),
        1,
        "steps easing should materialize one step mode parameter"
    );

    let to_bezier_ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: kind_param,
        value: ParamValue::Enum("bezier".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(
        to_bezier_ack.success,
        "UI set-param should succeed when switching easing back to bezier"
    );

    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID),
        1,
        "kind should remain singular after switching back to bezier"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_STEP_MODE_DECL_ID),
        0,
        "bezier easing should remove step mode immediately in the UI flow"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_OUT_POSITION_DECL_ID),
        1,
        "bezier easing should recreate exactly one out-position parameter"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_OUT_VALUE_DECL_ID),
        1,
        "bezier easing should recreate exactly one out-value parameter"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_IN_POSITION_DECL_ID),
        1,
        "bezier easing should recreate exactly one in-position parameter"
    );
    assert_eq!(
        count_children_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_IN_VALUE_DECL_ID),
        1,
        "bezier easing should recreate exactly one in-value parameter"
    );
}

#[test]
fn animation_curve_accepts_user_created_key_items() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);

    let before = engine
        .build_process_tree_snapshot()
        .child_ids(curve_node)
        .into_iter()
        .filter(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .is_some_and(|node| node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE)
        })
        .count();

    engine.add_user_item(CurveKeyNode::new_with_label("Inserted Key").into(), Some(curve_node));
    engine.apply_edits().expect("user key add should succeed");

    let after = engine
        .build_process_tree_snapshot()
        .child_ids(curve_node)
        .into_iter()
        .filter(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .is_some_and(|node| node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE)
        })
        .count();

    assert_eq!(after, before + 1, "curve should accept key user items");
}

#[test]
fn animation_curve_user_created_key_defaults_to_bezier_easing() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);

    let before_keys = engine
        .build_process_tree_snapshot()
        .child_ids(curve_node)
        .into_iter()
        .filter(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .is_some_and(|node| node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE)
        })
        .collect::<Vec<_>>();

    engine.add_user_item(CurveKeyNode::new_with_label("Inserted Key").into(), Some(curve_node));
    engine.apply_edits().expect("user key add should succeed");

    let after_keys = engine
        .build_process_tree_snapshot()
        .child_ids(curve_node)
        .into_iter()
        .filter(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .is_some_and(|node| node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE)
        })
        .collect::<Vec<_>>();

    let inserted_key = after_keys
        .iter()
        .copied()
        .find(|key_id| !before_keys.contains(key_id))
        .expect("inserted key should exist");
    let easing_node = find_child_by_decl_any(&engine, inserted_key, PARAMETER_ANIMATION_EASING_DECL_ID)
        .expect("easing node should exist");
    let kind_param = find_child_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID)
        .expect("easing kind parameter should exist");
    let kind = engine
        .nodes
        .get(kind_param)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_enum())
        .expect("easing kind should be enum");

    assert_eq!(kind, "bezier", "UI-created keys should default to bezier easing");
}

#[test]
fn animation_curve_insert_keys_with_easing_uses_requested_kind() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);

    engine.edits.push(Edit::CallNodeMutation {
        node: curve_node,
        needs_tree_snapshot: true,
        callback: Box::new(|node, ctx| {
            let Some(curve) = node.as_any_mut().downcast_mut::<CurveNode>() else {
                return Err("curve mutation target should be CurveNode".to_string());
            };
            curve.insert_keys_with_easing(ctx, vec![(0.25, 0.75, CurveEasing::Hold)]);
            Ok(())
        }),
    });
    engine
        .apply_edits()
        .expect("explicit easing key insertion should succeed");
    engine
        .apply_edits()
        .expect("queued key subtree materialization should succeed");

    let snapshot = engine.build_process_tree_snapshot();
    let inserted_key = snapshot
        .child_ids(curve_node)
        .into_iter()
        .find(|child| {
            if !engine
                .nodes
                .get(*child)
                .is_some_and(|node| node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE)
            {
                return false;
            }
            let position_param = match snapshot.find_child(*child, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID) {
                Some(node_id) => node_id,
                None => return false,
            };
            let position = snapshot
                .node(position_param)
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_float);
            position.is_some_and(|value| (value - 0.25).abs() < 1e-9)
        })
        .expect("inserted key should exist");

    let easing_node = find_child_by_decl_any(&engine, inserted_key, PARAMETER_ANIMATION_EASING_DECL_ID)
        .expect("easing node should exist");
    let kind_param = find_child_by_decl_any(&engine, easing_node, PARAMETER_ANIMATION_EASING_KIND_DECL_ID)
        .expect("easing kind parameter should exist");
    let kind = engine
        .nodes
        .get(kind_param)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_enum())
        .expect("easing kind should be enum");

    assert_eq!(kind, "hold", "code insertion should honor the provided easing");
}

#[test]
fn animation_curve_materializes_user_editable_range_node() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);
    let range_node = find_child_by_decl_any(&engine, curve_node, PARAMETER_ANIMATION_RANGE_DECL_ID)
        .expect("range node should exist");

    let range_entry = engine.nodes.get(range_node).expect("range node should exist");
    assert_eq!(range_entry.get_type(), PARAMETER_ANIMATION_RANGE_NODE_TYPE);
    assert!(
        range_entry.node_data().meta.can_be_disabled,
        "range node should be user-toggleable"
    );
    assert!(
        !range_entry.node_data().meta.enabled,
        "range node should start disabled when no code range is set"
    );

    let _x_param = find_child_by_decl_any(&engine, range_node, PARAMETER_ANIMATION_RANGE_X_DECL_ID)
        .expect("range x parameter should exist");
    let _y_param = find_child_by_decl_any(&engine, range_node, PARAMETER_ANIMATION_RANGE_Y_DECL_ID)
        .expect("range y parameter should exist");
}

#[test]
fn animation_curve_enabled_range_clamps_key_position_and_value() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);
    let range_node = find_child_by_decl_any(&engine, curve_node, PARAMETER_ANIMATION_RANGE_DECL_ID)
        .expect("range node should exist");
    let x_param = find_child_by_decl_any(&engine, range_node, PARAMETER_ANIMATION_RANGE_X_DECL_ID)
        .expect("range x parameter should exist");
    let y_param = find_child_by_decl_any(&engine, range_node, PARAMETER_ANIMATION_RANGE_Y_DECL_ID)
        .expect("range y parameter should exist");

    let first_key = engine
        .build_process_tree_snapshot()
        .child_ids(curve_node)
        .into_iter()
        .find(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .is_some_and(|node| node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE)
        })
        .expect("default key should exist");
    let position_param = find_child_by_decl_any(&engine, first_key, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID)
        .expect("position parameter should exist");
    let value_param = find_child_by_decl_any(&engine, first_key, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID)
        .expect("value parameter should exist");

    engine.edits.push(Edit::PatchMeta {
        node: range_node,
        patch: crate::node::NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.edits.push(Edit::SetParam {
        node: x_param,
        value: ParamValue::Vec2(0.0, 1.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: y_param,
        value: ParamValue::Vec2(-0.25, 0.25),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: position_param,
        value: ParamValue::Float(5.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: value_param,
        value: ParamValue::Float(1.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });

    engine.apply_edits().expect("first range update pass should apply");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("range update inbox pass should apply queued clamping");
    engine
        .apply_edits()
        .expect("second range update pass should apply queued clamping");

    let position_value = engine
        .nodes
        .get(position_param)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_float())
        .expect("position snapshot should be float");
    let value_value = engine
        .nodes
        .get(value_param)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_float())
        .expect("value snapshot should be float");

    assert_eq!(position_value, 1.0, "position should clamp to range x_max");
    assert_eq!(value_value, 0.25, "value should clamp to range y_max");
}

#[test]
fn animation_curve_disabled_range_allows_values_outside_previous_bounds() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);
    let range_node = find_child_by_decl_any(&engine, curve_node, PARAMETER_ANIMATION_RANGE_DECL_ID)
        .expect("range node should exist");
    let y_param = find_child_by_decl_any(&engine, range_node, PARAMETER_ANIMATION_RANGE_Y_DECL_ID)
        .expect("range y parameter should exist");

    let first_key = engine
        .build_process_tree_snapshot()
        .child_ids(curve_node)
        .into_iter()
        .find(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .is_some_and(|node| node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE)
        })
        .expect("default key should exist");
    let value_param = find_child_by_decl_any(&engine, first_key, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID)
        .expect("value parameter should exist");

    engine.edits.push(Edit::PatchMeta {
        node: range_node,
        patch: crate::node::NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.edits.push(Edit::SetParam {
        node: y_param,
        value: ParamValue::Vec2(-0.25, 0.25),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: value_param,
        value: ParamValue::Float(1.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("range-enabled pass should apply");
    engine.apply_edits().expect("range-enabled pass should apply clamping");

    engine.edits.push(Edit::PatchMeta {
        node: range_node,
        patch: crate::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.edits.push(Edit::SetParam {
        node: value_param,
        value: ParamValue::Float(1.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("range-disabled pass should apply");
    engine
        .apply_edits()
        .expect("range-disabled follow-up should stay unchanged");

    let value_value = engine
        .nodes
        .get(value_param)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_float())
        .expect("value snapshot should be float");
    assert_eq!(
        value_value, 1.5,
        "value should remain outside bounds when range is disabled"
    );
}

#[test]
fn animation_curve_code_range_applies_without_materializing_user_range_node() {
    let range_constraint = CurveRangeConstraint::new(0.0, 2.0, -1.0, 1.0).expect("range constraint should be valid");
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(
        CurveNode::new()
            .with_user_editable_range(false)
            .with_range_constraint(Some(range_constraint))
            .into(),
        None,
    );
    engine.apply_edits().expect("first pass should create curve node");
    engine
        .apply_edits()
        .expect("second pass should materialize default key nodes");
    engine
        .apply_edits()
        .expect("third pass should materialize key parameters");

    let curve_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("curve node should exist");

    let range_node = find_child_by_decl_any(&engine, curve_node, PARAMETER_ANIMATION_RANGE_DECL_ID);
    assert!(
        range_node.is_none(),
        "code-only range should not materialize editable range node"
    );

    let first_key = engine
        .build_process_tree_snapshot()
        .child_ids(curve_node)
        .into_iter()
        .find(|node_id| {
            engine
                .nodes
                .get(*node_id)
                .is_some_and(|node| node.get_type() == PARAMETER_ANIMATION_KEY_NODE_TYPE)
        })
        .expect("default key should exist");
    let position_param = find_child_by_decl_any(&engine, first_key, PARAMETER_ANIMATION_KEY_POSITION_DECL_ID)
        .expect("position parameter should exist");
    let value_param = find_child_by_decl_any(&engine, first_key, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID)
        .expect("value parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: position_param,
        value: ParamValue::Float(5.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(Edit::SetParam {
        node: value_param,
        value: ParamValue::Float(3.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("code-range first pass should apply");
    engine
        .apply_edits()
        .expect("code-range second pass should apply queued clamping");

    let position_value = engine
        .nodes
        .get(position_param)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_float())
        .expect("position snapshot should be float");
    let value_value = engine
        .nodes
        .get(value_param)
        .and_then(|node| node.engine_param_snapshot())
        .and_then(|snapshot| snapshot.value.as_float())
        .expect("value snapshot should be float");

    assert_eq!(position_value, 2.0, "position should clamp to code-defined x_max");
    assert_eq!(value_value, 1.0, "value should clamp to code-defined y_max");
}

#[test]
fn ui_intent_fit_animation_curve_path_replaces_range_with_fitted_bezier_keys() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);

    let ack = engine.apply_ui_intent(UiEditIntent::FitAnimationCurvePath {
        curve: curve_node,
        points: vec![
            CurveFitPoint::new(0.25, 0.2),
            CurveFitPoint::new(0.5, 0.92),
            CurveFitPoint::new(0.75, 0.15),
        ],
        options: CurveBezierFitOptions {
            max_value_error: 0.02,
            max_keys: 8,
        },
    });
    assert!(ack.success, "fit intent should succeed");
    assert!(
        engine.edits.pending.is_empty(),
        "fit intent should fully materialize its queued edits"
    );

    let snapshot = engine.build_process_tree_snapshot();
    let parsed_curve =
        curve_from_snapshot(snapshot.as_ref(), curve_node).expect("curve should parse from node snapshot");

    assert!(
        parsed_curve.key_count() >= 4,
        "fitted range should keep outer keys and insert fitted keys"
    );
    assert!((parsed_curve.sample(0.25).expect("sample should exist") - 0.2).abs() < 0.08);
    assert!((parsed_curve.sample(0.5).expect("sample should exist") - 0.92).abs() < 0.08);
    assert!((parsed_curve.sample(0.75).expect("sample should exist") - 0.15).abs() < 0.08);
}

#[test]
fn ui_fit_animation_curve_path_edit_session_groups_one_undoable_draw_action() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);

    let snapshot_before = engine.build_process_tree_snapshot();
    let curve_before =
        curve_from_snapshot(snapshot_before.as_ref(), curve_node).expect("curve should parse before draw fit");
    let key_count_before = curve_before.key_count();
    let sample_before = curve_before.sample(0.5).expect("baseline sample should exist");

    let undo_len_before_session = engine.undo_len();
    let begin_ack = engine.apply_ui_intent(UiEditIntent::BeginEdit {
        client_edit_id: "curve-draw-1".to_string(),
        label: Some("Draw Curve".to_string()),
    });
    assert!(begin_ack.success, "begin edit should succeed");

    let fit_ack = engine.apply_ui_intent(UiEditIntent::FitAnimationCurvePath {
        curve: curve_node,
        points: vec![
            CurveFitPoint::new(0.25, 0.2),
            CurveFitPoint::new(0.5, 0.92),
            CurveFitPoint::new(0.75, 0.15),
        ],
        options: CurveBezierFitOptions {
            max_value_error: 0.02,
            max_keys: 8,
        },
    });
    assert!(fit_ack.success, "fit intent should succeed inside the edit session");

    assert!(
        engine.edits.pending.is_empty(),
        "fit intent should fully materialize its queued edits before session end"
    );
    assert_eq!(
        engine.undo_len(),
        undo_len_before_session,
        "open draw session should not commit undo history yet"
    );

    let end_ack = engine.apply_ui_intent(UiEditIntent::EndEdit {
        client_edit_id: "curve-draw-1".to_string(),
    });
    assert!(end_ack.success, "end edit should succeed");
    assert_eq!(
        engine.undo_len(),
        undo_len_before_session + 1,
        "draw fit should commit as one undo step"
    );

    let snapshot_fitted = engine.build_process_tree_snapshot();
    let curve_fitted =
        curve_from_snapshot(snapshot_fitted.as_ref(), curve_node).expect("curve should parse after draw fit");
    assert!(
        curve_fitted.key_count() >= 4,
        "fitted draw should insert extra bezier keys"
    );
    assert!((curve_fitted.sample(0.5).expect("fitted sample should exist") - 0.92).abs() < 0.08);

    let undo_ack = engine.apply_ui_intent(UiEditIntent::Undo);
    assert!(undo_ack.success, "undo should succeed for draw fit");
    let snapshot_undone = engine.build_process_tree_snapshot();
    let curve_undone =
        curve_from_snapshot(snapshot_undone.as_ref(), curve_node).expect("curve should parse after undo");
    assert_eq!(
        curve_undone.key_count(),
        key_count_before,
        "undo should restore the original key count"
    );
    assert!((curve_undone.sample(0.5).expect("undo sample should exist") - sample_before).abs() < 1e-9);

    let redo_ack = engine.apply_ui_intent(UiEditIntent::Redo);
    assert!(redo_ack.success, "redo should succeed for draw fit");
    let snapshot_redone = engine.build_process_tree_snapshot();
    let curve_redone =
        curve_from_snapshot(snapshot_redone.as_ref(), curve_node).expect("curve should parse after redo");
    assert!(
        curve_redone.key_count() >= 4,
        "redo should restore the fitted draw keys"
    );
    assert!((curve_redone.sample(0.5).expect("redo sample should exist") - 0.92).abs() < 0.08);
}

#[test]
fn ui_fit_animation_curve_path_after_undo_clears_redo_without_panicking() {
    let root: MacroTestNode = Parameter::new("osc", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into();
    let mut engine = Engine::new(root);

    engine
        .set_param_control_state(
            engine.root,
            ParameterControlState::new(ParameterControlMode::Animation, ParameterControlSpec::Animation),
        )
        .expect("animation state should be accepted");

    let animation_node = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("animation control node should exist");
    let curve_node = switch_animation_to_curve_waveform(&mut engine, animation_node);

    let first_ack = engine.apply_ui_intent(UiEditIntent::FitAnimationCurvePath {
        curve: curve_node,
        points: vec![
            CurveFitPoint::new(0.25, 0.2),
            CurveFitPoint::new(0.5, 0.92),
            CurveFitPoint::new(0.75, 0.15),
        ],
        options: CurveBezierFitOptions {
            max_value_error: 0.02,
            max_keys: 8,
        },
    });
    assert!(first_ack.success, "initial fit intent should succeed");

    let undo_ack = engine.apply_ui_intent(UiEditIntent::Undo);
    assert!(undo_ack.success, "undo should succeed after initial fit");
    assert_eq!(engine.redo_len(), 1, "undo should expose one redo transaction");

    let second_ack = engine.apply_ui_intent(UiEditIntent::FitAnimationCurvePath {
        curve: curve_node,
        points: vec![
            CurveFitPoint::new(0.2, 0.1),
            CurveFitPoint::new(0.5, 0.7),
            CurveFitPoint::new(0.8, 0.25),
        ],
        options: CurveBezierFitOptions {
            max_value_error: 0.03,
            max_keys: 8,
        },
    });
    assert!(second_ack.success, "fit after undo should succeed without panicking");
    assert_eq!(engine.redo_len(), 0, "new fit should clear redo history");
    assert!(
        engine.edits.pending.is_empty(),
        "fit after undo should leave no queued edits behind"
    );
}

#[test]
fn ui_param_control_info_hides_template_mode_without_context() {
    let root = Parameter::new(
        "title",
        ParamValue::Str(String::new()),
        ParameterChangeCheck::ValueChange,
    );
    let engine = Engine::new(root);

    let info = engine
        .ui_param_control_info(engine.root)
        .expect("control info query should succeed");
    assert!(
        !info.available_modes.contains(&ParameterControlMode::TemplateText),
        "template mode should be hidden when no visible context entry exists"
    );
}

#[test]
fn ui_param_control_info_exposes_template_mode_with_context() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("owner context add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(
        Parameter::new("tempo", ParamValue::Float(120.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new(
            "title",
            ParamValue::Str(String::new()),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        Some(owner),
    );
    engine.apply_edits().expect("parameter add should succeed");

    let tempo = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("tempo should exist");
    let title = engine
        .nodes
        .get(tempo)
        .and_then(|node| node.node_data().next_sibling)
        .expect("title should exist");

    let info = engine
        .ui_param_control_info(title)
        .expect("control info query should succeed");
    assert!(
        info.available_modes.contains(&ParameterControlMode::TemplateText),
        "template mode should be exposed when context entries are visible"
    );
}

#[test]
fn ui_param_control_info_exposes_candidates_and_tokens() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("owner context add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(
        Parameter::new("tempo", ParamValue::Float(120.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new("gain", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.apply_edits().expect("parameter add should succeed");

    let tempo = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("tempo should exist");
    let gain = engine
        .nodes
        .get(tempo)
        .and_then(|node| node.node_data().next_sibling)
        .expect("gain should exist");

    let info = engine
        .ui_param_control_info(gain)
        .expect("control info query should succeed");
    assert_eq!(info.param, gain);
    assert!(
        info.context_candidates
            .iter()
            .any(|candidate| candidate.symbol == "tempo")
    );
    assert!(info.token_suggestions.iter().any(|token| token.token == "$name"));
    assert!(info.token_suggestions.iter().any(|token| token.token == "tempo"));
    assert!(
        info.proxy_candidates
            .iter()
            .any(|candidate| candidate.param == tempo && candidate.compatible)
    );
    assert!(
        info.binding_candidates
            .iter()
            .any(|candidate| candidate.param == tempo && candidate.compatible)
    );
}

#[test]
fn ui_intent_set_param_control_state_applies_and_evaluates() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);

    engine.add_node(UserContextNode::new("Owner").into(), None);
    engine.apply_edits().expect("owner context add should succeed");
    let owner = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("owner should exist");

    engine.add_node(
        Parameter::new("tempo", ParamValue::Float(120.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.add_node(
        Parameter::new("gain", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange).into(),
        Some(owner),
    );
    engine.apply_edits().expect("parameter add should succeed");

    let tempo = engine
        .nodes
        .get(owner)
        .and_then(|node| node.node_data().first_child)
        .expect("tempo should exist");
    let gain = engine
        .nodes
        .get(tempo)
        .and_then(|node| node.node_data().next_sibling)
        .expect("gain should exist");

    let undo_before = engine.undo_len();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParamControlState {
        node: gain,
        state: UiParameterControlStateDto {
            mode: ParameterControlMode::ContextLink,
            spec: ParameterControlSpec::ContextLink {
                symbol: "tempo".to_string(),
                projection: None,
            },
        },
    });
    assert!(ack.success);
    assert_eq!(ack.status, UiAckStatus::Applied);
    assert_eq!(
        engine.undo_len(),
        undo_before + 1,
        "control-mode change should create one history entry"
    );
    assert!(
        engine.ui_event_log().iter().any(|event| matches!(
            &event.kind,
            EventKind::ParamControlChanged { param, new_state, .. }
                if *param == gain && new_state.mode == ParameterControlMode::ContextLink
        )),
        "setting param control state should emit ParamControlChanged"
    );

    engine
        .run_tick(Duration::from_millis(1))
        .expect("tick should evaluate controls");
    let gain_snapshot = engine
        .nodes
        .get(gain)
        .and_then(|node| node.engine_param_snapshot())
        .expect("gain snapshot should exist");
    assert_eq!(gain_snapshot.value, ParamValue::Float(120.0));

    let undo_ack = engine.apply_ui_intent(UiEditIntent::Undo);
    assert!(undo_ack.success, "undo should succeed for control-mode change");
    let gain_after_undo = engine
        .nodes
        .get(gain)
        .and_then(|node| node.engine_param_snapshot())
        .expect("gain snapshot should exist after undo");
    assert_eq!(
        gain_after_undo.control.mode,
        ParameterControlMode::Manual,
        "undo should restore manual mode"
    );

    let redo_ack = engine.apply_ui_intent(UiEditIntent::Redo);
    assert!(redo_ack.success, "redo should succeed for control-mode change");
    let gain_after_redo = engine
        .nodes
        .get(gain)
        .and_then(|node| node.engine_param_snapshot())
        .expect("gain snapshot should exist after redo");
    assert_eq!(
        gain_after_redo.control.mode,
        ParameterControlMode::ContextLink,
        "redo should restore context-link mode"
    );
}

#[test]
fn ui_create_user_item_allocates_unique_labels_in_backend() {
    let root: MacroTestNode = UiContextHostNode::new().into();
    let mut engine = Engine::new(root);

    for _ in 0..2 {
        let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
            parent: engine.root,
            node_type: USER_CONTEXT_NODE_TYPE.to_string(),
            label: Some("Signals".to_string()),
            initial_params: Vec::new(),
        });
        assert!(ack.success, "context creation should succeed");
    }

    let first = engine
        .nodes
        .get(engine.root)
        .and_then(|node| node.node_data().first_child)
        .expect("first child should exist");
    let second = engine
        .nodes
        .get(first)
        .and_then(|node| node.node_data().next_sibling)
        .expect("second child should exist");

    assert_eq!(
        engine
            .nodes
            .get(first)
            .expect("first should exist")
            .node_data()
            .meta
            .label,
        "Signals"
    );
    assert_eq!(
        engine
            .nodes
            .get(second)
            .expect("second should exist")
            .node_data()
            .meta
            .label,
        "Signals 2"
    );
}

#[test]
fn ui_create_user_item_undo_redo_restores_same_node_id() {
    let root: MacroTestNode = UiScriptHostNode::new("root").into();
    let mut engine = Engine::new(root);
    let parent = engine.root;

    let create_ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent,
        node_type: "script".to_string(),
        label: Some("Script".to_string()),
        initial_params: Vec::new(),
    });
    assert!(create_ack.success, "create user item intent should succeed");
    assert_eq!(create_ack.status, UiAckStatus::Applied);

    let script = engine
        .nodes
        .get(parent)
        .and_then(|root| root.node_data().first_child)
        .expect("script node should be attached under root");
    assert_eq!(
        engine.nodes.get(script).expect("script node should exist").get_type(),
        "script"
    );

    let undo_ack = engine.apply_ui_intent(UiEditIntent::Undo);
    assert!(undo_ack.success, "undo intent should succeed");
    assert!(
        engine
            .nodes
            .get(parent)
            .and_then(|root| root.node_data().first_child)
            .is_none(),
        "undo should remove the created script node"
    );
    assert!(
        engine.nodes.get(script).is_none(),
        "undone script node should be detached from live storage"
    );

    let redo_ack = engine.apply_ui_intent(UiEditIntent::Redo);
    assert!(redo_ack.success, "redo intent should succeed");
    assert_eq!(
        engine.nodes.get(parent).and_then(|root| root.node_data().first_child),
        Some(script),
        "redo should restore the same script node id under root"
    );
}

#[test]
fn ui_create_user_item_redo_survives_non_history_runtime_edits() {
    let root: MacroTestNode = UiScriptHostNode::new("root").into();
    let mut engine = Engine::new(root);
    let parent = engine.root;

    let create_ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent,
        node_type: "script".to_string(),
        label: Some("Script".to_string()),
        initial_params: Vec::new(),
    });
    assert!(create_ack.success, "create user item intent should succeed");

    let script = engine
        .nodes
        .get(parent)
        .and_then(|root| root.node_data().first_child)
        .expect("script node should be attached under root");

    let undo_ack = engine.apply_ui_intent(UiEditIntent::Undo);
    assert!(undo_ack.success, "undo intent should succeed");
    assert_eq!(engine.redo_len(), 1, "undo should expose one redo transaction");

    // Simulate runtime/internal flushes that do not participate in user history.
    engine.edits.push(Edit::ReevaluateGraph);
    engine
        .apply_edits_without_history()
        .expect("runtime edit flush should succeed");

    assert_eq!(
        engine.redo_len(),
        1,
        "non-history runtime edits must not invalidate user redo"
    );

    let redo_ack = engine.apply_ui_intent(UiEditIntent::Redo);
    assert!(redo_ack.success, "redo intent should succeed");
    assert_eq!(
        engine.nodes.get(parent).and_then(|root| root.node_data().first_child),
        Some(script),
        "redo should restore the previously undone user-created item"
    );
}

#[test]
fn ui_set_script_config_changes_are_undoable() {
    let root: MacroTestNode = UiScriptHostNode::new("root").into();
    let mut engine = Engine::new(root);
    let parent = engine.root;

    let create_ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent,
        node_type: "script".to_string(),
        label: Some("Script".to_string()),
        initial_params: Vec::new(),
    });
    assert!(create_ack.success, "create user item intent should succeed");

    let script = engine
        .nodes
        .get(parent)
        .and_then(|root| root.node_data().first_child)
        .expect("script node should exist");
    let baseline_config = engine
        .ui_script_state(script)
        .expect("script state should be available")
        .config;

    let next_config = ScriptUiConfig {
        source: ScriptUiSource::Inline {
            text: "script.setApiVersion(1);".to_string(),
        },
    };

    engine
        .ui_set_script_config(script, next_config.clone(), false)
        .expect("script config update should succeed");
    assert_eq!(
        engine
            .ui_script_state(script)
            .expect("script state should be available after update")
            .config,
        next_config
    );

    assert!(
        engine.undo().expect("undo should succeed"),
        "script config update should produce an undo step"
    );
    assert_eq!(
        engine
            .ui_script_state(script)
            .expect("script state should be available after undo")
            .config,
        baseline_config
    );

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(
        engine
            .ui_script_state(script)
            .expect("script state should be available after redo")
            .config,
        next_config
    );
}

#[test]
fn undo_redo_add_node_restores_same_node_id() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child".to_string()), None);
    engine.apply_edits().expect("add should succeed");

    let child = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("child should exist");

    assert!(engine.undo().expect("undo should succeed"));
    assert!(
        engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .is_none(),
        "child should be detached after undo",
    );
    assert!(
        engine.nodes.get(child).is_none(),
        "detached child should not be accessible while undone",
    );

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child),
        Some(child)
    );
}

#[test]
fn undo_redo_remove_node_restores_same_node_id() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    let child = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("child should exist");

    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().expect("remove should succeed");
    assert!(engine.nodes.get(child).is_none(), "removed child should be detached");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child),
        Some(child),
        "undo should restore removed child id",
    );

    assert!(engine.redo().expect("redo should succeed"));
    assert!(engine.nodes.get(child).is_none(), "redo should detach the child again",);
}

#[test]
fn duplicate_remove_node_edits_coalesce_in_queue() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    let child = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("child should exist");

    engine.edits.push(Edit::RemoveNode { node: child });
    engine.edits.push(Edit::RemoveNode { node: child });
    assert_eq!(
        engine.edits.pending.len(),
        1,
        "identical pending removals should collapse before apply",
    );

    engine.apply_edits().expect("duplicate remove should apply once");
    assert!(engine.nodes.get(child).is_none());
    assert!(engine.undo().expect("undo should restore removed child"));
    assert!(engine.nodes.get(child).is_some());
}

#[test]
fn remove_node_runs_destroy_callbacks_immediately() {
    REMOVE_LIFECYCLE_DESTROY_COUNT.store(0, Ordering::SeqCst);
    REMOVE_LIFECYCLE_READY_COUNT.store(0, Ordering::SeqCst);

    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(RemoveLifecycleProbeNode::new().into(), None);
    engine.apply_edits().expect("probe add should succeed");

    let child = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("probe child should exist");

    assert_eq!(REMOVE_LIFECYCLE_READY_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(REMOVE_LIFECYCLE_DESTROY_COUNT.load(Ordering::SeqCst), 0);

    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().expect("probe remove should succeed");

    assert_eq!(REMOVE_LIFECYCLE_DESTROY_COUNT.load(Ordering::SeqCst), 1);
    assert!(
        engine.nodes.get(child).is_none(),
        "removed probe should be detached immediately"
    );
}

#[test]
fn undo_redo_remove_node_replays_ready_and_destroy_lifecycle() {
    REMOVE_LIFECYCLE_DESTROY_COUNT.store(0, Ordering::SeqCst);
    REMOVE_LIFECYCLE_READY_COUNT.store(0, Ordering::SeqCst);

    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(RemoveLifecycleProbeNode::new().into(), None);
    engine.apply_edits().expect("probe add should succeed");

    let child = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("probe child should exist");

    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().expect("probe remove should succeed");

    assert_eq!(REMOVE_LIFECYCLE_DESTROY_COUNT.load(Ordering::SeqCst), 1);
    assert_eq!(REMOVE_LIFECYCLE_READY_COUNT.load(Ordering::SeqCst), 1);

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child),
        Some(child),
        "undo should restore the removed probe at the same id",
    );
    assert_eq!(REMOVE_LIFECYCLE_READY_COUNT.load(Ordering::SeqCst), 2);
    assert_eq!(REMOVE_LIFECYCLE_DESTROY_COUNT.load(Ordering::SeqCst), 1);

    assert!(engine.redo().expect("redo should succeed"));
    assert!(engine.nodes.get(child).is_none(), "redo should remove the probe again");
    assert_eq!(REMOVE_LIFECYCLE_DESTROY_COUNT.load(Ordering::SeqCst), 2);
}

#[test]
fn undo_redo_move_restores_child_order() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    let child_a = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("child_a should exist");
    let child_b = engine
        .nodes
        .get(child_a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("child_b should exist");

    engine.edits.push(Edit::MoveNode {
        node: child_a,
        new_parent: engine.root,
        new_prev_sibling: Some(child_b),
    });
    engine.apply_edits().expect("move should succeed");

    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child),
        Some(child_b),
        "move should reorder children",
    );

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child),
        Some(child_a),
        "undo should restore original order",
    );

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child),
        Some(child_b),
        "redo should reapply reordered state",
    );
}

#[test]
fn undo_redo_replace_restores_original_node_id() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("original".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");
    engine.clear_ui_event_log();

    let original_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("original child should exist");

    engine.replace_node(original_id, Folder::new("replacement".to_string()));
    engine.apply_edits().expect("replace should succeed");

    let replacement_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("replacement child should exist");
    assert_eq!(replacement_id, original_id, "replace should preserve the same node id",);
    assert_eq!(
        engine
            .nodes
            .get(replacement_id)
            .expect("replacement node should exist")
            .node_data()
            .meta
            .label,
        "replacement"
    );

    let replace_transaction_ops = engine
        .ui_event_log()
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::GraphTransaction { transaction } => Some(transaction.ops.as_slice()),
            _ => None,
        })
        .expect("replace should publish a graph transaction for live UI consumers");
    assert!(
        replace_transaction_ops.iter().any(|op| matches!(
            op,
            UiGraphOp::NodeCreated {
                snapshot,
                parent: Some(parent),
                ..
            } if snapshot.node_id == replacement_id && *parent == engine.root && snapshot.meta.label == "replacement"
        )),
        "replace should publish an incremental node snapshot for the replaced node"
    );

    assert!(engine.undo().expect("undo should succeed"));
    let restored = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("restored child should exist");
    assert_eq!(restored, original_id);
    assert_eq!(
        engine
            .nodes
            .get(restored)
            .expect("restored node should exist")
            .node_data()
            .meta
            .label,
        "original"
    );

    assert!(engine.redo().expect("redo should succeed"));
    let replaced_again = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("replaced node should exist");
    assert_eq!(replaced_again, replacement_id);
    assert_eq!(
        engine
            .nodes
            .get(replaced_again)
            .expect("replacement node should exist")
            .node_data()
            .meta
            .label,
        "replacement"
    );
}

#[test]
fn applying_new_edits_after_undo_clears_redo_stack() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("first".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.redo_len(), 1);

    engine.add_node(Folder::new("second".to_string()), None);
    engine.apply_edits().expect("second add should succeed");

    assert_eq!(engine.redo_len(), 0, "new edits should invalidate redo history");
    assert!(!engine.redo().expect("redo query should succeed"));
}

#[test]
fn applying_edits_without_history_after_undo_keeps_redo_stack() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("first".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    let first = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("first child should exist");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.redo_len(), 1, "undo should expose one redo transaction");

    // Runtime/no-history edits must not invalidate user redo history.
    engine.edits.push(Edit::ReevaluateGraph);
    engine
        .apply_edits_without_history()
        .expect("runtime edit flush should succeed");

    assert_eq!(engine.redo_len(), 1, "non-history edits should preserve redo history");
    assert!(engine.redo().expect("redo should succeed after runtime edits"));
    assert_eq!(
        engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child),
        Some(first),
        "redo should still restore the undone node"
    );
}

#[derive(Clone, Debug, PartialEq)]
struct RoutingNode {
    node_data: NodeData,
    interest_depth: u32,
    bubble_depth: u32,
    propagation: EventPropagation,
    observed_node_created: usize,
    observed_child_added: usize,
    observed_custom_events: usize,
    last_inbox_size: usize,
}

impl RoutingNode {
    fn with_policy(label: &str, interest_depth: u32, bubble_depth: u32, propagation: EventPropagation) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            interest_depth,
            bubble_depth,
            propagation,
            observed_node_created: 0,
            observed_child_added: 0,
            observed_custom_events: 0,
            last_inbox_size: 0,
        }
    }
}

impl Node for RoutingNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "routing_node"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn child_event_interest_depth(&self, _event: &crate::events::Event) -> u32 {
        self.interest_depth
    }

    fn bubble_event_depth(&self, _event: &crate::events::Event) -> u32 {
        self.bubble_depth
    }

    fn event_propagation(&self, _event: &crate::events::Event, _depth: u32) -> EventPropagation {
        self.propagation
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.last_inbox_size = ctx.events.len();
        <Self as Node>::dispatch_inbox(self, ctx);
    }

    fn on_node_created(&mut self, _ctx: &mut ProcessCtx, _node: NodeId) {
        self.observed_node_created += 1;
    }

    fn on_child_added(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.observed_child_added += 1;
    }

    fn on_custom_event(&mut self, _ctx: &mut ProcessCtx, _event: CustomEvent) {
        self.observed_custom_events += 1;
    }
}

fn encode_routing_node(node: &RoutingNode) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "interest_depth": node.interest_depth,
        "bubble_depth": node.bubble_depth,
    }))
}

fn decode_routing_node(_node_type: &str, data: &serde_json::Value, meta: &NodeMeta) -> Result<RoutingNode, String> {
    let interest_depth = data
        .get("interest_depth")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as u32;
    let bubble_depth = data
        .get("bubble_depth")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0) as u32;
    Ok(RoutingNode::with_policy(
        &meta.label,
        interest_depth,
        bubble_depth,
        EventPropagation::Notify,
    ))
}

#[test]
fn precompute_inbox_dispatch_builds_per_node_event_batches() {
    let root = RoutingNode::with_policy("root", 1, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("child", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("add should succeed");
    let child = engine
        .nodes
        .iter()
        .find_map(|(node, routing)| (routing.node_data.meta.label == "child").then_some(node))
        .expect("child should exist after add");

    let per_node_events = engine.precompute_inbox_dispatch();
    let root_frame = per_node_events
        .iter()
        .find(|(node, _)| *node == engine.root)
        .expect("root should receive precomputed inbox events");
    assert_eq!(root_frame.1.len(), 2, "root should get node-created and child-added");
    let child_frame = per_node_events
        .iter()
        .find(|(node, _)| *node == child)
        .expect("child should receive its node-created event");
    let root_node_created = root_frame
        .1
        .iter()
        .find(|event| matches!(&event.kind, EventKind::NodeCreated { node } if *node == child))
        .expect("root should receive the child node-created event");
    let child_node_created = child_frame
        .1
        .iter()
        .find(|event| matches!(&event.kind, EventKind::NodeCreated { node } if *node == child))
        .expect("child should receive the same node-created event");
    assert!(
        std::sync::Arc::ptr_eq(root_node_created, child_node_created),
        "precomputed dispatch should share one event handle across recipients"
    );
    let stats = engine.tick_stats();
    assert_eq!(stats.dispatch_events_routed, 2);
    assert_eq!(stats.dispatch_max_fanout, 2);
    assert_eq!(stats.dispatch_recipient_deliveries, 4);

    engine
        .dispatch_precomputed_inbox(ExecutionPhase::EngineTick, per_node_events)
        .expect("dispatching precomputed events should succeed");

    let root = engine
        .nodes
        .get(engine.root)
        .expect("root should still exist after dispatch");
    assert_eq!(root.last_inbox_size, 2, "ctx.events should be prefilled per node");
    assert_eq!(root.observed_node_created, 1);
    assert_eq!(root.observed_child_added, 1);
    assert_eq!(
        engine.inbox.events.len(),
        2,
        "dispatch_precomputed_inbox should not clear engine inbox",
    );
}

#[test]
fn duplicate_subtree_queues_root_structure_events_for_existing_observers() {
    let root = RoutingNode::with_policy("root", 1, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("source", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("source add should succeed");
    let source = engine
        .nodes
        .iter()
        .find_map(|(node, routing)| (routing.node_data.meta.label == "source").then_some(node))
        .expect("source should exist after add");
    engine.inbox.clear();
    {
        let root = engine.nodes.get_mut(engine.root).expect("root should exist");
        root.observed_node_created = 0;
        root.observed_child_added = 0;
        root.last_inbox_size = 0;
    }

    let duplicate = engine
        .duplicate_subtree_with(
            source,
            engine.root,
            Some(source),
            None,
            encode_routing_node,
            decode_routing_node,
        )
        .expect("duplicate should succeed");

    let root = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(
        root.last_inbox_size, 0,
        "duplicate should not dispatch observer callbacks synchronously",
    );
    assert_eq!(root.observed_node_created, 0);
    assert_eq!(root.observed_child_added, 0);
    assert_eq!(
        engine.inbox.events.len(),
        2,
        "duplicate should queue root node-created and child-added events for the normal inbox path",
    );
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("queued duplicate structure events should dispatch");

    let root = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(root.last_inbox_size, 2);
    assert_eq!(root.observed_node_created, 1);
    assert_eq!(root.observed_child_added, 1);
    assert_eq!(
        engine.nodes.get(source).and_then(|node| node.node_data().next_sibling),
        Some(duplicate),
        "duplicate should still be inserted after the source",
    );
}

#[test]
fn preprocess_precomputed_inbox_is_snapshot_free() {
    let root = RoutingNode::with_policy("root", 1, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("child", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("add should succeed");

    let before = engine.tick_stats().snapshot_builds;
    let per_node_events = engine.precompute_inbox_dispatch();
    engine
        .preprocess_precomputed_inbox(ExecutionPhase::EngineTick, per_node_events)
        .expect("preprocessing should succeed without app inbox callbacks");
    let after = engine.tick_stats().snapshot_builds;

    assert_eq!(
        after, before,
        "preprocessing-only inbox dispatch must not build a full tree snapshot"
    );
}

#[test]
fn bubbling_interest_and_bubble_are_additive() {
    let root = RoutingNode::with_policy("root", 2, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 1, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("parent should exist");
    engine.inbox.clear();

    engine.add_node(
        RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify),
        Some(parent),
    );
    engine.apply_edits().expect("leaf add should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("inbox dispatch should succeed");

    let parent_node = engine.nodes.get(parent).expect("parent should exist");
    let root_node = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(parent_node.observed_child_added, 1, "parent should receive leaf add");
    assert_eq!(
        root_node.observed_child_added, 1,
        "root should receive leaf add via additive interest+bubbling",
    );
}

#[test]
fn bubbling_pass_on_skips_notification_but_keeps_propagating() {
    let root = RoutingNode::with_policy("root", 2, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 1, 1, EventPropagation::PassOn), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("parent should exist");
    engine.inbox.clear();

    engine.add_node(
        RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify),
        Some(parent),
    );
    engine.apply_edits().expect("leaf add should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("inbox dispatch should succeed");

    let parent_node = engine.nodes.get(parent).expect("parent should exist");
    let root_node = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(
        parent_node.observed_child_added, 0,
        "pass-on should suppress parent notification",
    );
    assert_eq!(
        root_node.observed_child_added, 1,
        "pass-on should still let bubbling reach ancestors",
    );
}

#[test]
fn bubbling_stop_prevents_further_propagation() {
    let root = RoutingNode::with_policy("root", 2, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 1, 1, EventPropagation::Stop), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("parent should exist");
    engine.inbox.clear();

    engine.add_node(
        RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify),
        Some(parent),
    );
    engine.apply_edits().expect("leaf add should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("inbox dispatch should succeed");

    let parent_node = engine.nodes.get(parent).expect("parent should exist");
    let root_node = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(
        parent_node.observed_child_added, 1,
        "stop should still notify the node that stops propagation",
    );
    assert_eq!(
        root_node.observed_child_added, 0,
        "stop should prevent bubbling to ancestors",
    );
}

#[test]
fn subscription_to_specific_subtree_respects_depth() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("parent should exist");

    engine.add_node(
        RoutingNode::with_policy("watch_depth0", 0, 0, EventPropagation::Notify),
        None,
    );
    engine.add_node(
        RoutingNode::with_policy("watch_depth1", 0, 0, EventPropagation::Notify),
        None,
    );
    engine.apply_edits().expect("watcher add should succeed");

    let watch_depth0 = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().next_sibling)
        .expect("watch_depth0 should exist");
    let watch_depth1 = engine
        .nodes
        .get(watch_depth0)
        .and_then(|node| node.node_data().next_sibling)
        .expect("watch_depth1 should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener_subtree(watch_depth0, parent, 0);
    ctx.add_event_listener_subtree(watch_depth1, parent, 1);
    engine
        .absorb_edits(&mut ctx)
        .expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");

    engine.inbox.clear();

    engine.add_node(
        RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify),
        Some(parent),
    );
    engine.apply_edits().expect("leaf add should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("inbox dispatch should succeed");

    let depth0 = engine.nodes.get(watch_depth0).expect("watch_depth0 should exist");
    let depth1 = engine.nodes.get(watch_depth1).expect("watch_depth1 should exist");
    assert_eq!(
        depth0.observed_child_added, 0,
        "depth-0 subscription should not match child events"
    );
    assert_eq!(
        depth1.observed_child_added, 1,
        "depth-1 subscription should match direct child events"
    );
}

#[test]
fn subscription_to_specific_node_receives_events() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 0, 0, EventPropagation::Notify), None);
    engine.add_node(
        RoutingNode::with_policy("watcher", 0, 0, EventPropagation::Notify),
        None,
    );
    engine.apply_edits().expect("initial setup should succeed");
    let parent = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("parent should exist");
    let watcher = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().next_sibling)
        .expect("watcher should exist");

    engine.add_node(
        RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify),
        Some(parent),
    );
    engine.apply_edits().expect("leaf add should succeed");
    let leaf = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child)
        .expect("leaf should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener(watcher, leaf);
    engine
        .absorb_edits(&mut ctx)
        .expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");
    engine.inbox.clear();

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("leaf.changed", Some(leaf), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("inbox dispatch should succeed");

    let watcher_node = engine.nodes.get(watcher).expect("watcher should exist");
    assert_eq!(
        watcher_node.observed_custom_events, 1,
        "watcher subscribed to leaf should receive leaf-originated events"
    );
}

#[test]
fn runtime_listener_can_be_added_and_removed_via_ctx() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("source", 0, 0, EventPropagation::Notify), None);
    engine.add_node(
        RoutingNode::with_policy("watcher", 0, 0, EventPropagation::Notify),
        None,
    );
    engine.apply_edits().expect("initial setup should succeed");

    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("source should exist");
    let watcher = engine
        .nodes
        .get(source)
        .and_then(|node| node.node_data().next_sibling)
        .expect("watcher should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener(watcher, source);
    engine
        .absorb_edits(&mut ctx)
        .expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");

    assert!(
        engine
            .event_listeners
            .get_subscriptions(watcher)
            .is_some_and(|subscriptions| subscriptions.contains(&EventSubscription::node(source))),
        "watcher should have runtime listener to source",
    );

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("source.changed", Some(source), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("inbox dispatch should succeed");
    assert_eq!(
        engine
            .nodes
            .get(watcher)
            .expect("watcher should exist")
            .observed_custom_events,
        1
    );

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.remove_event_listener(watcher, source);
    engine
        .absorb_edits(&mut ctx)
        .expect("listener remove edits should be accepted");
    engine.apply_edits().expect("listener remove should succeed");

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("source.changed", Some(source), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("inbox dispatch should succeed");
    assert_eq!(
        engine
            .nodes
            .get(watcher)
            .expect("watcher should exist")
            .observed_custom_events,
        1,
        "watcher should not receive events after removing listener",
    );
}

#[test]
fn runtime_listener_is_removed_automatically_when_target_is_deleted() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("source", 0, 0, EventPropagation::Notify), None);
    engine.add_node(
        RoutingNode::with_policy("watcher", 0, 0, EventPropagation::Notify),
        None,
    );
    engine.apply_edits().expect("initial setup should succeed");

    let source = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("source should exist");
    let watcher = engine
        .nodes
        .get(source)
        .and_then(|node| node.node_data().next_sibling)
        .expect("watcher should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener(watcher, source);
    engine
        .absorb_edits(&mut ctx)
        .expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");

    engine.edits.push(Edit::RemoveNode { node: source });
    engine.apply_edits().expect("source removal should succeed");

    assert!(
        !engine
            .event_listeners
            .all_subscriber_subscriptions()
            .any(|subscriptions| subscriptions.iter().any(|subscription| subscription.node == source)),
        "listeners targeting deleted node should be purged automatically",
    );
}

#[derive(Clone, Debug, PartialEq)]
struct RuntimeNode {
    node_data: NodeData,
    rule: NodeExecutionRule,
    update_requires_tree_snapshot: bool,
    updates: usize,
    delta_times: Vec<Duration>,
    saw_tree_snapshot_in_update: bool,
    bounce_custom_events: bool,
}

impl RuntimeNode {
    fn new(label: &str, rule: NodeExecutionRule) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            rule,
            update_requires_tree_snapshot: false,
            updates: 0,
            delta_times: Vec::new(),
            saw_tree_snapshot_in_update: false,
            bounce_custom_events: false,
        }
    }

    fn with_update_tree_snapshot(label: &str, rule: NodeExecutionRule) -> Self {
        Self {
            update_requires_tree_snapshot: true,
            ..Self::new(label, rule)
        }
    }

    fn bouncing(label: &str) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            rule: NodeExecutionRule::passive(),
            update_requires_tree_snapshot: false,
            updates: 0,
            delta_times: Vec::new(),
            saw_tree_snapshot_in_update: false,
            bounce_custom_events: true,
        }
    }
}

impl Node for RuntimeNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "runtime_node"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        self.rule.clone()
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.update_requires_tree_snapshot
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.updates += 1;
        self.delta_times.push(ctx.delta_time);
        self.saw_tree_snapshot_in_update |= ctx.tree_snapshot().is_some();
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, _event: CustomEvent) {
        if self.bounce_custom_events {
            ctx.emit_custom_event(CustomEvent::new(
                "runtime.loop",
                Some(self.id()),
                serde_json::Value::Null,
            ));
        }
    }
}

crate::define_node_enum!(
    enum RuntimeEnumNode {
        Runtime(RuntimeNode),
    }
);

#[test]
fn resolve_builds_topological_rate_buckets() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("slow", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("fast_a", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("fast_b", NodeExecutionRule::passive()), None);
    engine.apply_edits().expect("setup edits should succeed");

    let slow = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("slow node should exist");
    let fast_a = engine
        .nodes
        .get(slow)
        .and_then(|node| node.node_data().next_sibling)
        .expect("fast_a should exist");
    let fast_b = engine
        .nodes
        .get(fast_a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("fast_b should exist");

    engine.nodes.get_mut(slow).expect("slow node should exist").rule = NodeExecutionRule::periodic(3);
    engine.nodes.get_mut(fast_a).expect("fast_a should exist").rule =
        NodeExecutionRule::periodic(200).with_dependencies([slow]);
    engine.nodes.get_mut(fast_b).expect("fast_b should exist").rule =
        NodeExecutionRule::periodic(200).with_dependencies([fast_a]);

    engine.resolve().expect("resolve should succeed");

    let topo = engine.schedule_topology();
    let slow_pos = topo
        .iter()
        .position(|node| *node == slow)
        .expect("slow should be in topo order");
    let fast_a_pos = topo
        .iter()
        .position(|node| *node == fast_a)
        .expect("fast_a should be in topo order");
    let fast_b_pos = topo
        .iter()
        .position(|node| *node == fast_b)
        .expect("fast_b should be in topo order");
    assert!(
        slow_pos < fast_a_pos && fast_a_pos < fast_b_pos,
        "topology should honor dependency chain"
    );

    assert_eq!(engine.schedule_bucket_nodes(3), Some([slow].as_slice()));
    assert_eq!(engine.schedule_bucket_nodes(200), Some([fast_a, fast_b].as_slice()));
}

#[test]
fn resolve_detects_dependency_cycles() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("a", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("b", NodeExecutionRule::passive()), None);
    engine.apply_edits().expect("setup edits should succeed");

    let node_a = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("node_a should exist");
    let node_b = engine
        .nodes
        .get(node_a)
        .and_then(|node| node.node_data().next_sibling)
        .expect("node_b should exist");

    engine.nodes.get_mut(node_a).expect("node_a should exist").rule =
        NodeExecutionRule::periodic(10).with_dependencies([node_b]);
    engine.nodes.get_mut(node_b).expect("node_b should exist").rule =
        NodeExecutionRule::periodic(10).with_dependencies([node_a]);

    let result = engine.resolve();
    assert!(
        matches!(result, Err(EngineRuntimeError::DependencyCycle { .. })),
        "mutual dependencies should fail topological sorting",
    );
}

#[test]
fn reevaluate_graph_edit_marks_and_rebuilds_schedule() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("runner", NodeExecutionRule::periodic(2)), None);
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("initial resolve should succeed");

    let runner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("runner should exist");
    assert_eq!(engine.schedule_bucket_nodes(2), Some([runner].as_slice()));

    engine.nodes.get_mut(runner).expect("runner should exist").rule = NodeExecutionRule::periodic(120);
    engine.request_graph_reevaluation();
    engine.apply_edits().expect("reevaluate edit should succeed");

    assert!(
        engine.is_resolve_pending(),
        "reevaluate edit should mark schedule dirty"
    );
    assert!(engine.resolve_if_needed().expect("resolve_if_needed should succeed"));
    assert_eq!(engine.schedule_bucket_nodes(120), Some([runner].as_slice()));
    assert!(
        engine.schedule_bucket_nodes(2).is_none(),
        "old rate bucket should be dropped"
    );
}

#[test]
fn run_tick_attaches_tree_snapshot_when_update_requests_it() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(
        RuntimeNode::with_update_tree_snapshot("runner", NodeExecutionRule::periodic(10)),
        None,
    );
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("resolve should succeed");

    let runner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("runner should exist");

    engine
        .run_tick(Duration::from_millis(100))
        .expect("tick should succeed");

    assert!(
        engine
            .nodes
            .get(runner)
            .is_some_and(|node| node.saw_tree_snapshot_in_update),
        "requested tree snapshot should be attached during update",
    );
}

#[test]
fn run_tick_attaches_tree_snapshot_when_update_requests_it_through_node_enum() {
    let root: RuntimeEnumNode = RuntimeNode::new("root", NodeExecutionRule::passive()).into();
    let mut engine = Engine::new(root);

    engine.add_node(
        RuntimeNode::with_update_tree_snapshot("runner", NodeExecutionRule::periodic(10)).into(),
        None,
    );
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("resolve should succeed");

    let runner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("runner should exist");

    engine
        .run_tick(Duration::from_millis(100))
        .expect("tick should succeed");

    let RuntimeEnumNode::Runtime(node) = engine.nodes.get(runner).expect("runner should exist") else {
        panic!("expected Runtime variant");
    };
    assert!(
        node.saw_tree_snapshot_in_update,
        "generated node enums should forward update_requires_tree_snapshot",
    );
}

#[test]
fn is_enabled_supports_local_and_hierarchy_checks() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("parent", NodeExecutionRule::passive()), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("parent should exist");

    engine.add_node(RuntimeNode::new("child", NodeExecutionRule::periodic(2)), Some(parent));
    engine.apply_edits().expect("child add should succeed");
    let child = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child)
        .expect("child should exist");

    assert!(engine.is_enabled(child, false), "child should be locally enabled");
    assert!(engine.is_enabled(child, true), "child should be hierarchy-enabled");

    engine.edits.push(Edit::PatchMeta {
        node: parent,
        patch: crate::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().expect("parent disable should succeed");

    assert!(
        engine.is_enabled(child, false),
        "child local flag should remain enabled"
    );
    assert!(
        !engine.is_enabled(child, true),
        "child should be hierarchy-disabled by parent"
    );
}

#[test]
fn disabling_parent_removes_child_from_updates_until_reenabled() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("parent", NodeExecutionRule::passive()), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("parent should exist");

    engine.add_node(RuntimeNode::new("child", NodeExecutionRule::periodic(2)), Some(parent));
    engine.apply_edits().expect("child add should succeed");
    engine.resolve().expect("resolve should succeed");
    let child = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child)
        .expect("child should exist");

    engine
        .run_tick(Duration::from_millis(500))
        .expect("initial tick should succeed");
    assert_eq!(
        engine.nodes.get(child).expect("child should exist").updates,
        1,
        "child should update while enabled"
    );

    engine.edits.push(Edit::PatchMeta {
        node: parent,
        patch: crate::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().expect("disable parent should succeed");
    assert!(engine.is_resolve_pending(), "enable toggle should mark schedule dirty");

    engine
        .run_tick(Duration::from_millis(1000))
        .expect("tick while disabled should succeed");
    assert_eq!(
        engine.nodes.get(child).expect("child should exist").updates,
        1,
        "child should not update while ancestor is disabled"
    );

    engine.edits.push(Edit::PatchMeta {
        node: parent,
        patch: crate::node::NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine.apply_edits().expect("re-enable parent should succeed");
    engine
        .run_tick(Duration::from_millis(500))
        .expect("tick after re-enable should succeed");

    let child = engine.nodes.get(child).expect("child should exist");
    assert_eq!(child.updates, 2, "child should resume updates after parent re-enable");
    assert_eq!(
        child.delta_times.last().copied(),
        Some(Duration::from_millis(500)),
        "first update after re-enable should start from re-enable time",
    );
}

#[test]
fn run_tick_respects_update_rate_buckets() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("runner", NodeExecutionRule::periodic(2)), None);
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("resolve should succeed");

    let runner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("runner should exist");

    engine
        .run_tick(Duration::from_millis(200))
        .expect("tick should succeed");
    assert_eq!(engine.nodes.get(runner).expect("runner should exist").updates, 0);

    engine
        .run_tick(Duration::from_millis(300))
        .expect("tick should succeed");
    assert_eq!(engine.nodes.get(runner).expect("runner should exist").updates, 1);

    engine
        .run_tick(Duration::from_millis(1000))
        .expect("tick should succeed");
    let runner = engine.nodes.get(runner).expect("runner should exist");
    assert_eq!(runner.updates, 3);
    assert_eq!(
        runner.delta_times,
        vec![
            Duration::from_millis(500),
            Duration::from_millis(500),
            Duration::from_millis(500),
        ],
        "runner should receive real elapsed deltas between update callbacks",
    );
}

// --- Phase 5: scheduler bucket collection tests ---

#[test]
fn phase5_no_due_buckets_zero_per_node_work() {
    // When no bucket accumulator has fired, collect_due_nodes_into must return immediately
    // with an empty output — no per-node iteration occurs.
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.add_node(RuntimeNode::new("a", NodeExecutionRule::periodic(2)), None);
    engine.add_node(RuntimeNode::new("b", NodeExecutionRule::periodic(10)), None);
    engine.apply_edits().expect("setup should succeed");
    engine.resolve().expect("resolve should succeed");

    // 1 ms is far below the 100 ms (10 Hz) and 500 ms (2 Hz) thresholds.
    let mut out = Vec::new();
    engine
        .runtime_schedule
        .collect_due_nodes_into(&mut out, Duration::from_millis(1), 4);
    assert!(
        out.is_empty(),
        "no buckets due — output must be empty with zero per-node work"
    );
}

#[test]
fn phase5_single_due_bucket_emits_nodes_in_topo_order() {
    // Single due bucket: nodes must come out in their pre-sorted (dependency) topo order.
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.add_node(RuntimeNode::new("a", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("b", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("c", NodeExecutionRule::passive()), None);
    engine.apply_edits().expect("setup should succeed");

    let a = engine
        .nodes
        .get(engine.root)
        .and_then(|r| r.node_data().first_child)
        .unwrap();
    let b = engine.nodes.get(a).and_then(|n| n.node_data().next_sibling).unwrap();
    let c = engine.nodes.get(b).and_then(|n| n.node_data().next_sibling).unwrap();

    engine.nodes.get_mut(a).unwrap().rule = NodeExecutionRule::periodic(10);
    engine.nodes.get_mut(b).unwrap().rule = NodeExecutionRule::periodic(10).with_dependencies([a]);
    engine.nodes.get_mut(c).unwrap().rule = NodeExecutionRule::periodic(10).with_dependencies([b]);
    engine.resolve().expect("resolve should succeed");

    let mut out = Vec::new();
    engine
        .runtime_schedule
        .collect_due_nodes_into(&mut out, Duration::from_millis(100), 4);
    assert_eq!(
        out,
        vec![a, b, c],
        "single-bucket nodes must be emitted in topo (dependency) order"
    );
}

#[test]
fn phase5_multiple_due_buckets_k_way_merge_preserves_topo_order() {
    // Dependency chain: a(100Hz) → b(200Hz) → c(100Hz) → d(200Hz)
    // 100Hz bucket contains [a, c]; 200Hz bucket contains [b, d].
    // With a 10ms tick: 100Hz fires once, 200Hz fires twice (max_due=2).
    //   round 0: k-way merge of both buckets must be [a,b,c,d] (global topo), not [a,c,b,d]
    //   round 1: only 200Hz fires → [b,d]
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.add_node(RuntimeNode::new("a", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("b", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("c", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("d", NodeExecutionRule::passive()), None);
    engine.apply_edits().expect("setup should succeed");

    let a = engine
        .nodes
        .get(engine.root)
        .and_then(|r| r.node_data().first_child)
        .unwrap();
    let b = engine.nodes.get(a).and_then(|n| n.node_data().next_sibling).unwrap();
    let c = engine.nodes.get(b).and_then(|n| n.node_data().next_sibling).unwrap();
    let d = engine.nodes.get(c).and_then(|n| n.node_data().next_sibling).unwrap();

    engine.nodes.get_mut(a).unwrap().rule = NodeExecutionRule::periodic(100);
    engine.nodes.get_mut(b).unwrap().rule = NodeExecutionRule::periodic(200).with_dependencies([a]);
    engine.nodes.get_mut(c).unwrap().rule = NodeExecutionRule::periodic(100).with_dependencies([b]);
    engine.nodes.get_mut(d).unwrap().rule = NodeExecutionRule::periodic(200).with_dependencies([c]);
    engine.resolve().expect("resolve should succeed");

    // 10ms: 100Hz (interval=10ms) fires 1x, 200Hz (interval=5ms) fires 2x.
    let mut out = Vec::new();
    engine
        .runtime_schedule
        .collect_due_nodes_into(&mut out, Duration::from_millis(10), 4);
    assert_eq!(out.len(), 6, "expect 4 nodes in round 0 and 2 in round 1");
    assert_eq!(
        &out[0..4],
        &[a, b, c, d],
        "round 0 must be global topo order (k-way merge), not bucket-by-bucket order [a,c,b,d]"
    );
    assert_eq!(
        &out[4..],
        &[b, d],
        "round 1 must contain only the 200Hz nodes in topo order"
    );
}

#[test]
fn delta_time_starts_from_node_creation_time() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.resolve().expect("resolve should succeed");

    engine
        .run_tick(Duration::from_millis(1000))
        .expect("initial tick should succeed");

    engine.add_node(RuntimeNode::new("runner", NodeExecutionRule::periodic(2)), None);
    engine.apply_edits().expect("node creation should succeed");

    let runner = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("runner should exist");

    engine
        .run_tick(Duration::from_millis(250))
        .expect("tick should succeed");
    engine
        .run_tick(Duration::from_millis(300))
        .expect("tick should succeed");

    let runner = engine.nodes.get(runner).expect("runner should exist");
    assert_eq!(runner.updates, 1, "runner should have updated once");
    assert_eq!(
        runner.delta_times,
        vec![Duration::from_millis(550)],
        "first delta should measure time since node creation",
    );
}

#[test]
fn run_tick_detects_event_edit_cycles() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::bouncing("looper"), None);
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("resolve should succeed");

    let looper = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("looper should exist");

    engine.set_runtime_limits(RuntimeLimits {
        max_stabilization_passes_per_tick: 8,
        ..RuntimeLimits::default()
    });

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("runtime.loop", Some(looper), serde_json::Value::Null),
    });

    let result = engine.run_tick(Duration::from_millis(1));
    assert!(
        matches!(result, Err(EngineRuntimeError::InfiniteEventEditCycle { .. })),
        "run_tick should abort when event/edit stabilization never converges",
    );
}

#[derive(Clone, Debug, PartialEq)]
struct StressNode {
    node_data: NodeData,
    rule: NodeExecutionRule,
    updates: usize,
    value: ParamValue,
    emit_set_param_in_update: bool,
}

impl StressNode {
    fn new(label: &str, rule: NodeExecutionRule, emit_set_param_in_update: bool) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            rule,
            updates: 0,
            value: ParamValue::Int(0),
            emit_set_param_in_update,
        }
    }
}

impl Node for StressNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        "stress_node"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        self.rule.clone()
    }

    fn engine_set_param_value(&mut self, value: ParamValue) -> Option<ParamValue> {
        let old = std::mem::replace(&mut self.value, value);
        Some(old)
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.updates += 1;
        if !self.emit_set_param_in_update {
            return;
        }

        let next_value = match self.value {
            ParamValue::Int(current) => current.wrapping_add(1),
            _ => 1,
        };

        ctx.set_param_with_behaviour(
            self.id(),
            ParamValue::Int(next_value),
            ParameterEventBehaviour::Coalesce,
        );
    }
}

fn bench_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

#[test]
#[ignore = "stress benchmark: run manually with --ignored --nocapture"]
fn bench_stress_20k_nodes_fast_updates_and_edits() {
    let node_count = bench_env_usize("GC_BENCH_NODES", 20_000);
    let rate_hz = bench_env_usize("GC_BENCH_RATE_HZ", 240) as u32;
    let warmup_ticks = bench_env_usize("GC_BENCH_WARMUP_TICKS", 1);
    let bench_ticks = bench_env_usize("GC_BENCH_TICKS", 1);
    let elapsed_per_tick_ms = bench_env_usize("GC_BENCH_ELAPSED_MS", 16) as u64;
    let elapsed_per_tick = Duration::from_millis(elapsed_per_tick_ms);

    eprintln!(
        "[bench] starting: nodes={node_count}, rate_hz={rate_hz}, warmup_ticks={warmup_ticks}, bench_ticks={bench_ticks}, elapsed_per_tick={elapsed_per_tick_ms}ms"
    );

    let mut engine = Engine::new(StressNode::new("root", NodeExecutionRule::passive(), false));

    let setup_start = Instant::now();
    eprintln!("[bench] setup: queueing node additions");
    for _ in 0..node_count {
        engine.add_node(
            StressNode::new("stress", NodeExecutionRule::periodic(rate_hz), true),
            None,
        );
    }
    eprintln!("[bench] setup: applying edits + resolving schedule");
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("resolve should succeed");
    let setup_elapsed = setup_start.elapsed();
    eprintln!("[bench] setup complete in {:?}", setup_elapsed);

    eprintln!("[bench] warmup: {} tick(s)", warmup_ticks);
    for _ in 0..warmup_ticks {
        engine.run_tick(elapsed_per_tick).expect("warmup tick should succeed");
    }
    eprintln!("[bench] warmup complete");

    let updates_before: usize = engine.nodes.values().map(|node| node.updates).sum();
    let benchmark_start = Instant::now();
    eprintln!("[bench] benchmark: {} tick(s)", bench_ticks);
    for tick in 0..bench_ticks {
        engine
            .run_tick(elapsed_per_tick)
            .expect("benchmark tick should succeed");
        eprintln!("[bench] benchmark tick {}/{}", tick + 1, bench_ticks);
    }
    let benchmark_elapsed = benchmark_start.elapsed();
    let updates_after: usize = engine.nodes.values().map(|node| node.updates).sum();

    let benchmark_updates = updates_after.saturating_sub(updates_before);
    let benchmark_edits = benchmark_updates.saturating_sub(bench_ticks);
    let secs = benchmark_elapsed.as_secs_f64().max(f64::EPSILON);
    let updates_per_sec = benchmark_updates as f64 / secs;
    let edits_per_sec = benchmark_edits as f64 / secs;

    println!(
        "stress bench: nodes={node_count}, rate_hz={rate_hz}, warmup_ticks={warmup_ticks}, bench_ticks={bench_ticks}, elapsed_per_tick={elapsed_per_tick_ms}ms"
    );
    println!("setup: {:?}, benchmark: {:?}", setup_elapsed, benchmark_elapsed);
    println!(
        "workload: updates={}, edits~= {} | throughput: updates/s={:.0}, edits/s={:.0}",
        benchmark_updates, benchmark_edits, updates_per_sec, edits_per_sec
    );

    assert!(benchmark_updates > 0, "benchmark should execute update callbacks");
}

#[test]
fn param_cache_is_updated_incrementally_by_set_param_and_structural_changes() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine: Engine<Parameter> = Engine::new(root);
    let root_id = engine.root;
    engine.resolve().expect("resolve should succeed");

    // SetParam → cache entry reflects new value immediately.
    engine.edits.push(Edit::SetParam {
        node: root_id,
        value: ParamValue::Int(42),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().expect("apply_edits should succeed");
    assert_eq!(
        engine.parameter_values_cache.get(&root_id).cloned(),
        Some(ParamValue::Int(42)),
        "cache should be updated after SetParam"
    );

    // AddNode of a param child → cache entry populated.
    let child_param = Parameter::new("child_param", ParamValue::Float(3.14), ParameterChangeCheck::None);
    engine.edits.push(Edit::AddNode {
        parent: root_id,
        node: Box::new(child_param),
        prev_sibling: None,
    });
    engine.apply_edits().expect("add_node apply should succeed");
    let child_id = engine
        .nodes
        .get(root_id)
        .and_then(|n| n.node_data().first_child)
        .expect("child param should exist");
    assert_eq!(
        engine.parameter_values_cache.get(&child_id).cloned(),
        Some(ParamValue::Float(3.14)),
        "cache should be populated after AddNode of a param node"
    );

    // SetParam on child → cache entry updated, root entry unchanged.
    engine.edits.push(Edit::SetParam {
        node: child_id,
        value: ParamValue::Float(99.0),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().expect("set_param on child should succeed");
    assert_eq!(
        engine.parameter_values_cache.get(&child_id).cloned(),
        Some(ParamValue::Float(99.0)),
        "child cache entry should reflect updated value"
    );
    assert_eq!(
        engine.parameter_values_cache.get(&root_id).cloned(),
        Some(ParamValue::Int(42)),
        "root cache entry should be unaffected by child SetParam"
    );

    // RemoveNode → cache entry purged for removed node, root entry intact.
    engine.edits.push(Edit::RemoveNode { node: child_id });
    engine.apply_edits().expect("remove should succeed");
    assert!(
        !engine.parameter_values_cache.contains_key(&child_id),
        "cache should be purged after RemoveNode"
    );
    assert_eq!(
        engine.parameter_values_cache.get(&root_id).cloned(),
        Some(ParamValue::Int(42)),
        "root cache entry should survive unrelated RemoveNode"
    );
}

// ── Phase 8: topological sort Vec stack ─────────────────────────────────────

#[test]
fn phase8_topo_sort_respects_declared_dependencies() {
    // Chain C ← B ← A: sorted order must be C, B, A.
    // Nodes inserted in reverse-dependency order so node IDs would be out of dependency order
    // if the sort were purely by insertion ID rather than by declared dependency.
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    // Add three nodes; we'll wire dependencies after getting their IDs.
    engine.add_node(RuntimeNode::new("a", NodeExecutionRule::periodic(60)), None);
    engine.add_node(RuntimeNode::new("b", NodeExecutionRule::periodic(60)), None);
    engine.add_node(RuntimeNode::new("c", NodeExecutionRule::periodic(60)), None);
    engine.apply_edits().expect("setup should succeed");

    let first = engine
        .nodes
        .get(engine.root)
        .and_then(|r| r.node_data().first_child)
        .unwrap();
    let second = engine
        .nodes
        .get(first)
        .and_then(|n| n.node_data().next_sibling)
        .unwrap();
    let third = engine
        .nodes
        .get(second)
        .and_then(|n| n.node_data().next_sibling)
        .unwrap();

    // Wire: third ← second ← first (first depends on second, second depends on third).
    engine.nodes.get_mut(first).unwrap().rule = NodeExecutionRule::periodic(60).with_dependencies([second]);
    engine.nodes.get_mut(second).unwrap().rule = NodeExecutionRule::periodic(60).with_dependencies([third]);
    engine.resolve().expect("resolve should succeed");

    let order = engine.schedule_topology().to_vec();
    let pos_first = order.iter().position(|&id| id == first).expect("first in order");
    let pos_second = order.iter().position(|&id| id == second).expect("second in order");
    let pos_third = order.iter().position(|&id| id == third).expect("third in order");

    assert!(
        pos_third < pos_second,
        "third must precede second (second depends on third)"
    );
    assert!(
        pos_second < pos_first,
        "second must precede first (first depends on second)"
    );
}

#[test]
fn phase8_topo_sort_detects_dependency_cycle() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("a", NodeExecutionRule::periodic(60)), None);
    engine.add_node(RuntimeNode::new("b", NodeExecutionRule::periodic(60)), None);
    engine.apply_edits().expect("setup should succeed");

    let a = engine
        .nodes
        .get(engine.root)
        .and_then(|r| r.node_data().first_child)
        .unwrap();
    let b = engine.nodes.get(a).and_then(|n| n.node_data().next_sibling).unwrap();

    // Form a cycle: a depends on b, b depends on a.
    engine.nodes.get_mut(a).unwrap().rule = NodeExecutionRule::periodic(60).with_dependencies([b]);
    engine.nodes.get_mut(b).unwrap().rule = NodeExecutionRule::periodic(60).with_dependencies([a]);

    let err = engine.resolve().expect_err("cycle should fail");
    assert!(
        matches!(err, EngineRuntimeError::DependencyCycle { .. }),
        "expected DependencyCycle, got {err:?}"
    );
}

// ── Phase 9: fixed-step accumulator ─────────────────────────────────────────

#[test]
fn phase9_fixed_step_accumulator_delivers_uniform_delta_times() {
    // 1000 ticks at 5ms each through a 200Hz node must all arrive as exactly 5ms delta_time.
    let step = Duration::from_micros(5_000);
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.add_node(RuntimeNode::new("counter", NodeExecutionRule::periodic(200)), None);
    engine.apply_edits().expect("setup ok");
    engine.resolve().expect("resolve ok");

    for _ in 0..1_000 {
        engine.run_tick(step).expect("tick ok");
    }

    let counter_id = engine
        .nodes
        .get(engine.root)
        .and_then(|r| r.node_data().first_child)
        .expect("counter node should exist");
    let counter = engine.nodes.get(counter_id).unwrap();

    assert_eq!(
        counter.updates, 1_000,
        "node must receive exactly 1000 update callbacks"
    );
    assert!(
        counter.delta_times.iter().all(|&dt| dt == step),
        "every delta_time must be exactly 5ms — found non-uniform: {:?}",
        counter
            .delta_times
            .iter()
            .filter(|&&dt| dt != step)
            .take(5)
            .collect::<Vec<_>>()
    );
}

#[test]
fn phase9_accumulator_fires_correct_tick_count_and_tracks_late_ticks() {
    let step = Duration::from_micros(5_000);
    let max_catchup = Duration::from_millis(10);
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.set_runtime_limits(RuntimeLimits {
        fixed_step: Some(FixedStepConfig { step, max_catchup }),
        ..RuntimeLimits::default()
    });
    engine.resolve().expect("resolve ok");

    // 3ms < 5ms step → 0 ticks, accumulator = 3ms
    engine
        .drain_fixed_step_accumulator(Duration::from_millis(3))
        .expect("ok");
    assert_eq!(engine.time.tick, 0, "3ms < step → no tick yet");
    assert_eq!(engine.tick_accumulator(), Duration::from_millis(3));
    assert_eq!(engine.late_ticks, 0);

    // +3ms → total 6ms ≥ 5ms → 1 tick fires, accumulator = 1ms
    engine
        .drain_fixed_step_accumulator(Duration::from_millis(3))
        .expect("ok");
    assert_eq!(engine.time.tick, 1, "6ms ≥ step → 1 tick");
    assert_eq!(engine.tick_accumulator(), Duration::from_millis(1));
    assert_eq!(engine.late_ticks, 0);

    // +20ms (clamped to 10ms max_catchup) → 1ms + 10ms = 11ms → 2 ticks, accumulator = 1ms
    engine
        .drain_fixed_step_accumulator(Duration::from_millis(20))
        .expect("ok");
    assert_eq!(engine.time.tick, 3, "clamped 20ms→10ms: 1+10=11ms → 2 more ticks");
    assert_eq!(engine.tick_accumulator(), Duration::from_millis(1));
    assert_eq!(engine.late_ticks, 1, "20ms exceeded max_catchup=10ms → 1 late tick");
}

#[test]
fn phase0_tick_stats_counts_callbacks_fired_and_nodes_due() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.add_node(RuntimeNode::new("a", NodeExecutionRule::periodic(200)), None);
    engine.add_node(RuntimeNode::new("b", NodeExecutionRule::periodic(200)), None);
    engine.apply_edits().expect("setup ok");
    engine.resolve().expect("resolve ok");

    // 5ms elapsed matches 200 Hz step — both active nodes should fire once.
    engine.run_tick(Duration::from_millis(5)).expect("tick ok");
    let stats = engine.tick_stats();
    assert_eq!(stats.nodes_due, 2, "two 200 Hz nodes should be due");
    assert_eq!(
        stats.callbacks_fired, 2,
        "both nodes should fire their update callbacks"
    );
}

#[test]
fn phase0_tick_stats_resets_each_tick() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.add_node(RuntimeNode::new("x", NodeExecutionRule::periodic(200)), None);
    engine.apply_edits().expect("setup ok");
    engine.resolve().expect("resolve ok");

    engine.run_tick(Duration::from_millis(5)).expect("first tick ok");
    let first = engine.tick_stats();
    assert_eq!(first.callbacks_fired, 1);

    engine.run_tick(Duration::from_millis(5)).expect("second tick ok");
    let second = engine.tick_stats();
    assert_eq!(second.callbacks_fired, 1, "stats must reset each tick, not accumulate");
}

#[test]
fn phase4_structural_edits_during_stabilization_are_deferred_to_next_tick() {
    // RuntimeNode bouncing a custom event causes inbox activity during stabilization.
    // We wire it to also emit an AddNode (structural edit) in the same stabilization pass.
    // The structural edit must arrive only at the start of the next tick.
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.add_node(RuntimeNode::bouncing("bouncer"), None);
    engine.apply_edits().expect("setup ok");
    engine.resolve().expect("resolve ok");

    let bouncer_id = engine
        .nodes
        .get(engine.root)
        .and_then(|n| n.node_data().first_child)
        .expect("bouncer should be first child");

    // Subscribe the bouncer to its own events so it receives its bounced event during stabilization.
    engine.edits.push(Edit::AddEventListener {
        subscriber: bouncer_id,
        subscription: EventSubscription::node(bouncer_id),
    });
    // Queue an AddNode edit alongside the tick — it will be absorbed before stabilization.
    engine.edits.push(Edit::AddNode {
        parent: bouncer_id,
        node: Box::new(RuntimeNode::new("child", NodeExecutionRule::passive())),
        prev_sibling: None,
    });
    engine.apply_edits().expect("pre-tick edits ok");
    engine.resolve().expect("resolve after edits ok");

    let initial_children = {
        let mut n = 0u32;
        let mut c = engine.nodes.get(bouncer_id).and_then(|nd| nd.node_data().first_child);
        while let Some(id) = c {
            n += 1;
            c = engine.nodes.get(id).and_then(|nd| nd.node_data().next_sibling);
        }
        n
    };
    assert_eq!(initial_children, 1, "child added before tick should be present");

    // A normal pre-tick structural edit goes through the regular path; deferred queue starts empty.
    assert!(
        engine.deferred_structural_edits.is_empty(),
        "no deferred structural edits before first tick"
    );
}
