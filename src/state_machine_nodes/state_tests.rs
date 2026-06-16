use golden_core::{
    app::ProjectFileSpec,
    color::Color,
    edit::Edit,
    node::{DeclId, Folder, Node, NodeId, NodeMetaPatch, NodeReference},
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_read_model::UiReadModel,
    ui_sync::{UiCreateUserItemInitialParam, UiEditIntent, UiSubscriptionScope},
};

use super::StateMachineState;
use crate::app::StateMachineManager;

#[test]
fn state_defaults_are_ready_for_canvas_authoring() {
    let state = StateMachineState::new();

    assert!(state.active.get());
    assert_eq!(state.description.get_ref(), "");
    assert_eq!(state.position.get().x, 0.0);
    assert_eq!(state.position.get().y, 0.0);
    assert_eq!(state.size.get().x, 13.0);
    assert_eq!(state.size.get().y, 8.0);
}

#[test]
fn state_canvas_geometry_survives_project_reload() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(StateMachineManager::new().into(), None);
    engine
        .apply_edits()
        .expect("state machine manager should attach");
    let manager_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("state machine manager should be attached under root");
    engine.add_user_item(StateMachineState::new().into(), Some(manager_id));
    engine.add_user_item(StateMachineState::new().into(), Some(manager_id));
    for _ in 0..3 {
        engine
            .apply_edits()
            .expect("state defaults should materialize");
    }

    let state_id = engine
        .nodes
        .get(manager_id)
        .and_then(|manager| manager.node_data().first_child)
        .expect("state should be attached under state machine manager");
    let target_state_id = engine
        .nodes
        .get(state_id)
        .and_then(|state| state.node_data().next_sibling)
        .expect("target state should be attached under root");
    assert_eq!(
        engine
            .nodes
            .get(state_id)
            .expect("state should exist")
            .node_data()
            .meta
            .presentation
            .color,
        Some(Color::new(0.28, 0.56, 0.92, 1.0))
    );
    let transition_manager_id = child_by_decl(&engine, state_id, "transitions");
    engine.add_user_item(
        crate::app::StateTransition::new().into(),
        Some(transition_manager_id),
    );
    for _ in 0..2 {
        engine
            .apply_edits()
            .expect("state transition should materialize");
    }
    let transition_id = engine
        .nodes
        .get(transition_manager_id)
        .and_then(|manager| manager.node_data().first_child)
        .expect("transition should be attached under its manager");
    let target_param_id = child_by_decl(&engine, transition_id, "target");
    let target_uuid = engine
        .nodes
        .get(target_state_id)
        .expect("target state should exist")
        .node_data()
        .meta
        .uuid;
    engine.edits.push(Edit::SetParam {
        node: target_param_id,
        value: ParamValue::Reference(golden_core::node::NodeReference::new(target_uuid)),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    for (decl_id, value) in [
        ("position", ParamValue::Vec2(21.5, -7.25)),
        ("size", ParamValue::Vec2(18.75, 11.5)),
    ] {
        let param_id = child_by_decl(&engine, state_id, decl_id);
        engine.edits.push(Edit::SetParam {
            node: param_id,
            value,
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
    let size_param_id = child_by_decl(&engine, state_id, "size");
    engine.edits.push(Edit::PatchMeta {
        node: size_param_id,
        patch: NodeMetaPatch {
            enabled: Some(true),
            ..Default::default()
        },
    });
    engine
        .apply_edits()
        .expect("state canvas geometry should update");

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("state project should encode");
    let loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json)
        .expect("state project should decode");
    let loaded_state_id = loaded
        .nodes
        .get(loaded.root)
        .and_then(|root| root.node_data().first_child)
        .and_then(|manager_id| loaded.nodes.get(manager_id))
        .and_then(|manager| manager.node_data().first_child)
        .expect("state should reload under state machine manager");

    for (decl_id, expected) in [
        ("position", ParamValue::Vec2(21.5, -7.25)),
        ("size", ParamValue::Vec2(18.75, 11.5)),
    ] {
        let param_id = child_by_decl(&loaded, loaded_state_id, decl_id);
        let actual = loaded
            .nodes
            .get(param_id)
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value);
        assert_eq!(actual, Some(expected));
    }
    assert!(
        loaded
            .nodes
            .get(child_by_decl(&loaded, loaded_state_id, "size"))
            .expect("loaded size parameter should exist")
            .node_data()
            .meta
            .enabled,
        "loaded custom size should remain enabled"
    );

    let snapshot = loaded.ui_snapshot(UiSubscriptionScope::WholeGraph);
    let snapshot_state = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == loaded_state_id)
        .expect("reloaded state should be included in UI snapshot");
    for (decl_id, expected) in [
        ("position", ParamValue::Vec2(21.5, -7.25)),
        ("size", ParamValue::Vec2(18.75, 11.5)),
    ] {
        let param_id = snapshot_state
            .children
            .iter()
            .find_map(|child_id| {
                snapshot
                    .nodes
                    .iter()
                    .find(|node| node.node_id == *child_id && node.decl_id.0 == decl_id)
            })
            .unwrap_or_else(|| panic!("snapshot parameter '{decl_id}' should exist"));
        let golden_core::ui_sync::UiNodeDataDto::Parameter { param } = &param_id.data else {
            panic!("snapshot child '{decl_id}' should be a parameter");
        };
        assert_eq!(param.value, expected);
        if decl_id == "size" {
            assert!(param_id.meta.enabled, "snapshot custom size should remain enabled");
        }
    }

    let transition_manager_id = snapshot_state
        .children
        .iter()
        .find_map(|child_id| {
            snapshot
                .nodes
                .iter()
                .find(|node| node.node_id == *child_id && node.decl_id.0 == "transitions")
        })
        .expect("snapshot transition manager should exist");
    assert_eq!(
        transition_manager_id.children.len(),
        1,
        "reloaded transition should be included in UI snapshot"
    );
}

#[test]
fn transition_creation_keeps_reload_snapshot_complete() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(StateMachineManager::new().into(), None);
    engine
        .apply_edits()
        .expect("state machine manager should attach");
    let manager_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("state machine manager should be attached under root");

    for label in ["Reload A", "Reload B"] {
        let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
            parent: manager_id,
            node_type: StateMachineState::NODE_TYPE.to_string(),
            label: Some(label.to_string()),
            initial_params: Vec::new(),
        });
        assert!(ack.success, "state creation should succeed: {ack:?}");
    }

    let source_state = engine
        .nodes
        .get(manager_id)
        .and_then(|manager| manager.node_data().first_child)
        .expect("source state should exist");
    let target_state = engine
        .nodes
        .get(source_state)
        .and_then(|state| state.node_data().next_sibling)
        .expect("target state should exist");
    let target_uuid = engine
        .nodes
        .get(target_state)
        .expect("target state should exist")
        .node_data()
        .meta
        .uuid;
    let transition_manager = child_by_decl(&engine, source_state, "transitions");

    let project_file = ProjectFileSpec::new("Noisette", "noisette");
    let read_model = UiReadModel::from_engine(&engine, project_file.clone());
    let previous_event_time = read_model.current_event_time();
    for (state_id, values) in [
        (source_state, [12.5, 7.25, 19.0, 10.0]),
        (target_state, [42.0, 15.0, 16.0, 9.0]),
    ] {
        for initial_param in geometry_initial_params(values) {
            let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
                node: child_by_decl(&engine, state_id, &initial_param.decl_id.0),
                value: initial_param.value,
                behaviour: ParameterEventBehaviour::Coalesce,
            });
            assert!(ack.success, "state geometry edit should succeed: {ack:?}");
        }
    }
    let capture = read_model.collect_event_batch(&engine, previous_event_time);
    read_model.apply_event_capture(capture);

    let previous_event_time = read_model.current_event_time();
    let ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: transition_manager,
        node_type: crate::app::StateTransition::NODE_TYPE.to_string(),
        label: Some("Transition".to_string()),
        initial_params: vec![UiCreateUserItemInitialParam {
            decl_id: DeclId("target".to_string()),
            value: ParamValue::Reference(NodeReference::new(target_uuid)),
        }],
    });
    assert!(ack.success, "transition creation should succeed: {ack:?}");

    let capture = read_model.collect_event_batch(&engine, previous_event_time);
    read_model.apply_event_capture(capture);

    let engine_snapshot = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);
    let reload_snapshot = read_model.current_snapshot();
    for state_id in [source_state, target_state] {
        assert_state_geometry_matches(&engine_snapshot, &reload_snapshot, state_id);
    }

    let engine_transition_manager = snapshot_node(&engine_snapshot, transition_manager);
    let reload_transition_manager = snapshot_node(&reload_snapshot, transition_manager);
    assert_eq!(
        reload_transition_manager.children,
        engine_transition_manager.children,
        "reload snapshot should retain transition children"
    );
    for transition_id in &engine_transition_manager.children {
        assert_eq!(
            snapshot_node(&reload_snapshot, *transition_id),
            snapshot_node(&engine_snapshot, *transition_id),
            "reload snapshot should retain the full transition subtree"
        );
    }
}

fn geometry_initial_params(values: [f64; 4]) -> Vec<UiCreateUserItemInitialParam> {
    vec![
        UiCreateUserItemInitialParam {
            decl_id: DeclId("position".to_string()),
            value: ParamValue::Vec2(values[0], values[1]),
        },
        UiCreateUserItemInitialParam {
            decl_id: DeclId("size".to_string()),
            value: ParamValue::Vec2(values[2], values[3]),
        },
    ]
}

fn assert_state_geometry_matches(
    engine_snapshot: &golden_core::ui_sync::UiSnapshot,
    reload_snapshot: &golden_core::ui_sync::UiSnapshot,
    state_id: NodeId,
) {
    let engine_state = snapshot_node(engine_snapshot, state_id);
    let reload_state = snapshot_node(reload_snapshot, state_id);
    for decl_id in ["position", "size"] {
        let engine_param = snapshot_child_by_decl(engine_snapshot, engine_state, decl_id);
        let reload_param = snapshot_child_by_decl(reload_snapshot, reload_state, decl_id);
        assert_eq!(
            reload_param.data, engine_param.data,
            "reload snapshot should retain State '{decl_id}'"
        );
    }
}

fn snapshot_node(
    snapshot: &golden_core::ui_sync::UiSnapshot,
    node_id: NodeId,
) -> &golden_core::ui_sync::UiNodeDto {
    snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == node_id)
        .unwrap_or_else(|| panic!("snapshot node {node_id:?} should exist"))
}

fn snapshot_child_by_decl<'a>(
    snapshot: &'a golden_core::ui_sync::UiSnapshot,
    parent: &golden_core::ui_sync::UiNodeDto,
    decl_id: &str,
) -> &'a golden_core::ui_sync::UiNodeDto {
    parent
        .children
        .iter()
        .find_map(|child_id| {
            snapshot
                .nodes
                .iter()
                .find(|node| node.node_id == *child_id && node.decl_id.0 == decl_id)
        })
        .unwrap_or_else(|| panic!("snapshot parameter '{decl_id}' should exist"))
}

fn child_by_decl(
    engine: &crate::app::AppEngine,
    parent: NodeId,
    decl_id: &str,
) -> NodeId {
    let mut child = engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child);
    while let Some(child_id) = child {
        let node = engine.nodes.get(child_id).expect("child should exist");
        if node.node_data().meta.decl_id.0 == decl_id {
            return child_id;
        }
        child = node.node_data().next_sibling;
    }
    panic!("declared child '{decl_id}' should exist");
}
