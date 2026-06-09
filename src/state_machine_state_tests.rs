use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
};

use super::StateMachineState;

#[test]
fn state_defaults_are_ready_for_canvas_authoring() {
    let state = StateMachineState::new();

    assert!(state.active.get());
    assert_eq!(state.description.get_ref(), "");
    assert_eq!(state.x.get(), 0.0);
    assert_eq!(state.y.get(), 0.0);
    assert_eq!(state.width.get(), 13.0);
    assert_eq!(state.height.get(), 8.0);
}

#[test]
fn state_canvas_geometry_survives_project_reload() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(StateMachineState::new().into(), None);
    engine.add_node(StateMachineState::new().into(), None);
    for _ in 0..3 {
        engine
            .apply_edits()
            .expect("state defaults should materialize");
    }

    let state_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("state should be attached under root");
    let target_state_id = engine
        .nodes
        .get(state_id)
        .and_then(|state| state.node_data().next_sibling)
        .expect("target state should be attached under root");
    let transition_manager_id = child_by_decl(&engine, state_id, "transitions");
    engine.add_node(
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
    for (decl_id, value) in [("x", 21.5), ("y", -7.25), ("width", 18.75), ("height", 11.5)] {
        let param_id = child_by_decl(&engine, state_id, decl_id);
        engine.edits.push(Edit::SetParam {
            node: param_id,
            value: ParamValue::Float(value),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
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
        .expect("state should reload under root");

    for (decl_id, expected) in [("x", 21.5), ("y", -7.25), ("width", 18.75), ("height", 11.5)] {
        let param_id = child_by_decl(&loaded, loaded_state_id, decl_id);
        let actual = loaded
            .nodes
            .get(param_id)
            .and_then(|node| node.engine_param_snapshot())
            .map(|snapshot| snapshot.value);
        assert_eq!(actual, Some(ParamValue::Float(expected)));
    }
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
