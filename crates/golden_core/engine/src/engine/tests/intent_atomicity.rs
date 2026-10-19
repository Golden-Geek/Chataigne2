use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::{
    edit::NodeTree,
    node::{
        DeclId, Folder, Node, NodeCreationContext, NodeId, NodeReference, ParameterHandle, UserContainerRules,
        UserCreatableItem,
    },
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ReferenceTargetKind},
    process_ctx::ProcessCtx,
    ui_sync::{UiCreateUserItemInitialParam, UiEditIntent},
};

static FAILED_ITEM_DESTROYED: AtomicUsize = AtomicUsize::new(0);
static INVALID_REFERENCE_READY: AtomicUsize = AtomicUsize::new(0);

#[crate::node("creation_factory")]
struct CreationFactory {}

#[crate::node("creation_factory", from_struct)]
impl Node for CreationFactory {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&["creation_item"]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new("creation_item", "creation_item", "Item")]
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        (node_type == "creation_item").then(|| {
            let mut target = Parameter::new(
                "target",
                ParamValue::Reference(NodeReference::default()),
                ParameterChangeCheck::ValueChange,
            );
            target.node_data_mut().meta.decl_id = DeclId("reference/target".into());
            target.constraints.reference.target_kind = ReferenceTargetKind::ParameterOnly;
            NodeTree::new(CreationItem::new(0.0, None)).with_child(NodeTree::new(target))
        })
    }
}

#[crate::node("creation_item")]
struct CreationItem {
    #[param(default = 0.5)]
    first: ParameterHandle<f64>,
    #[param(default = 0.25)]
    second: ParameterHandle<f64>,
    ready_value: f64,
    ready_reference: Option<NodeReference>,
}

#[crate::node("creation_item", from_struct)]
impl Node for CreationItem {
    fn user_item_kind(&self) -> &str {
        "creation_item"
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.first.set(ctx, 7.0);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.ready_value = self.first.get();
        self.ready_reference = ctx.tree_snapshot().and_then(|snapshot| {
            let target = snapshot.resolve_path_from(self.id(), "target")?;
            match snapshot.node(target)?.param_value.as_ref()? {
                ParamValue::Reference(reference) => Some(reference.clone()),
                _ => None,
            }
        });
        if self.node_data().meta.label == "InvalidReference" {
            INVALID_REFERENCE_READY.fetch_add(1, Ordering::SeqCst);
        }
        if self.node_data().meta.label == "Fail" {
            ctx.set_param(NodeId(u64::MAX), ParamValue::Int(1));
        }
        if matches!(
            self.node_data().meta.label.as_str(),
            "ExistingWrites" | "FailExistingWrites"
        ) {
            let root = ctx.tree_snapshot().unwrap().root();
            ctx.patch_node_meta(
                root,
                crate::node::NodeMetaPatch {
                    label: Some("Leaked lifecycle rename".into()),
                    ..Default::default()
                },
            );
            ctx.add_child(root, Folder::new("Leaked lifecycle sibling"), None);
            if self.node_data().meta.label == "FailExistingWrites" {
                ctx.set_param(NodeId(u64::MAX), ParamValue::Int(1));
            }
        }
        if matches!(
            self.node_data().meta.label.as_str(),
            "FailExistingRemove" | "FailExistingMove" | "FailExistingReplace" | "FailNestedOrdering"
        ) {
            let snapshot = ctx.tree_snapshot().unwrap();
            let root = snapshot.root();
            let existing = snapshot
                .child_ids(root)
                .iter()
                .copied()
                .find(|id| snapshot.node(*id).unwrap().label == "Existing")
                .unwrap();
            let edit = match self.node_data().meta.label.as_str() {
                "FailExistingRemove" => crate::edit::Edit::RemoveNode { node: existing },
                "FailExistingMove" => crate::edit::Edit::MoveNode {
                    node: existing,
                    new_parent: self.id(),
                    new_prev_sibling: None,
                },
                "FailExistingReplace" => crate::edit::Edit::ReplaceNode {
                    node: existing,
                    new_node: Box::new(Folder::new("Replacement")),
                },
                _ => crate::edit::Edit::AddNode {
                    node: Box::new(NestedMover::new(existing)),
                    parent: root,
                    prev_sibling: None,
                },
            };
            ctx.edits.push(edit);
            ctx.set_param(NodeId(u64::MAX), ParamValue::Int(1));
        }
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        if self.node_data().meta.label == "Fail" {
            FAILED_ITEM_DESTROYED.fetch_add(1, Ordering::SeqCst);
        }
    }
}

#[crate::node("nested_mover")]
struct NestedMover {
    existing: NodeId,
}

#[crate::node("nested_mover", from_struct)]
impl Node for NestedMover {
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        ctx.edits.push(crate::edit::Edit::MoveNode {
            node: self.existing,
            new_parent: self.id(),
            new_prev_sibling: None,
        });
    }
}

crate::define_node_enum!(
    enum CreationTestNode {
        CreationFactory,
        CreationItem,
        NestedMover,
    }
);

fn initializer(decl_id: &str, value: f64) -> UiCreateUserItemInitialParam {
    UiCreateUserItemInitialParam {
        decl_id: DeclId(decl_id.into()),
        value: ParamValue::Float(value),
    }
}

fn create(
    engine: &Engine<CreationTestNode>,
    label: &str,
    initial_params: Vec<UiCreateUserItemInitialParam>,
) -> UiEditIntent {
    UiEditIntent::CreateUserItem {
        parent: engine.root,
        node_type: "creation_item".into(),
        label: Some(label.into()),
        initial_params,
    }
}

fn assert_rejected_without_mutation(engine: &mut Engine<CreationTestNode>, intent: UiEditIntent) {
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    let history = engine.ui_history_state();
    let events = engine.ui_event_log().to_vec();
    let graph_version = engine.ui_graph_version;
    let dirty = engine.project_dirty_nodes.clone();
    let ack = engine.apply_ui_intent(intent);
    assert!(!ack.success, "intent unexpectedly succeeded");
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(engine.ui_event_log(), events);
    assert_eq!(engine.ui_graph_version, graph_version);
    assert_eq!(engine.project_dirty_nodes, dirty);
    assert!(engine.edits.pending.is_empty());
}

#[test]
fn create_rejects_invalid_first_middle_and_last_initializer_before_commit() {
    for active_session in [false, true] {
        for bad_index in 0..3 {
            let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
            engine.add_node(Folder::new("undoable").into(), None);
            engine.apply_edits().unwrap();
            engine.undo().unwrap();
            if active_session {
                assert!(
                    engine
                        .apply_ui_intent(UiEditIntent::BeginEdit {
                            client_edit_id: "outer".into(),
                            label: None
                        })
                        .success
                );
            }
            let mut values = vec![
                initializer("first", 1.0),
                initializer("second", 2.0),
                initializer("first", 3.0),
            ];
            values[bad_index] = initializer("missing", 9.0);
            let intent = create(&engine, "Rejected", values);
            assert_rejected_without_mutation(&mut engine, intent);
            assert_eq!(engine.has_active_edit_session(), active_session);
            assert_eq!(engine.redo_len(), 1);
            if active_session {
                assert!(
                    engine
                        .apply_ui_intent(UiEditIntent::EndEdit {
                            client_edit_id: "outer".into()
                        })
                        .success
                );
            }
            assert!(engine.redo().unwrap());
            assert_eq!(engine.nodes.len(), 2);
            engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
            engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
        }
    }
}

#[test]
fn create_ready_failure_restores_history_dirty_publication_and_releases_item() {
    FAILED_ITEM_DESTROYED.store(0, Ordering::SeqCst);
    let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
    let intent = create(&engine, "Fail", vec![initializer("first", 3.5)]);
    assert_rejected_without_mutation(&mut engine, intent);
    assert_eq!(FAILED_ITEM_DESTROYED.load(Ordering::SeqCst), 1);
    assert_eq!(engine.nodes.len(), 1);
    let intent = create(&engine, "Next", vec![initializer("first", 3.5)]);
    assert!(engine.apply_ui_intent(intent).success);
    assert!(engine.undo().unwrap());
    assert_eq!(engine.nodes.len(), 1);
}

#[test]
fn rejected_creation_rolls_back_existing_metadata_and_structure_from_ready() {
    let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
    engine.add_node(Folder::new("Undoable").into(), None);
    engine.apply_edits().unwrap();
    engine.undo().unwrap();
    let intent = create(&engine, "FailExistingWrites", Vec::new());
    assert_rejected_without_mutation(&mut engine, intent);
    assert_eq!(engine.redo_len(), 1);
    assert!(engine.redo().unwrap());
    engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
    engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
}

#[test]
fn accepted_existing_lifecycle_writes_share_one_undo_and_redo_without_repetition() {
    let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    let intent = create(&engine, "ExistingWrites", Vec::new());
    assert!(engine.apply_ui_intent(intent).success);
    let after = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert!(engine.redo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        after
    );
}

#[test]
fn provisional_removal_move_replace_and_nested_callbacks_restore_existing_identity() {
    for label in [
        "FailExistingRemove",
        "FailExistingMove",
        "FailExistingReplace",
        "FailNestedOrdering",
    ] {
        let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
        engine.add_node(Folder::new("Existing").into(), None);
        engine.apply_edits().unwrap();
        let existing = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
        let uuid = engine.nodes.get(existing).unwrap().node_data().meta.uuid;
        let blueprint_id = crate::blueprints::BlueprintId::new("existing_blueprint");
        let blueprint_meta = crate::blueprints::BlueprintInstanceMeta::new(
            blueprint_id.clone(),
            3,
            [(DeclId("existing".into()), existing)].into(),
        );
        engine.blueprints.register_instance(existing, blueprint_meta.clone());
        engine.add_node(Folder::new("Undoable").into(), None);
        engine.apply_edits().unwrap();
        engine.undo().unwrap();
        engine
            .event_listeners
            .add(engine.root, crate::node::EventSubscription::node(existing));
        let intent = create(&engine, label, Vec::new());
        assert_rejected_without_mutation(&mut engine, intent);
        assert_eq!(
            engine.event_listeners.listeners_for_origin(existing),
            &[(engine.root, 0)],
            "{label}"
        );
        assert_eq!(engine.node_id_by_uuid(uuid), Some(existing), "{label}");
        assert_eq!(
            engine.blueprint_instance_meta(existing),
            Some(&blueprint_meta),
            "{label}"
        );
        assert_eq!(
            engine.blueprints.instance_roots_for(&blueprint_id),
            &[existing],
            "{label}"
        );
        assert_eq!(
            engine.nodes.get(existing).unwrap().node_data().parent,
            Some(engine.root)
        );
        assert_eq!(engine.redo_len(), 1);
        assert!(engine.redo().unwrap());
        engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
        assert_eq!(engine.node_id_by_uuid(uuid), Some(existing));
    }
}

#[test]
fn blueprint_instance_indexes_follow_add_remove_and_replace_history() {
    for operation in ["add", "remove", "replace"] {
        let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
        engine.add_node(Folder::new("Instance").into(), None);
        engine.apply_edits().unwrap();
        let instance = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
        let blueprint_id = crate::blueprints::BlueprintId::new("history_blueprint");
        let meta = crate::blueprints::BlueprintInstanceMeta::new(
            blueprint_id.clone(),
            4,
            [(DeclId("instance".into()), instance)].into(),
        );
        engine.blueprints.register_instance(instance, meta.clone());
        if operation != "add" {
            engine.clear_history();
            engine.edits.push(if operation == "remove" {
                crate::edit::Edit::RemoveNode { node: instance }
            } else {
                crate::edit::Edit::ReplaceNode {
                    node: instance,
                    new_node: Box::new(Folder::new("Replacement")),
                }
            });
            engine.apply_edits().unwrap();
        }
        let instance_roots = [instance];
        for _ in 0..2 {
            assert!(engine.undo().unwrap());
            let restored = operation != "add";
            assert_eq!(
                engine.blueprint_instance_meta(instance),
                restored.then_some(&meta),
                "{operation}"
            );
            assert_eq!(
                engine.blueprints.instance_roots_for(&blueprint_id),
                if restored { &instance_roots[..] } else { &[] },
                "{operation}"
            );
            assert!(engine.redo().unwrap());
            let restored = operation == "add";
            assert_eq!(
                engine.blueprint_instance_meta(instance),
                restored.then_some(&meta),
                "{operation}"
            );
            assert_eq!(
                engine.blueprints.instance_roots_for(&blueprint_id),
                if restored { &instance_roots[..] } else { &[] },
                "{operation}"
            );
        }
    }
}

#[test]
fn explicit_initializers_win_over_init_defaults_before_ready_and_replay_as_one_edit() {
    let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
    let intent = create(
        &engine,
        "Valid",
        vec![initializer("first", 3.5), initializer("second", 2.0)],
    );
    let ack = engine.apply_ui_intent(intent);
    assert!(ack.success, "{:?}", ack.error_message);
    let item = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let state = engine
        .nodes
        .get(item)
        .unwrap()
        .as_any()
        .downcast_ref::<CreationItem>()
        .unwrap();
    assert_eq!(state.ready_value, 3.5);
    assert_eq!(state.first.get(), 3.5);
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(engine.nodes.len(), 1);
    assert!(engine.redo().unwrap());
    let snapshot = engine.process_tree_snapshot();
    let first = snapshot.resolve_path_from(item, "first").unwrap();
    assert_eq!(snapshot.node(first).unwrap().param_value, Some(ParamValue::Float(3.5)));
}

#[test]
fn binding_initializers_validate_all_targets_before_applying_values() {
    let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
    let intent = create(&engine, "Existing", Vec::new());
    assert!(engine.apply_ui_intent(intent).success);
    let item = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    let history = engine.ui_history_state();
    let events = engine.ui_event_log().to_vec();
    assert!(
        engine
            .ui_apply_initial_params_to_node(
                item,
                vec![initializer("first", 1.0), initializer("missing", 2.0)],
                "TestBinding"
            )
            .is_err()
    );
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(engine.ui_event_log(), events);
}

#[test]
fn prepared_reference_resolves_against_live_graph_before_ready() {
    let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
    engine.add_node(
        Parameter::new("External", ParamValue::Float(1.0), ParameterChangeCheck::None).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let external = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let uuid = engine.nodes.get(external).unwrap().node_data().meta.uuid;
    let intent = create(
        &engine,
        "Reference",
        vec![UiCreateUserItemInitialParam {
            decl_id: DeclId("reference/target".into()),
            value: ParamValue::Reference(NodeReference::with_cached_id(uuid, Some(external))),
        }],
    );
    let ack = engine.apply_ui_intent(intent);
    assert!(ack.success, "{:?}", ack.error_message);
    let item = engine.nodes.get(external).unwrap().node_data().next_sibling.unwrap();
    let item = engine
        .nodes
        .get(item)
        .unwrap()
        .as_any()
        .downcast_ref::<CreationItem>()
        .unwrap();
    let reference = item.ready_reference.as_ref().unwrap();
    assert_eq!(reference.uuid(), uuid);
    assert_eq!(reference.cached_id(), Some(external));
}

#[test]
fn invalid_live_reference_rejects_before_ready_without_publishing_or_losing_redo() {
    INVALID_REFERENCE_READY.store(0, Ordering::SeqCst);
    let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
    let invalid = NodeReference::with_cached_id(
        engine.nodes.get(engine.root).unwrap().node_data().meta.uuid,
        Some(engine.root),
    );
    engine.add_node(Folder::new("Undoable").into(), None);
    engine.apply_edits().unwrap();
    engine.undo().unwrap();
    let intent = create(
        &engine,
        "InvalidReference",
        vec![UiCreateUserItemInitialParam {
            decl_id: DeclId("target".into()),
            value: ParamValue::Reference(invalid),
        }],
    );
    assert_rejected_without_mutation(&mut engine, intent);
    assert_eq!(INVALID_REFERENCE_READY.load(Ordering::SeqCst), 0);
    assert_eq!(engine.redo_len(), 1);
    assert!(engine.redo().unwrap());
}

#[test]
fn binding_parameter_batch_retains_one_undoable_transaction() {
    let mut engine = Engine::new(CreationTestNode::from(CreationFactory::new()));
    let intent = create(&engine, "Existing", Vec::new());
    assert!(engine.apply_ui_intent(intent).success);
    let item = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.clear_history();
    engine
        .ui_apply_initial_params_to_node(
            item,
            vec![initializer("first", 1.0), initializer("second", 2.0)],
            "TestBinding",
        )
        .unwrap();
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert!(engine.redo().unwrap());
    let snapshot = engine.process_tree_snapshot();
    let first = snapshot.resolve_path_from(item, "first").unwrap();
    assert_eq!(snapshot.node(first).unwrap().param_value, Some(ParamValue::Float(1.0)));
}

fn dashboard_page() -> (Engine<CreationTestNode>, NodeId) {
    let mut engine = Engine::new(CreationTestNode::from(Folder::new("Root")));
    engine.add_node(crate::node::DashboardNode::new().into(), None);
    engine.apply_edits().unwrap();
    let dashboard = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    engine
        .queue_catalog_create(
            dashboard,
            crate::node::DASHBOARD_PAGE_NODE_TYPE,
            Some("Page".into()),
            None,
        )
        .unwrap();
    engine.apply_edits().unwrap();
    let page = engine.nodes.get(dashboard).unwrap().node_data().first_child.unwrap();
    (engine, page)
}

#[test]
fn dashboard_creation_rejects_bad_placement_and_layout_before_insertion() {
    let (mut engine, page) = dashboard_page();
    assert_rejected_without_mutation(
        &mut engine,
        UiEditIntent::CreateDashboardContainerWidget {
            parent: page,
            label: None,
            placement: None,
            layout_kind: None,
            prev_sibling: Some(NodeId(u64::MAX)),
        },
    );
    assert_rejected_without_mutation(
        &mut engine,
        UiEditIntent::CreateDashboardContainerWidget {
            parent: page,
            label: None,
            placement: None,
            layout_kind: Some("invalid-layout".into()),
            prev_sibling: None,
        },
    );
}

#[test]
fn wrapping_existing_dashboard_widget_restores_original_parent_on_undo() {
    let (mut engine, page) = dashboard_page();
    let ack = engine.apply_ui_intent(UiEditIntent::CreateDashboardContainerWidget {
        parent: page,
        label: Some("Existing".into()),
        placement: None,
        layout_kind: None,
        prev_sibling: None,
    });
    assert!(ack.success, "{:?}", ack.error_message);
    let widget = engine
        .process_tree_snapshot()
        .child_ids(page)
        .iter()
        .copied()
        .find(|id| engine.nodes.get(*id).unwrap().node_data().meta.label == "Existing")
        .unwrap();
    let before = engine.to_project_json_with(|node| node.project_encode_data()).unwrap();
    engine.clear_history();
    let ack = engine.apply_ui_intent(UiEditIntent::WrapDashboardWidgetInContainer {
        widget,
        placement: None,
        layout_kind: None,
    });
    assert!(ack.success, "{:?}", ack.error_message);
    assert_eq!(engine.undo_len(), 1);
    assert_ne!(engine.nodes.get(widget).unwrap().node_data().parent, Some(page));
    assert!(engine.undo().unwrap());
    assert_eq!(engine.nodes.get(widget).unwrap().node_data().parent, Some(page));
    assert_eq!(
        engine.to_project_json_with(|node| node.project_encode_data()).unwrap(),
        before
    );
    assert!(engine.redo().unwrap());
    assert_ne!(engine.nodes.get(widget).unwrap().node_data().parent, Some(page));
}
