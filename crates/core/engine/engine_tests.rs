use super::*;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::edit::Edit;
use crate::events::{CustomEvent, EventKind};
use crate::logger::{self, UI_LOG_CLEARED_TOPIC, UI_LOG_MAX_ENTRIES_TOPIC, UI_LOG_RECORD_TOPIC};
use crate::node::{EventPropagation, EventSubscription, Folder, Node, NodeData, NodeId, NodeMeta, NodeReference, NodeUuid, UserContainerRules, UserNodeRole};
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterConstraintPolicy, ParameterConstraints, ParameterEnumOption, ParameterEventBehaviour};
use crate::process_ctx::{ExecutionPhase, ProcessCtx};
use crate::ui_sync::{UiAckStatus, UiEditIntent, UiNodeDataDto, UiSubscriptionScope};

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

#[test]
fn item_macro_sets_user_item_kind_when_not_overridden() {
    let node = ItemMacroAutoKindNode::new("Auto");
    assert_eq!(node.user_item_kind(), "sequence");
}

#[test]
fn item_macro_keeps_manual_user_item_kind_override() {
    let node = ItemMacroOverrideKindNode::new("Override");
    assert_eq!(node.user_item_kind(), "custom_sequence");
}

#[test]
fn absorb_edits_reports_node_type_mismatch() {
    let root = Folder::new("root".to_string());
    let mut engine = Engine::new(root);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.add_child(NodeId(0), Parameter::new("param", ParamValue::Int(0), ParameterChangeCheck::None), None);

    let result = engine.absorb_edits(&mut ctx);
    assert!(matches!(result, Err(EngineEditError::NodeTypeMismatch { operation: "AddNode", .. })));
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

    engine.absorb_edits(&mut ctx).expect("absorb warning edits should succeed");
    assert!(engine.edits.pending.is_empty(), "no-op warning edits should be dropped during absorb");
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
    assert_eq!(engine.nodes.get(root_id).expect("root parameter should exist").value, ParamValue::Int(33), "external set-param should be drained and applied",);
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

    let child = engine.nodes.get(root_id).and_then(|root| root.node_data().first_child).expect("root should contain one child");
    assert_eq!(engine.nodes.get(child).expect("child node should exist").node_data().meta.label, "external_child");
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

    engine.run_tick(Duration::from_millis(1)).expect("tick should drain and apply external edits");

    assert_eq!(engine.nodes.get(root_id).expect("root parameter should exist").value, ParamValue::Int(7));
    assert_eq!(engine.undo_len(), 0, "runtime tick edits should not create undo entries");
    assert_eq!(engine.redo_len(), 0, "runtime tick edits should not create redo entries");
    assert!(!engine.undo().expect("undo query should succeed"), "runtime-only edits should not be undoable");
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

    engine.absorb_external_edits().expect("external edits should be absorbed");
    assert_eq!(engine.edits.pending.len(), 1, "coalesced external edits should collapse before apply");

    engine.apply_edits().expect("external edits should apply");
    assert_eq!(engine.nodes.get(root_id).expect("root parameter should exist").value, ParamValue::Int(2), "latest coalesced value should win",);
}

#[test]
fn parameter_handle_trigger_value_emits_even_when_unchanged() {
    let mut handle = crate::node::ParameterHandle::<ParamValue>::new(ParamValue::Trigger());
    handle.set_node_id(NodeId(42));

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

    handle.set(&mut ctx, ParamValue::Trigger());

    assert_eq!(ctx.edits.pending.len(), 1, "trigger writes should emit even when value appears unchanged");
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

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

    parameter.set(&mut ctx, ParamValue::Trigger());

    assert_eq!(ctx.edits.pending.len(), 1, "trigger values should bypass value-change dedupe");
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
    #[param(default = 0.5, label = "Decay", description = "Envelope decay time", min = 0.0, max = 1.0, step = 0.05, step_base = 0.0, policy = "ClampAdapt")]
    decay: crate::node::ParameterHandle<f64>,

    #[potential_node(decl_id = "value")]
    value: crate::node::PotentialNodeHandle,
}

struct ViaNodeCore {
    node_data: NodeData,
}

impl ViaNodeCore {
    fn new(label: impl Into<String>) -> Self {
        Self { node_data: NodeData::new(label.into()) }
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
#[params(
    folder(output, label = "Output") {
        host: String = "127.0.0.1" (label = "Host");
    }
)]
struct ReuseFolderBaseNode {}

#[crate::node("reuse_folder_base_node", from_struct)]
impl Node for ReuseFolderBaseNode {}

#[crate::node("reuse_folder_via_node")]
#[params(
    folder(output, label = "Output", reuse = true) {
        gain: f64 = 0.5 [0.0..1.0] (label = "Gain");
    }
)]
struct ReuseFolderViaNode {
    base: ReuseFolderBaseNode,
}

#[crate::node("reuse_folder_via_node", via = base, from_struct)]
impl Node for ReuseFolderViaNode {}

#[crate::node("dsl_params_node")]
#[params(
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

#[crate::node("dsl_enum_defaults_node")]
#[params(
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
#[params(
    target_ref: crate::node::NodeReference (label = "Target Reference");
)]
struct DslReferenceDefaultNode {}

#[crate::node("dsl_meta_params_node")]
#[params(
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
        presentation = crate::node::PresentationHint {
            color: Some(crate::color::Color::new(0.1, 0.2, 0.3, 1.0)),
            ..Default::default()
        },
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
            presentation = crate::node::PresentationHint {
                color: Some(crate::color::Color::new(0.7, 0.8, 0.9, 1.0)),
                ..Default::default()
            },
        );
    }
)]
struct DslMetaParamsNode {}

#[crate::node("manual_inbox_params_node")]
#[params(
    value: f64 = 0.5 [0.0..1.0] (label = "Value");
)]
struct ManualInboxParamsNode {
    observed_inbox_value: Option<f64>,
}

#[crate::node("params_with_custom_init_node")]
#[params(
    value: f64 = 0.5 [0.0..1.0] (label = "Value");
)]
struct ParamsWithCustomInitNode {
    init_calls: usize,
    init_observed_value: Option<f64>,
    init_observed_bound: bool,
    init_observed_id: Option<NodeId>,
}

#[crate::node("nested_init_binding_node")]
#[params(
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
#[params(
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

#[crate::node("dsl_params_node", from_struct)]
impl Node for DslParamsNode {
    fn on_param_change(&mut self, _ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if param == self.feedback.id() {
            self.observed_feedback_old = Some(old_value);
            self.observed_feedback_new = Some(self.feedback.get());
        }
    }
}

#[crate::node("dsl_enum_defaults_node", from_struct)]
impl Node for DslEnumDefaultsNode {}

#[crate::node("dsl_reference_default_node", from_struct)]
impl Node for DslReferenceDefaultNode {}

#[crate::node("dsl_meta_params_node", from_struct)]
impl Node for DslMetaParamsNode {}

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
        DslParamsNode,
        DslEnumDefaultsNode,
        DslReferenceDefaultNode,
        DslMetaParamsNode,
        ManualInboxParamsNode,
        ParamsWithCustomInitNode,
        NestedInitBindingNode,
        DslCallbackParamsNode,
        FieldCallbackParamsNode,
    }
);

#[test]
fn node_struct_macro_declares_param_and_binds_handle_after_child_event() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(AutoDeclaredNode::new("declared").into(), None);

    // First pass adds the node and runs generated init, which queues param creation.
    engine.apply_edits().expect("first apply should succeed");
    // Second pass materializes generated child param nodes.
    engine.apply_edits().expect("second apply should succeed");

    let declared_id = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("declared child should exist");

    let child_added_decl = engine.inbox.events.iter().any(|event| {
        matches!(
            &event.kind,
            EventKind::ChildAdded { parent, decl_id, .. }
                if *parent == declared_id && decl_id.0 == "decay"
        )
    });
    assert!(child_added_decl, "generated param child should emit ChildAdded with decl_id=decay");

    let decay_param = find_child_by_decl(&engine, declared_id, "decay").expect("decay child should exist");
    let decay_meta = engine.nodes.get(decay_param).expect("decay node should exist").node_data().meta.clone();
    assert_eq!(decay_meta.label, "Decay");
    assert_eq!(decay_meta.description.as_deref(), Some("Envelope decay time"));
    let MacroTestNode::Parameter(decay_param_node) = engine.nodes.get(decay_param).expect("decay parameter should exist") else {
        panic!("expected Parameter variant");
    };
    assert_eq!(decay_param_node.constraints.min, Some(0.0));
    assert_eq!(decay_param_node.constraints.max, Some(1.0));
    assert_eq!(decay_param_node.constraints.step, Some(0.05));
    assert_eq!(decay_param_node.constraints.step_base, Some(0.0));
    assert_eq!(decay_param_node.constraints.policy, ParameterConstraintPolicy::ClampAdapt);

    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");

    let MacroTestNode::AutoDeclaredNode(node) = engine.nodes.get(declared_id).expect("declared node should exist") else {
        panic!("expected AutoDeclaredNode variant");
    };

    assert!(node.decay.is_bound(), "generated param handle should be bound after ChildAdded dispatch");
    assert!(!node.value.is_pending_create(), "potential slot should not be pending by default");
}

fn find_child_by_decl(engine: &Engine<MacroTestNode>, parent: NodeId, decl_id: &str) -> Option<NodeId> {
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
    engine.add_node(DslParamsNode::new("dsl", None, None).into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("dsl node should be attached under root");

    let output = find_child_by_decl(&engine, owner, "output").expect("output folder should exist");
    let color = find_child_by_decl(&engine, output, "output/color").expect("output/color folder should exist");
    let feedback = find_child_by_decl(&engine, owner, "feedback").expect("feedback parameter should exist");
    let host = find_child_by_decl(&engine, output, "output/host").expect("output/host parameter should exist");
    let gamma = find_child_by_decl(&engine, color, "output/color/gamma").expect("output/color/gamma parameter should exist");

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

    let feedback_meta = engine.nodes.get(feedback).expect("feedback node should exist").node_data().meta.clone();
    assert_eq!(feedback_meta.label, "Feedback");
    assert_eq!(feedback_meta.description.as_deref(), Some("Delay feedback amount"));
    let MacroTestNode::Parameter(feedback_param) = engine.nodes.get(feedback).expect("feedback parameter should exist") else {
        panic!("expected Parameter variant");
    };
    assert_eq!(feedback_param.constraints.min, Some(0.0));
    assert_eq!(feedback_param.constraints.max, Some(1.0));
    assert_eq!(feedback_param.constraints.step, Some(0.1));
    assert_eq!(feedback_param.constraints.step_base, Some(0.0));
    assert_eq!(feedback_param.constraints.policy, ParameterConstraintPolicy::Reject);
    assert!(feedback_param.read_only);

    let host_meta = engine.nodes.get(host).expect("host node should exist").node_data().meta.clone();
    assert_eq!(host_meta.label, "Host");
    assert_eq!(host_meta.description.as_deref(), Some("OSC destination host"));
}

#[test]
fn params_macro_supports_simple_enum_option_lists_and_default_resolution() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslEnumDefaultsNode::new("enum-defaults").into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("enum defaults node should be attached under root");
    let mode_marked = find_child_by_decl(&engine, owner, "mode_marked").expect("mode_marked parameter should exist");
    let mode_explicit = find_child_by_decl(&engine, owner, "mode_explicit").expect("mode_explicit parameter should exist");
    let mode_first = find_child_by_decl(&engine, owner, "mode_first").expect("mode_first parameter should exist");

    let MacroTestNode::DslEnumDefaultsNode(node) = engine.nodes.get(owner).expect("enum defaults node should exist") else {
        panic!("expected DslEnumDefaultsNode variant");
    };
    assert!(node.mode_marked.is_bound(), "mode_marked handle should be bound");
    assert!(node.mode_explicit.is_bound(), "mode_explicit handle should be bound");
    assert!(node.mode_first.is_bound(), "mode_first handle should be bound");

    let MacroTestNode::Parameter(marked_param) = engine.nodes.get(mode_marked).expect("mode_marked parameter should exist") else {
        panic!("expected Parameter variant");
    };
    assert_eq!(marked_param.value, ParamValue::Enum("auto".to_string()));
    assert_eq!(marked_param.constraints.enum_options.len(), 3);
    assert_eq!(marked_param.constraints.enum_options[0].variant_id, "off");
    assert_eq!(marked_param.constraints.enum_options[1].variant_id, "on");
    assert_eq!(marked_param.constraints.enum_options[2].variant_id, "auto");

    let MacroTestNode::Parameter(explicit_param) = engine.nodes.get(mode_explicit).expect("mode_explicit parameter should exist") else {
        panic!("expected Parameter variant");
    };
    assert_eq!(explicit_param.value, ParamValue::Enum("on".to_string()));

    let MacroTestNode::Parameter(first_param) = engine.nodes.get(mode_first).expect("mode_first parameter should exist") else {
        panic!("expected Parameter variant");
    };
    assert_eq!(first_param.value, ParamValue::Enum("a".to_string()));
}

#[test]
fn params_macro_allows_reference_without_explicit_default_value() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslReferenceDefaultNode::new("ref-default").into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("reference default node should be attached under root");
    let target_ref = find_child_by_decl(&engine, owner, "target_ref").expect("target_ref parameter should exist");

    let MacroTestNode::DslReferenceDefaultNode(node) = engine.nodes.get(owner).expect("reference default node should exist") else {
        panic!("expected DslReferenceDefaultNode variant");
    };
    assert!(node.target_ref.is_bound(), "target_ref handle should be bound");

    let MacroTestNode::Parameter(reference_param) = engine.nodes.get(target_ref).expect("target_ref parameter should exist") else {
        panic!("expected Parameter variant");
    };
    let ParamValue::Reference(reference) = &reference_param.value else {
        panic!("expected ParamValue::Reference");
    };
    assert!(reference.uuid().is_nil(), "reference default should use nil uuid");
    assert_eq!(reference.cached_id(), None, "reference default should have no cached id");
}

#[test]
fn params_macro_applies_metadata_overrides_for_generated_nodes() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslMetaParamsNode::new("meta").into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("dsl meta node should be attached under root");
    let settings = find_child_by_decl(&engine, owner, "settings").expect("settings folder should exist");
    let gain = find_child_by_decl(&engine, settings, "settings/gain").expect("settings/gain parameter should exist");

    let settings_meta = engine.nodes.get(settings).expect("settings folder should exist").node_data().meta.clone();
    assert_eq!(settings_meta.label, "Settings");
    assert_eq!(settings_meta.short_name, "settings_folder");
    assert!(!settings_meta.enabled);
    assert!(settings_meta.can_be_disabled);
    assert_eq!(settings_meta.description.as_deref(), Some("Settings folder metadata"));
    assert_eq!(settings_meta.tags, vec![String::from("group")]);
    assert_eq!(settings_meta.semantics.intent.as_deref(), Some("container"));
    assert_eq!(settings_meta.semantics.unit.as_deref(), Some("section"));
    assert_eq!(settings_meta.presentation.color, Some(crate::color::Color::new(0.1, 0.2, 0.3, 1.0)));

    let gain_meta = engine.nodes.get(gain).expect("gain parameter should exist").node_data().meta.clone();
    assert_eq!(gain_meta.label, "Gain");
    assert_eq!(gain_meta.short_name, "gain_param");
    assert!(!gain_meta.enabled);
    assert!(gain_meta.can_be_disabled);
    assert_eq!(gain_meta.description.as_deref(), Some("Gain parameter metadata"));
    assert_eq!(gain_meta.tags, vec![String::from("audio"), String::from("gain")]);
    assert_eq!(gain_meta.semantics.intent.as_deref(), Some("level"));
    assert_eq!(gain_meta.semantics.unit.as_deref(), Some("db"));
    assert_eq!(gain_meta.presentation.color, Some(crate::color::Color::new(0.7, 0.8, 0.9, 1.0)));
}

#[test]
fn params_macro_syncs_handle_cache_before_on_param_change_callback() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslParamsNode::new("dsl", None, None).into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("dsl node should be attached under root");
    let feedback = find_child_by_decl(&engine, owner, "feedback").expect("feedback parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: feedback,
        value: ParamValue::Float(0.6),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should succeed");
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");

    let MacroTestNode::DslParamsNode(node) = engine.nodes.get(owner).expect("dsl node should exist") else {
        panic!("expected DslParamsNode variant");
    };

    assert!(node.observed_feedback_new.is_some_and(|value| (value - 0.6).abs() < 1e-9), "on_param_change should observe synced handle cache with new value",);
    assert!(matches!(node.observed_feedback_old, Some(ParamValue::Float(value)) if (value - 0.5).abs() < 1e-9), "on_param_change should still receive previous parameter value",);
}

#[test]
fn params_macro_supports_default_named_and_closure_callbacks() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslCallbackParamsNode::new("callbacks", 0, 0, 0, 0, None, None, None).into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("callback node should be attached under root");
    let default_value = find_child_by_decl(&engine, owner, "default_value").expect("default_value parameter should exist");
    let named_value = find_child_by_decl(&engine, owner, "named_value").expect("named_value parameter should exist");
    let closure_value = find_child_by_decl(&engine, owner, "closure_value").expect("closure_value parameter should exist");

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
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");

    let MacroTestNode::DslCallbackParamsNode(node) = engine.nodes.get(owner).expect("callback node should exist") else {
        panic!("expected DslCallbackParamsNode variant");
    };

    assert_eq!(node.default_callback_calls, 1, "default callback should run once");
    assert_eq!(node.named_callback_calls, 1, "named callback should run once");
    assert_eq!(node.closure_callback_calls, 1, "closure callback should run once");
    assert_eq!(node.on_param_change_calls, 3, "callbacks should not replace on_param_change");
    assert!(matches!(node.default_callback_old, Some(ParamValue::Float(value)) if (value - 0.1).abs() < 1e-9), "default callback should receive previous value",);
    assert!(matches!(node.named_callback_old, Some(ParamValue::Float(value)) if (value - 0.2).abs() < 1e-9), "named callback should receive previous value",);
    assert!(matches!(node.closure_callback_old, Some(ParamValue::Float(value)) if (value - 0.3).abs() < 1e-9), "closure callback should receive previous value",);
}

#[test]
fn field_params_support_default_named_and_closure_callbacks() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(FieldCallbackParamsNode::new("callbacks", 0, 0, 0, 0, None, None, None).into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("callback node should be attached under root");
    let default_value = find_child_by_decl(&engine, owner, "default_value").expect("default_value parameter should exist");
    let named_value = find_child_by_decl(&engine, owner, "named_value").expect("named_value parameter should exist");
    let closure_value = find_child_by_decl(&engine, owner, "closure_value").expect("closure_value parameter should exist");

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
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");

    let MacroTestNode::FieldCallbackParamsNode(node) = engine.nodes.get(owner).expect("callback node should exist") else {
        panic!("expected FieldCallbackParamsNode variant");
    };

    assert_eq!(node.default_callback_calls, 1, "default callback should run once");
    assert_eq!(node.named_callback_calls, 1, "named callback should run once");
    assert_eq!(node.closure_callback_calls, 1, "closure callback should run once");
    assert_eq!(node.on_param_change_calls, 3, "callbacks should not replace on_param_change");
    assert!(matches!(node.default_callback_old, Some(ParamValue::Float(value)) if (value - 0.4).abs() < 1e-9), "default callback should receive previous value",);
    assert!(matches!(node.named_callback_old, Some(ParamValue::Float(value)) if (value - 0.5).abs() < 1e-9), "named callback should receive previous value",);
    assert!(matches!(node.closure_callback_old, Some(ParamValue::Float(value)) if (value - 0.6).abs() < 1e-9), "closure callback should receive previous value",);
}

#[test]
fn engine_preprocesses_inbox_before_custom_on_inbox_logic() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(ManualInboxParamsNode::new("manual", None).into(), None);

    for _ in 0..4 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("manual node should be attached under root");
    let value_param = find_child_by_decl(&engine, owner, "value").expect("value parameter should exist");

    engine.edits.push(Edit::SetParam {
        node: value_param,
        value: ParamValue::Float(0.6),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should succeed");
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");

    let MacroTestNode::ManualInboxParamsNode(node) = engine.nodes.get(owner).expect("manual node should exist") else {
        panic!("expected ManualInboxParamsNode variant");
    };

    assert!(node.observed_inbox_value.is_some_and(|value| (value - 0.6).abs() < 1e-9), "custom on_inbox should observe already-preprocessed handle value",);
}

#[test]
fn params_macro_keeps_init_and_child_interest_overrides_available() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(ParamsWithCustomInitNode::new("custom", 0, None, false, None).into(), None);

    for _ in 0..4 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("custom node should be attached under root");
    let value_param = find_child_by_decl(&engine, owner, "value").expect("value parameter should exist");

    let MacroTestNode::ParamsWithCustomInitNode(node) = engine.nodes.get(owner).expect("custom node should exist") else {
        panic!("expected ParamsWithCustomInitNode variant");
    };

    assert_eq!(node.init_calls, 1, "custom init override should remain active");
    assert_eq!(node.value.id(), value_param, "params preprocessing should still bind handles");
    assert!(node.init_observed_value.is_some_and(|value| (value - 0.5).abs() < 1e-9), "custom init should observe declared default value before app init runs",);
    assert!(node.init_observed_bound, "custom init should observe a bound declared handle");
    assert_eq!(node.init_observed_id, Some(value_param), "custom init should observe the runtime parameter id");
}

#[test]
fn nested_declared_params_are_bound_during_init() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(NestedInitBindingNode::new("nested", 0, false, None).into(), None);

    engine.apply_edits().expect("single apply should materialize nested declarations before init");

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("nested node should be attached under root");
    let group = find_child_by_decl(&engine, owner, "group").expect("group folder should exist");
    let value_param = find_child_by_decl(&engine, group, "group/value").expect("nested parameter should exist");

    let MacroTestNode::NestedInitBindingNode(node) = engine.nodes.get(owner).expect("nested node should exist") else {
        panic!("expected NestedInitBindingNode variant");
    };

    assert_eq!(node.init_calls, 1, "init should run exactly once");
    assert!(node.init_observed_bound, "nested declared parameter should already be bound during init");
    assert_eq!(node.init_observed_id, Some(value_param), "init should observe the concrete nested parameter id");
}

#[test]
fn struct_param_declarations_delegate_wiring_into_impl_node() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(StructDeclaredParamsNode::new("decl", 0, None).into(), None);

    for _ in 0..4 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("node should be attached under root");
    let value_param = find_child_by_decl(&engine, owner, "value").expect("value parameter should exist");

    let MacroTestNode::StructDeclaredParamsNode(node) = engine.nodes.get(owner).expect("node should exist") else {
        panic!("expected StructDeclaredParamsNode variant");
    };

    assert_eq!(node.init_calls, 1, "custom init override should run once");
    assert_eq!(node.value.id(), value_param, "struct-declared param handle should bind to runtime parameter child");
    assert!(node.init_observed_value.is_some_and(|value| (value - 0.5).abs() < 1e-9), "custom init should observe struct-declared default before app init runs",);
}

#[test]
fn struct_param_declarations_with_via_use_composed_node_data() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(ViaStructDeclaredParamsNode::new("decl", ViaNodeCore::new("base"), 0, None).into(), None);

    for _ in 0..4 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("node should be attached under root");
    let value_param = find_child_by_decl(&engine, owner, "value").expect("value parameter should exist");

    let MacroTestNode::ViaStructDeclaredParamsNode(node) = engine.nodes.get(owner).expect("node should exist") else {
        panic!("expected ViaStructDeclaredParamsNode variant");
    };

    assert_eq!(node.base.node_data.id, owner, "via path should be the runtime node identity source");
    assert_eq!(node.value.id(), value_param, "struct-declared param handle should bind using composed node identity");
    assert_eq!(node.init_calls, 1, "custom init override should run once");
    assert!(node.init_observed_value.is_some_and(|value| (value - 0.5).abs() < 1e-9), "custom init should observe struct-declared default before app init runs",);
}

#[test]
fn from_struct_via_composed_nodes_forwards_generated_param_wiring_recursively() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(ViaComposedRootNode::new("composed", ViaComposedMidNode::new("mid", ViaComposedLeafNode::new("leaf"))).into(), None);

    for _ in 0..8 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("node should be attached under root");
    let decl_ids = child_decl_ids(&engine, owner);
    let root_value = find_child_by_decl(&engine, owner, "root_value").expect("root_value parameter should exist");
    let mid_value = find_child_by_decl(&engine, owner, "mid_value").expect("mid_value parameter should exist");
    let leaf_value = find_child_by_decl(&engine, owner, "leaf_value").expect("leaf_value parameter should exist");

    let MacroTestNode::ViaComposedRootNode(node) = engine.nodes.get(owner).expect("node should exist") else {
        panic!("expected ViaComposedRootNode variant");
    };

    assert_eq!(node.root_value.id(), root_value, "root-level generated handle should bind");
    assert_eq!(node.mid.mid_value.id(), mid_value, "mid-level generated handle should bind via delegation");
    assert_eq!(node.mid.leaf.leaf_value.id(), leaf_value, "leaf-level generated handle should bind via recursive delegation");
    assert_eq!(decl_ids, vec!["leaf_value".to_string(), "mid_value".to_string(), "root_value".to_string()], "when using `via`, nested parameters should materialize before outer parameters",);
}

#[test]
fn params_macro_folder_reuse_reuses_via_folder_when_decl_id_matches() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(ReuseFolderViaNode::new("reuse", ReuseFolderBaseNode::new("base")).into(), None);

    for _ in 0..8 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("node should be attached under root");
    let owner_decl_ids = child_decl_ids(&engine, owner);
    let output_count = owner_decl_ids.iter().filter(|decl| decl.as_str() == "output").count();
    assert_eq!(output_count, 1, "folder(reuse = true) should avoid creating a duplicate folder when via already queued the same decl_id");

    let output = find_child_by_decl(&engine, owner, "output").expect("shared output folder should exist");
    let host = find_child_by_decl(&engine, output, "output/host").expect("base param should exist in shared folder");
    let gain = find_child_by_decl(&engine, output, "output/gain").expect("outer param should exist in shared folder");
    let output_decl_ids = child_decl_ids(&engine, output);

    assert_eq!(output_decl_ids.iter().filter(|decl| decl.as_str() == "output/host").count(), 1);
    assert_eq!(output_decl_ids.iter().filter(|decl| decl.as_str() == "output/gain").count(), 1);
    assert_eq!(output_decl_ids, vec!["output/host".to_string(), "output/gain".to_string()], "reused folder children should preserve inner(via) items first and outer items at the end",);

    let MacroTestNode::ReuseFolderViaNode(node) = engine.nodes.get(owner).expect("node should exist") else {
        panic!("expected ReuseFolderViaNode variant");
    };

    assert_eq!(node.base.host.id(), host, "base handle should bind to shared-folder host parameter");
    assert_eq!(node.gain.id(), gain, "outer handle should bind to shared-folder gain parameter");
}

#[test]
fn bound_handle_refreshes_from_runtime_parameter_value_without_param_changed_event() {
    let root: MacroTestNode = Folder::new("root".to_string()).into();
    let mut engine = Engine::new(root);
    engine.add_node(DslParamsNode::new("dsl", None, None).into(), None);

    for _ in 0..6 {
        engine.apply_edits().expect("apply should succeed");
        engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");
    }

    let owner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("dsl node should be attached under root");
    let feedback = find_child_by_decl(&engine, owner, "feedback").expect("feedback parameter should exist");

    let MacroTestNode::Parameter(feedback_param) = engine.nodes.get_mut(feedback).expect("feedback parameter should exist") else {
        panic!("expected Parameter variant");
    };
    feedback_param.value = ParamValue::Float(0.9);

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("owner.ping", Some(owner), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).expect("dispatch should succeed");

    let MacroTestNode::DslParamsNode(node) = engine.nodes.get(owner).expect("dsl node should exist") else {
        panic!("expected DslParamsNode variant");
    };

    assert!((node.feedback.get() - 0.9).abs() < 1e-9, "bound handle should refresh from runtime parameter value before node callbacks",);
}

#[test]
fn apply_edits_adds_children_in_call_order() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);

    engine.apply_edits().expect("apply_edits should succeed");

    let root_data = engine.nodes.get(engine.root).expect("root should exist").node_data();
    let first = root_data.first_child.expect("first child should exist");
    let second = engine.nodes.get(first).and_then(|node| node.node_data().next_sibling).expect("second child should exist");

    assert_eq!(engine.nodes.get(first).expect("first node should exist").node_data().meta.label, "child_a");
    assert_eq!(engine.nodes.get(second).expect("second node should exist").node_data().meta.label, "child_b");
    assert_eq!(engine.nodes.get(second).and_then(|node| node.node_data().next_sibling), None);
}

#[test]
fn apply_edits_move_reorders_children() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);
    engine.apply_edits().expect("initial apply_edits should succeed");

    let child_a = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("child_a should exist");
    let child_b = engine.nodes.get(child_a).and_then(|child| child.node_data().next_sibling).expect("child_b should exist");

    engine.edits.push(Edit::MoveNode {
        node: child_a,
        new_parent: engine.root,
        new_prev_sibling: Some(child_b),
    });
    engine.apply_edits().expect("move should succeed");

    let root_data = engine.nodes.get(engine.root).expect("root should exist").node_data();
    assert_eq!(root_data.first_child, Some(child_b));
    assert_eq!(engine.nodes.get(child_b).and_then(|node| node.node_data().next_sibling), Some(child_a));
    assert_eq!(engine.nodes.get(child_a).and_then(|node| node.node_data().next_sibling), None);
    assert!(
        matches!(
            engine.inbox.events.last().map(|event| &event.kind),
            Some(EventKind::ChildReordered { parent, child }) if *parent == engine.root && *child == child_a
        ),
        "last event should report child reordering",
    );
}

#[test]
fn apply_edits_rejects_cycle_move() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("parent".to_string()), None);
    engine.apply_edits().expect("initial apply should succeed");

    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");

    engine.add_node(Folder::new("child".to_string()), Some(parent));
    engine.apply_edits().expect("second apply should succeed");

    let child = engine.nodes.get(parent).and_then(|node| node.node_data().first_child).expect("child should exist");

    engine.edits.push(Edit::MoveNode {
        node: parent,
        new_parent: child,
        new_prev_sibling: None,
    });

    let result = engine.apply_edits();
    assert!(matches!(result, Err(EngineEditError::CycleDetected { operation: "MoveNode", .. })));
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

    engine.add_node(ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]), None);
    engine.apply_edits().expect("container setup should succeed");

    let manager = engine.nodes.get(engine.root).and_then(|node| node.node_data().first_child).expect("manager should exist");

    engine.add_user_item(ContainerTestNode::regular("Sequence 1", "sequence"), Some(manager));
    engine.apply_edits().expect("user item add should succeed");

    let sequence = engine.nodes.get(manager).and_then(|node| node.node_data().first_child).expect("sequence should exist");
    assert_eq!(engine.nodes.get(sequence).expect("sequence should exist").node_data().user_role, UserNodeRole::ItemRoot, "AddUserItem should classify inserted node as item root",);
}

#[test]
fn add_user_item_rejects_kind_when_container_does_not_accept_it() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]), None);
    engine.apply_edits().expect("container setup should succeed");

    let manager = engine.nodes.get(engine.root).and_then(|node| node.node_data().first_child).expect("manager should exist");
    engine.add_user_item(ContainerTestNode::regular("Layer 1", "layer"), Some(manager));

    let result = engine.apply_edits();
    assert!(matches!(result, Err(EngineEditError::UserItemKindRejected { operation: "AddUserItem", .. })));
}

#[test]
fn move_item_root_between_containers_requires_target_acceptance() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(ContainerTestNode::container("Sequences A", "sequence_manager", &["sequence"]), None);
    engine.add_node(ContainerTestNode::container("Sequences B", "sequence_manager", &["layer"]), None);
    engine.apply_edits().expect("container setup should succeed");

    let manager_a = engine.nodes.get(engine.root).and_then(|node| node.node_data().first_child).expect("manager_a should exist");
    let manager_b = engine.nodes.get(manager_a).and_then(|node| node.node_data().next_sibling).expect("manager_b should exist");

    engine.add_user_item(ContainerTestNode::regular("Sequence 1", "sequence"), Some(manager_a));
    engine.apply_edits().expect("initial user item add should succeed");

    let sequence = engine.nodes.get(manager_a).and_then(|node| node.node_data().first_child).expect("sequence should exist");
    engine.edits.push(Edit::MoveNode {
        node: sequence,
        new_parent: manager_b,
        new_prev_sibling: None,
    });

    let rejected = engine.apply_edits();
    assert!(matches!(rejected, Err(EngineEditError::UserItemKindRejected { operation: "MoveNode", .. })));

    engine.add_node(ContainerTestNode::container("Sequences C", "sequence_manager", &["sequence"]), None);
    engine.apply_edits().expect("adding compatible target container should succeed");
    let manager_c = engine.nodes.get(manager_b).and_then(|node| node.node_data().next_sibling).expect("manager_c should exist");

    engine.edits.push(Edit::MoveNode {
        node: sequence,
        new_parent: manager_c,
        new_prev_sibling: None,
    });
    engine.apply_edits().expect("moving item root to compatible container should succeed");

    let sequence_data = engine.nodes.get(sequence).expect("sequence should exist after move").node_data();
    assert_eq!(sequence_data.parent, Some(manager_c));
    assert_eq!(sequence_data.user_role, UserNodeRole::ItemRoot);
}

#[test]
fn move_item_root_outside_any_container_is_rejected() {
    let root = ContainerTestNode::regular("root", "root");
    let mut engine = Engine::new(root);

    engine.add_node(ContainerTestNode::container("Sequences", "sequence_manager", &["sequence"]), None);
    engine.apply_edits().expect("container setup should succeed");

    let manager = engine.nodes.get(engine.root).and_then(|node| node.node_data().first_child).expect("manager should exist");
    engine.add_user_item(ContainerTestNode::regular("Sequence 1", "sequence"), Some(manager));
    engine.apply_edits().expect("initial user item add should succeed");

    let sequence = engine.nodes.get(manager).and_then(|node| node.node_data().first_child).expect("sequence should exist");
    engine.edits.push(Edit::MoveNode {
        node: sequence,
        new_parent: engine.root,
        new_prev_sibling: Some(manager),
    });

    let result = engine.apply_edits();
    assert!(matches!(result, Err(EngineEditError::UserItemContainerRequired { operation: "MoveNode", .. })));
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
fn parameter_set_coalesces_pending_set_param_edits_by_default() {
    let mut parameter = Parameter::new("param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

    parameter.set(&mut ctx, ParamValue::Int(1));
    parameter.set(&mut ctx, ParamValue::Int(2));

    assert_eq!(ctx.edits.pending.len(), 1, "coalesce mode should keep only one queued SetParam");
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

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, EngineTime { tick: 0, micro: 0, seq: 0 });

    parameter.set(&mut ctx, ParamValue::Int(1));
    parameter.set(&mut ctx, ParamValue::Int(2));

    assert_eq!(ctx.edits.pending.len(), 2, "append mode should keep every queued SetParam");
    assert!(
        matches!(ctx.edits.pending.first().map(|request| &request.edit), Some(Edit::SetParam { behaviour: ParameterEventBehaviour::Append, .. })),
        "queued edits should retain append behaviour metadata",
    );
}

#[test]
fn apply_set_param_clamps_value_when_constraints_use_clamp_adapt_policy() {
    let mut root = Parameter::new("root_param", ParamValue::Float(0.0), ParameterChangeCheck::None);
    root.constraints = ParameterConstraints {
        min: Some(0.0),
        max: Some(1.0),
        step: Some(0.25),
        step_base: Some(0.0),
        enum_options: Vec::new(),
        policy: ParameterConstraintPolicy::ClampAdapt,
    };
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Float(1.13),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should clamp and adapt");

    let value = engine.nodes.get(engine.root).expect("root parameter should exist").value.clone();
    assert_eq!(value, ParamValue::Float(1.0), "value should clamp to max after step adaptation");
}

#[test]
fn apply_set_param_rejects_value_when_constraints_use_reject_policy() {
    let mut root = Parameter::new("root_param", ParamValue::Float(0.0), ParameterChangeCheck::None);
    root.constraints = ParameterConstraints {
        min: Some(0.0),
        max: Some(1.0),
        step: Some(0.5),
        step_base: Some(0.0),
        enum_options: Vec::new(),
        policy: ParameterConstraintPolicy::Reject,
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
fn apply_set_param_rejects_values_outside_enum_constraints() {
    let mut root = Parameter::new("mode", ParamValue::Enum("a".to_string()), ParameterChangeCheck::None);
    root.constraints = ParameterConstraints {
        min: None,
        max: None,
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
    let mut root = Parameter::new("mode", ParamValue::Str("legacy_a".to_string()), ParameterChangeCheck::None);
    root.constraints = ParameterConstraints {
        min: None,
        max: None,
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
    };
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Enum("legacy_b".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("enum variant ids should be accepted against legacy string enum values");

    let value = engine.nodes.get(engine.root).expect("root parameter should exist").value.clone();
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
    let value: ParamValue = serde_json::from_value(data.get("value").cloned().ok_or("missing 'value' field")?).map_err(|err| format!("invalid parameter value payload: {err}"))?;
    let change_check: ParameterChangeCheck = serde_json::from_value(data.get("change_check").cloned().ok_or("missing 'change_check' field")?).map_err(|err| format!("invalid change_check payload: {err}"))?;
    let event_behaviour: ParameterEventBehaviour = serde_json::from_value(data.get("event_behaviour").cloned().ok_or("missing 'event_behaviour' field")?).map_err(|err| format!("invalid event_behaviour payload: {err}"))?;

    let mut node = Parameter::new(&meta.label, value, change_check);
    node.event_behaviour = event_behaviour;
    Ok(node)
}

#[test]
fn project_roundtrip_restores_reference_uuid_and_cached_runtime_id() {
    let root = Parameter::new("root", ParamValue::Int(10), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(Parameter::new("target", ParamValue::Float(0.75), ParameterChangeCheck::None), None);
    engine.add_node(Parameter::new("target_ref", ParamValue::Reference(NodeReference::new(NodeUuid(Uuid::new_v4()))), ParameterChangeCheck::None), None);
    engine.apply_edits().expect("initial add should succeed");

    let target = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("target child should exist");
    let target_ref = engine.nodes.get(target).and_then(|node| node.node_data().next_sibling).expect("reference child should exist");
    let target_uuid = engine.nodes.get(target).expect("target node should exist").node_data().meta.uuid;

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");

    let json = engine.to_project_json_with(encode_parameter_node).expect("project serialization should succeed");
    let loaded = Engine::<Parameter>::from_project_json_with(&json, decode_parameter_node).expect("project load should succeed");

    let loaded_target = loaded.nodes.get(loaded.root).and_then(|root| root.node_data().first_child).expect("loaded target child should exist");
    let loaded_target_ref = loaded.nodes.get(loaded_target).and_then(|node| node.node_data().next_sibling).expect("loaded reference child should exist");
    let loaded_target_uuid = loaded.nodes.get(loaded_target).expect("loaded target node should exist").node_data().meta.uuid;

    let loaded_ref_value = &loaded.nodes.get(loaded_target_ref).expect("loaded reference node should exist").value;
    match loaded_ref_value {
        ParamValue::Reference(reference) => {
            assert_eq!(reference.uuid(), loaded_target_uuid);
            assert_eq!(reference.cached_id(), Some(loaded_target));
        }
        other => panic!("expected reference value after load, got {:?}", other),
    }
}

#[test]
fn project_roundtrip_keeps_dangling_reference_uuid_with_empty_cache() {
    let dangling_uuid = NodeUuid(Uuid::new_v4());
    let root = Parameter::new("root", ParamValue::Reference(NodeReference::new(dangling_uuid)), ParameterChangeCheck::None);
    let engine = Engine::new(root);

    let json = engine.to_project_json_with(encode_parameter_node).expect("project serialization should succeed");
    let loaded = Engine::<Parameter>::from_project_json_with(&json, decode_parameter_node).expect("project load should succeed");

    let loaded_root = loaded.nodes.get(loaded.root).expect("loaded root should exist");
    match &loaded_root.value {
        ParamValue::Reference(reference) => {
            assert_eq!(reference.uuid(), dangling_uuid);
            assert_eq!(reference.cached_id(), None);
        }
        other => panic!("expected dangling reference value, got {:?}", other),
    }
}

#[test]
fn project_load_save_load_roundtrip_is_stable() {
    let root = Parameter::new("root", ParamValue::Int(123), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.add_node(Parameter::new("target", ParamValue::Float(0.75), ParameterChangeCheck::None), None);
    engine.add_node(Parameter::new("target_ref", ParamValue::Reference(NodeReference::new(NodeUuid(Uuid::new_v4()))), ParameterChangeCheck::None), None);
    engine.apply_edits().expect("initial add should succeed");

    let target = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("target child should exist");
    let target_ref = engine.nodes.get(target).and_then(|node| node.node_data().next_sibling).expect("reference child should exist");
    let target_uuid = engine.nodes.get(target).expect("target node should exist").node_data().meta.uuid;

    engine.edits.push(Edit::SetParam {
        node: target_ref,
        value: ParamValue::Reference(NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("reference set should succeed");

    let json1 = engine.to_project_json_pretty_with(encode_parameter_node).expect("first project serialization should succeed");
    let loaded1 = Engine::<Parameter>::from_project_json_with(&json1, decode_parameter_node).expect("first project load should succeed");

    let json2 = loaded1.to_project_json_pretty_with(encode_parameter_node).expect("second project serialization should succeed");
    let loaded2 = Engine::<Parameter>::from_project_json_with(&json2, decode_parameter_node).expect("second project load should succeed");

    let json3 = loaded2.to_project_json_pretty_with(encode_parameter_node).expect("third project serialization should succeed");

    let value1: serde_json::Value = serde_json::from_str(&json1).expect("json1 should parse");
    let value2: serde_json::Value = serde_json::from_str(&json2).expect("json2 should parse");
    let value3: serde_json::Value = serde_json::from_str(&json3).expect("json3 should parse");
    assert_eq!(value1, value2, "load-save should preserve full project data");
    assert_eq!(value2, value3, "second load-save should remain stable");

    let loaded2_target = loaded2.nodes.get(loaded2.root).and_then(|root| root.node_data().first_child).expect("loaded2 target child should exist");
    let loaded2_target_ref = loaded2.nodes.get(loaded2_target).and_then(|node| node.node_data().next_sibling).expect("loaded2 reference child should exist");

    match &loaded2.nodes.get(loaded2_target_ref).expect("loaded2 reference node should exist").value {
        ParamValue::Reference(reference) => {
            assert_eq!(reference.uuid(), loaded2.nodes.get(loaded2_target).expect("loaded2 target should exist").node_data().meta.uuid);
            assert_eq!(reference.cached_id(), Some(loaded2_target));
        }
        other => panic!("expected loaded2 reference value, got {:?}", other),
    }
}

#[test]
fn project_serialization_omits_null_and_empty_meta_fields() {
    let root = Parameter::new("root", ParamValue::Int(1), ParameterChangeCheck::None);
    let engine = Engine::new(root);

    let json = engine.to_project_json_with(encode_parameter_node).expect("project serialization should succeed");
    let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");

    let meta = value.get("root").and_then(|root| root.get("meta")).and_then(|meta| meta.as_object()).expect("root.meta should be an object");

    assert!(!meta.contains_key("description"), "null description should be omitted");
    assert!(!meta.contains_key("tags"), "empty tags should be omitted");
    assert!(!meta.contains_key("semantics"), "empty semantics should be omitted");
    assert!(!meta.contains_key("presentation"), "empty presentation should be omitted");
}

#[test]
fn project_serialization_omits_null_data_and_empty_children() {
    let engine = Engine::new(Folder::new("root"));

    let json = engine.to_project_json_with(|_node| Ok(serde_json::Value::Null)).expect("project serialization should succeed");
    let value: serde_json::Value = serde_json::from_str(&json).expect("project json should parse");

    let root = value.get("root").and_then(|root| root.as_object()).expect("root should be an object");

    assert!(!root.contains_key("data"), "null data should be omitted");
    assert!(!root.contains_key("children"), "empty children should be omitted");
}

#[test]
fn emit_custom_event_uses_edit_pipeline() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);

    ctx.emit_custom_event(CustomEvent::new("transport.play", Some(engine.root), serde_json::Value::Null));

    assert!(ctx.events.is_empty(), "custom event should not be injected directly into ctx events");
    assert_eq!(ctx.edits.pending.len(), 1, "custom event should enqueue one edit request");

    engine.absorb_edits(&mut ctx).expect("absorb_edits should accept custom event edits");
    engine.apply_edits().expect("apply_edits should convert custom event edit into engine event");

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

    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Int(42));
    assert_eq!(engine.undo_len(), 1);

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Int(10));
    assert_eq!(engine.redo_len(), 1);

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Int(42));
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

    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Float(0.7));
    assert_eq!(engine.undo_len(), 1, "same-tick coalesced updates should keep one undo step");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Float(0.3), "undo should restore the original value before the first coalesced update",);
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

    assert_eq!(engine.undo_len(), 2, "append mode should keep both updates in undo history");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Float(0.5), "append undo should step back to the immediately previous value",);
}

#[test]
fn begin_end_edit_session_groups_multiple_queue_drains_into_one_undo() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::BeginEditSession {
        origin: crate::edit::EditOrigin::Ui,
        label: Some("Slider drag".to_string()),
        client_edit_id: "drag-1".to_string(),
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
    assert_eq!(engine.undo_len(), 0, "undo entry should not be committed before EndEditSession");

    engine.edits.push(Edit::EndEditSession { client_edit_id: "drag-1".to_string() });
    engine.apply_edits().expect("session end should commit history");

    assert!(!engine.has_active_edit_session());
    assert_eq!(engine.undo_len(), 1, "all session edits should be grouped as one undo step");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Int(0));
}

#[test]
fn clear_history_drops_active_edit_session_state() {
    let root = Parameter::new("root_param", ParamValue::Int(0), ParameterChangeCheck::None);
    let mut engine = Engine::new(root);

    engine.edits.push(Edit::BeginEditSession {
        origin: crate::edit::EditOrigin::Ui,
        label: Some("bootstrap".to_string()),
        client_edit_id: "bootstrap-1".to_string(),
    });
    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(10),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("bootstrap edit session should apply");

    assert!(engine.has_active_edit_session(), "session should remain open before clear");
    assert_eq!(engine.undo_len(), 0, "open session should not commit undo history yet");
    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Int(10));

    engine.clear_history();

    assert!(!engine.has_active_edit_session(), "clear_history should drop active session state");
    assert_eq!(engine.undo_len(), 0);
    assert_eq!(engine.redo_len(), 0);

    engine.edits.push(Edit::EndEditSession { client_edit_id: "bootstrap-1".to_string() });
    let stale_end = engine.apply_edits();
    assert!(matches!(stale_end, Err(EngineEditError::EditSessionNotActive { .. })), "stale session end should fail after clear_history");

    engine.edits.push(Edit::SetParam {
        node: engine.root,
        value: ParamValue::Int(25),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("post-clear edit should apply");
    assert_eq!(engine.undo_len(), 1, "only post-clear edits should be undoable");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Int(10), "undo should restore to the runtime-baseline value, not pre-clear session history",);
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

    let root_meta = &engine.nodes.get(engine.root).expect("root should exist").node_data().meta;
    assert_eq!(root_meta.label, "Renamed Root");
    assert!(!root_meta.enabled);
    assert_eq!(root_meta.description.as_deref(), Some("Updated from UI"));
}

#[test]
fn engine_warning_helpers_replace_clear_and_clear_all() {
    let mut engine = Engine::new(Folder::new("root".to_string()));

    engine.set_node_warning(engine.root, "default warning");
    engine.set_node_warning_with(engine.root, Some("port"), "invalid port", Some("port must be in [1..65535]"));
    engine.set_node_warning(engine.root, "default warning updated");
    engine.apply_edits().expect("warning edits should apply");

    let presentation = &engine.nodes.get(engine.root).expect("root should exist").node_data().meta.presentation;
    assert_eq!(presentation.warnings.len(), 2, "same-id warnings should be replaced");
    assert_eq!(presentation.warning(None).map(|warning| warning.message.as_str()), Some("default warning updated"));
    assert_eq!(presentation.warning(Some("port")).and_then(|warning| warning.detail.as_deref()), Some("port must be in [1..65535]"));

    engine.clear_node_warning(engine.root, Some("port"));
    engine.apply_edits().expect("clear warning should apply");
    let presentation = &engine.nodes.get(engine.root).expect("root should exist").node_data().meta.presentation;
    assert!(presentation.warning(Some("port")).is_none(), "specific warning should be cleared");
    assert!(presentation.warning(None).is_some(), "default warning should remain");

    engine.clear_all_node_warnings(engine.root);
    engine.apply_edits().expect("clear all warnings should apply");
    let presentation = &engine.nodes.get(engine.root).expect("root should exist").node_data().meta.presentation;
    assert!(presentation.warnings.is_empty(), "all warnings should be cleared");
}

#[test]
fn engine_warning_helpers_set_child_warning_depth() {
    let mut engine = Engine::new(Folder::new("root".to_string()));

    engine.set_node_child_warning_depth(engine.root, 3);
    engine.apply_edits().expect("set child warning depth should apply");

    let root_meta = &engine.nodes.get(engine.root).expect("root should exist").node_data().meta;
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

    assert_eq!(engine.ui_event_log().len(), 1, "ui event log should capture emitted events");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("dispatch should succeed");
    assert!(engine.inbox.events.is_empty(), "inbox should be cleared by dispatch");
    assert_eq!(engine.ui_event_log().len(), 1, "ui event log should remain available for replay");
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
            assert_eq!(param.default_value, ParamValue::Float(0.5));
        }
        UiNodeDataDto::Node { .. } => panic!("expected parameter payload for parameter node"),
    }
}

#[test]
fn ui_snapshot_includes_logger_state() {
    logger::clear();
    crate::log!(tag = "tests", level = error; "snapshot logger payload");

    let engine = Engine::new(Folder::new("root".to_string()));
    let snapshot = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);

    assert!(snapshot.logger.max_entries >= 1);
    assert_eq!(snapshot.logger.records.len(), 1);
    assert_eq!(snapshot.logger.records[0].tag, "tests");
    assert_eq!(snapshot.logger.records[0].message, "snapshot logger payload");

    logger::clear();
}

#[test]
fn ui_logger_intents_emit_custom_events() {
    logger::clear();
    let mut engine = Engine::new(Folder::new("root".to_string()));

    let set_ack = engine.apply_ui_intent(UiEditIntent::SetLogMaxEntries { max_entries: 3 });
    assert!(set_ack.success);
    assert_eq!(logger::max_entries(), 3);

    crate::log!("entry");
    assert_eq!(logger::records().len(), 1);

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
    logger::clear();
    let mut engine = Engine::new(Folder::new("root".to_string()));

    crate::log!(origin = engine.root, tag = "runtime"; "pending logger event");
    assert!(engine.ui_event_log().is_empty(), "logger events should flush on runtime tick");

    engine.run_tick(Duration::from_millis(16)).expect("run_tick should succeed");

    let logged = engine.ui_event_log().iter().find(|event| {
        matches!(
            &event.kind,
            EventKind::Custom(custom) if custom.topic == UI_LOG_RECORD_TOPIC
        )
    });
    assert!(logged.is_some(), "run_tick should project pending logger records to ui event log");

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
    assert_eq!(engine.nodes.get(engine.root).expect("root parameter should exist").value, ParamValue::Int(7));
}

#[test]
fn undo_redo_add_node_restores_same_node_id() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child".to_string()), None);
    engine.apply_edits().expect("add should succeed");

    let child = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("child should exist");

    assert!(engine.undo().expect("undo should succeed"));
    assert!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).is_none(), "child should be detached after undo",);
    assert!(engine.nodes.get(child).is_none(), "detached child should not be accessible while undone",);

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child), Some(child));
}

#[test]
fn undo_redo_remove_node_restores_same_node_id() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    let child = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("child should exist");

    engine.edits.push(Edit::RemoveNode { node: child });
    engine.apply_edits().expect("remove should succeed");
    assert!(engine.nodes.get(child).is_none(), "removed child should be detached");

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child), Some(child), "undo should restore removed child id",);

    assert!(engine.redo().expect("redo should succeed"));
    assert!(engine.nodes.get(child).is_none(), "redo should detach the child again",);
}

#[test]
fn undo_redo_move_restores_child_order() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("child_a".to_string()), None);
    engine.add_node(Folder::new("child_b".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    let child_a = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("child_a should exist");
    let child_b = engine.nodes.get(child_a).and_then(|node| node.node_data().next_sibling).expect("child_b should exist");

    engine.edits.push(Edit::MoveNode {
        node: child_a,
        new_parent: engine.root,
        new_prev_sibling: Some(child_b),
    });
    engine.apply_edits().expect("move should succeed");

    assert_eq!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child), Some(child_b), "move should reorder children",);

    assert!(engine.undo().expect("undo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child), Some(child_a), "undo should restore original order",);

    assert!(engine.redo().expect("redo should succeed"));
    assert_eq!(engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child), Some(child_b), "redo should reapply reordered state",);
}

#[test]
fn undo_redo_replace_restores_original_node_id() {
    let mut engine = Engine::new(Folder::new("root".to_string()));
    engine.add_node(Folder::new("original".to_string()), None);
    engine.apply_edits().expect("initial add should succeed");

    let original_id = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("original child should exist");

    engine.replace_node(original_id, Folder::new("replacement".to_string()));
    engine.apply_edits().expect("replace should succeed");

    let replacement_id = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("replacement child should exist");
    assert_eq!(replacement_id, original_id, "replace should preserve the same node id",);
    assert_eq!(engine.nodes.get(replacement_id).expect("replacement node should exist").node_data().meta.label, "replacement");

    assert!(engine.undo().expect("undo should succeed"));
    let restored = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("restored child should exist");
    assert_eq!(restored, original_id);
    assert_eq!(engine.nodes.get(restored).expect("restored node should exist").node_data().meta.label, "original");

    assert!(engine.redo().expect("redo should succeed"));
    let replaced_again = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("replaced node should exist");
    assert_eq!(replaced_again, replacement_id);
    assert_eq!(engine.nodes.get(replaced_again).expect("replacement node should exist").node_data().meta.label, "replacement");
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

#[test]
fn precompute_inbox_dispatch_builds_per_node_event_batches() {
    let root = RoutingNode::with_policy("root", 1, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("child", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("add should succeed");

    let per_node_events = engine.precompute_inbox_dispatch();
    let root_events = per_node_events.iter().find(|(node, _)| *node == engine.root).map(|(_, events)| events.len()).expect("root should receive precomputed inbox events");
    assert_eq!(root_events, 2, "root should get node-created and child-added");

    engine.dispatch_precomputed_inbox(ExecutionPhase::EngineTick, per_node_events).expect("dispatching precomputed events should succeed");

    let root = engine.nodes.get(engine.root).expect("root should still exist after dispatch");
    assert_eq!(root.last_inbox_size, 2, "ctx.events should be prefilled per node");
    assert_eq!(root.observed_node_created, 1);
    assert_eq!(root.observed_child_added, 1);
    assert_eq!(engine.inbox.events.len(), 2, "dispatch_precomputed_inbox should not clear engine inbox",);
}

#[test]
fn bubbling_interest_and_bubble_are_additive() {
    let root = RoutingNode::with_policy("root", 2, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 1, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");
    engine.inbox.clear();

    engine.add_node(RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify), Some(parent));
    engine.apply_edits().expect("leaf add should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");

    let parent_node = engine.nodes.get(parent).expect("parent should exist");
    let root_node = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(parent_node.observed_child_added, 1, "parent should receive leaf add");
    assert_eq!(root_node.observed_child_added, 1, "root should receive leaf add via additive interest+bubbling",);
}

#[test]
fn bubbling_pass_on_skips_notification_but_keeps_propagating() {
    let root = RoutingNode::with_policy("root", 2, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 1, 1, EventPropagation::PassOn), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");
    engine.inbox.clear();

    engine.add_node(RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify), Some(parent));
    engine.apply_edits().expect("leaf add should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");

    let parent_node = engine.nodes.get(parent).expect("parent should exist");
    let root_node = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(parent_node.observed_child_added, 0, "pass-on should suppress parent notification",);
    assert_eq!(root_node.observed_child_added, 1, "pass-on should still let bubbling reach ancestors",);
}

#[test]
fn bubbling_stop_prevents_further_propagation() {
    let root = RoutingNode::with_policy("root", 2, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 1, 1, EventPropagation::Stop), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");
    engine.inbox.clear();

    engine.add_node(RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify), Some(parent));
    engine.apply_edits().expect("leaf add should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");

    let parent_node = engine.nodes.get(parent).expect("parent should exist");
    let root_node = engine.nodes.get(engine.root).expect("root should exist");
    assert_eq!(parent_node.observed_child_added, 1, "stop should still notify the node that stops propagation",);
    assert_eq!(root_node.observed_child_added, 0, "stop should prevent bubbling to ancestors",);
}

#[test]
fn subscription_to_specific_subtree_respects_depth() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("parent add should succeed");
    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");

    engine.add_node(RoutingNode::with_policy("watch_depth0", 0, 0, EventPropagation::Notify), None);
    engine.add_node(RoutingNode::with_policy("watch_depth1", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("watcher add should succeed");

    let watch_depth0 = engine.nodes.get(parent).and_then(|node| node.node_data().next_sibling).expect("watch_depth0 should exist");
    let watch_depth1 = engine.nodes.get(watch_depth0).and_then(|node| node.node_data().next_sibling).expect("watch_depth1 should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener_subtree(watch_depth0, parent, 0);
    ctx.add_event_listener_subtree(watch_depth1, parent, 1);
    engine.absorb_edits(&mut ctx).expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");

    engine.inbox.clear();

    engine.add_node(RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify), Some(parent));
    engine.apply_edits().expect("leaf add should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");

    let depth0 = engine.nodes.get(watch_depth0).expect("watch_depth0 should exist");
    let depth1 = engine.nodes.get(watch_depth1).expect("watch_depth1 should exist");
    assert_eq!(depth0.observed_child_added, 0, "depth-0 subscription should not match child events");
    assert_eq!(depth1.observed_child_added, 1, "depth-1 subscription should match direct child events");
}

#[test]
fn subscription_to_specific_node_receives_events() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("parent", 0, 0, EventPropagation::Notify), None);
    engine.add_node(RoutingNode::with_policy("watcher", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("initial setup should succeed");
    let parent = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("parent should exist");
    let watcher = engine.nodes.get(parent).and_then(|node| node.node_data().next_sibling).expect("watcher should exist");

    engine.add_node(RoutingNode::with_policy("leaf", 0, 0, EventPropagation::Notify), Some(parent));
    engine.apply_edits().expect("leaf add should succeed");
    let leaf = engine.nodes.get(parent).and_then(|node| node.node_data().first_child).expect("leaf should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener(watcher, leaf);
    engine.absorb_edits(&mut ctx).expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");
    engine.inbox.clear();

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("leaf.changed", Some(leaf), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");

    let watcher_node = engine.nodes.get(watcher).expect("watcher should exist");
    assert_eq!(watcher_node.observed_custom_events, 1, "watcher subscribed to leaf should receive leaf-originated events");
}

#[test]
fn runtime_listener_can_be_added_and_removed_via_ctx() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("source", 0, 0, EventPropagation::Notify), None);
    engine.add_node(RoutingNode::with_policy("watcher", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("initial setup should succeed");

    let source = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("source should exist");
    let watcher = engine.nodes.get(source).and_then(|node| node.node_data().next_sibling).expect("watcher should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener(watcher, source);
    engine.absorb_edits(&mut ctx).expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");

    assert!(engine.event_listeners.get(&watcher).is_some_and(|subscriptions| subscriptions.contains(&EventSubscription::node(source))), "watcher should have runtime listener to source",);

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("source.changed", Some(source), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");
    assert_eq!(engine.nodes.get(watcher).expect("watcher should exist").observed_custom_events, 1);

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.remove_event_listener(watcher, source);
    engine.absorb_edits(&mut ctx).expect("listener remove edits should be accepted");
    engine.apply_edits().expect("listener remove should succeed");

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("source.changed", Some(source), serde_json::Value::Null),
    });
    engine.apply_edits().expect("custom event emit should succeed");
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox dispatch should succeed");
    assert_eq!(engine.nodes.get(watcher).expect("watcher should exist").observed_custom_events, 1, "watcher should not receive events after removing listener",);
}

#[test]
fn runtime_listener_is_removed_automatically_when_target_is_deleted() {
    let root = RoutingNode::with_policy("root", 0, 0, EventPropagation::Notify);
    let mut engine = Engine::new(root);

    engine.add_node(RoutingNode::with_policy("source", 0, 0, EventPropagation::Notify), None);
    engine.add_node(RoutingNode::with_policy("watcher", 0, 0, EventPropagation::Notify), None);
    engine.apply_edits().expect("initial setup should succeed");

    let source = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("source should exist");
    let watcher = engine.nodes.get(source).and_then(|node| node.node_data().next_sibling).expect("watcher should exist");

    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.add_event_listener(watcher, source);
    engine.absorb_edits(&mut ctx).expect("listener add edits should be accepted");
    engine.apply_edits().expect("listener add should succeed");

    engine.edits.push(Edit::RemoveNode { node: source });
    engine.apply_edits().expect("source removal should succeed");

    assert!(
        !engine.event_listeners.values().any(|subscriptions| subscriptions.iter().any(|subscription| subscription.node == source)),
        "listeners targeting deleted node should be purged automatically",
    );
}

#[derive(Clone, Debug, PartialEq)]
struct RuntimeNode {
    node_data: NodeData,
    rule: NodeExecutionRule,
    updates: usize,
    delta_times: Vec<Duration>,
    bounce_custom_events: bool,
}

impl RuntimeNode {
    fn new(label: &str, rule: NodeExecutionRule) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            rule,
            updates: 0,
            delta_times: Vec::new(),
            bounce_custom_events: false,
        }
    }

    fn bouncing(label: &str) -> Self {
        Self {
            node_data: NodeData::new(label.to_string()),
            rule: NodeExecutionRule::passive(),
            updates: 0,
            delta_times: Vec::new(),
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

    fn execution_rule(&self) -> NodeExecutionRule {
        self.rule.clone()
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.updates += 1;
        self.delta_times.push(ctx.delta_time);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, _event: CustomEvent) {
        if self.bounce_custom_events {
            ctx.emit_custom_event(CustomEvent::new("runtime.loop", Some(self.id()), serde_json::Value::Null));
        }
    }
}

#[test]
fn resolve_builds_topological_rate_buckets() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("slow", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("fast_a", NodeExecutionRule::passive()), None);
    engine.add_node(RuntimeNode::new("fast_b", NodeExecutionRule::passive()), None);
    engine.apply_edits().expect("setup edits should succeed");

    let slow = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("slow node should exist");
    let fast_a = engine.nodes.get(slow).and_then(|node| node.node_data().next_sibling).expect("fast_a should exist");
    let fast_b = engine.nodes.get(fast_a).and_then(|node| node.node_data().next_sibling).expect("fast_b should exist");

    engine.nodes.get_mut(slow).expect("slow node should exist").rule = NodeExecutionRule::periodic(3);
    engine.nodes.get_mut(fast_a).expect("fast_a should exist").rule = NodeExecutionRule::periodic(200).with_dependencies([slow]);
    engine.nodes.get_mut(fast_b).expect("fast_b should exist").rule = NodeExecutionRule::periodic(200).with_dependencies([fast_a]);

    engine.resolve().expect("resolve should succeed");

    let topo = engine.schedule_topology();
    let slow_pos = topo.iter().position(|node| *node == slow).expect("slow should be in topo order");
    let fast_a_pos = topo.iter().position(|node| *node == fast_a).expect("fast_a should be in topo order");
    let fast_b_pos = topo.iter().position(|node| *node == fast_b).expect("fast_b should be in topo order");
    assert!(slow_pos < fast_a_pos && fast_a_pos < fast_b_pos, "topology should honor dependency chain");

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

    let node_a = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("node_a should exist");
    let node_b = engine.nodes.get(node_a).and_then(|node| node.node_data().next_sibling).expect("node_b should exist");

    engine.nodes.get_mut(node_a).expect("node_a should exist").rule = NodeExecutionRule::periodic(10).with_dependencies([node_b]);
    engine.nodes.get_mut(node_b).expect("node_b should exist").rule = NodeExecutionRule::periodic(10).with_dependencies([node_a]);

    let result = engine.resolve();
    assert!(matches!(result, Err(EngineRuntimeError::DependencyCycle { .. })), "mutual dependencies should fail topological sorting",);
}

#[test]
fn reevaluate_graph_edit_marks_and_rebuilds_schedule() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("runner", NodeExecutionRule::periodic(2)), None);
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("initial resolve should succeed");

    let runner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("runner should exist");
    assert_eq!(engine.schedule_bucket_nodes(2), Some([runner].as_slice()));

    engine.nodes.get_mut(runner).expect("runner should exist").rule = NodeExecutionRule::periodic(120);
    engine.request_graph_reevaluation();
    engine.apply_edits().expect("reevaluate edit should succeed");

    assert!(engine.is_resolve_pending(), "reevaluate edit should mark schedule dirty");
    assert!(engine.resolve_if_needed().expect("resolve_if_needed should succeed"));
    assert_eq!(engine.schedule_bucket_nodes(120), Some([runner].as_slice()));
    assert!(engine.schedule_bucket_nodes(2).is_none(), "old rate bucket should be dropped");
}

#[test]
fn run_tick_respects_update_rate_buckets() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::new("runner", NodeExecutionRule::periodic(2)), None);
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("resolve should succeed");

    let runner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("runner should exist");

    engine.run_tick(Duration::from_millis(200)).expect("tick should succeed");
    assert_eq!(engine.nodes.get(runner).expect("runner should exist").updates, 0);

    engine.run_tick(Duration::from_millis(300)).expect("tick should succeed");
    assert_eq!(engine.nodes.get(runner).expect("runner should exist").updates, 1);

    engine.run_tick(Duration::from_millis(1000)).expect("tick should succeed");
    let runner = engine.nodes.get(runner).expect("runner should exist");
    assert_eq!(runner.updates, 3);
    assert_eq!(runner.delta_times, vec![Duration::from_millis(500), Duration::from_millis(500), Duration::from_millis(500),], "runner should receive real elapsed deltas between update callbacks",);
}

#[test]
fn delta_time_starts_from_node_creation_time() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);
    engine.resolve().expect("resolve should succeed");

    engine.run_tick(Duration::from_millis(1000)).expect("initial tick should succeed");

    engine.add_node(RuntimeNode::new("runner", NodeExecutionRule::periodic(2)), None);
    engine.apply_edits().expect("node creation should succeed");

    let runner = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("runner should exist");

    engine.run_tick(Duration::from_millis(250)).expect("tick should succeed");
    engine.run_tick(Duration::from_millis(300)).expect("tick should succeed");

    let runner = engine.nodes.get(runner).expect("runner should exist");
    assert_eq!(runner.updates, 1, "runner should have updated once");
    assert_eq!(runner.delta_times, vec![Duration::from_millis(550)], "first delta should measure time since node creation",);
}

#[test]
fn run_tick_detects_event_edit_cycles() {
    let root = RuntimeNode::new("root", NodeExecutionRule::passive());
    let mut engine = Engine::new(root);

    engine.add_node(RuntimeNode::bouncing("looper"), None);
    engine.apply_edits().expect("setup edits should succeed");
    engine.resolve().expect("resolve should succeed");

    let looper = engine.nodes.get(engine.root).and_then(|root| root.node_data().first_child).expect("looper should exist");

    engine.set_runtime_limits(RuntimeLimits {
        max_stabilization_passes_per_tick: 8,
        ..RuntimeLimits::default()
    });

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("runtime.loop", Some(looper), serde_json::Value::Null),
    });

    let result = engine.run_tick(Duration::from_millis(1));
    assert!(matches!(result, Err(EngineRuntimeError::InfiniteEventEditCycle { .. })), "run_tick should abort when event/edit stabilization never converges",);
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

        ctx.set_param_with_behaviour(self.id(), ParamValue::Int(next_value), ParameterEventBehaviour::Coalesce);
    }
}

fn bench_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|raw| raw.parse::<usize>().ok()).filter(|value| *value > 0).unwrap_or(default)
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

    eprintln!("[bench] starting: nodes={node_count}, rate_hz={rate_hz}, warmup_ticks={warmup_ticks}, bench_ticks={bench_ticks}, elapsed_per_tick={elapsed_per_tick_ms}ms");

    let mut engine = Engine::new(StressNode::new("root", NodeExecutionRule::passive(), false));

    let setup_start = Instant::now();
    eprintln!("[bench] setup: queueing node additions");
    for _ in 0..node_count {
        engine.add_node(StressNode::new("stress", NodeExecutionRule::periodic(rate_hz), true), None);
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
        engine.run_tick(elapsed_per_tick).expect("benchmark tick should succeed");
        eprintln!("[bench] benchmark tick {}/{}", tick + 1, bench_ticks);
    }
    let benchmark_elapsed = benchmark_start.elapsed();
    let updates_after: usize = engine.nodes.values().map(|node| node.updates).sum();

    let benchmark_updates = updates_after.saturating_sub(updates_before);
    let benchmark_edits = benchmark_updates.saturating_sub(bench_ticks);
    let secs = benchmark_elapsed.as_secs_f64().max(f64::EPSILON);
    let updates_per_sec = benchmark_updates as f64 / secs;
    let edits_per_sec = benchmark_edits as f64 / secs;

    println!("stress bench: nodes={node_count}, rate_hz={rate_hz}, warmup_ticks={warmup_ticks}, bench_ticks={bench_ticks}, elapsed_per_tick={elapsed_per_tick_ms}ms");
    println!("setup: {:?}, benchmark: {:?}", setup_elapsed, benchmark_elapsed);
    println!("workload: updates={}, edits~= {} | throughput: updates/s={:.0}, edits/s={:.0}", benchmark_updates, benchmark_edits, updates_per_sec, edits_per_sec);

    assert!(benchmark_updates > 0, "benchmark should execute update callbacks");
}
