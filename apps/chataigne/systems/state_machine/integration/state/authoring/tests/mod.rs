use super::*;
use golden_core::{
    application::apply_project_intent,
    node::{Folder, NodeUuid},
    ui_sync::{UiAck, UiEditIntent},
};

mod native;

fn fixture() -> (AppEngine, StateGraphIdentity, [StateGraphIdentity; 2]) {
    let mut engine = AppEngine::new(Folder::new("Root").into());
    let manager = crate::app::StateMachineManager::new();
    let manager_uuid = manager.node_data().meta.uuid;
    engine.add_node(manager.into(), None);
    engine.apply_edits().unwrap();
    let manager = StateGraphIdentity {
        node: engine.node_id_by_uuid(manager_uuid).unwrap(),
        uuid: manager_uuid,
    };
    let states = [0, 1].map(|_| {
        let node = crate::app::StateMachineState::new();
        let uuid = node.node_data().meta.uuid;
        engine.add_user_item(node.into(), Some(manager.node));
        uuid
    });
    engine.apply_edits().unwrap();
    let states = states.map(|uuid| StateGraphIdentity {
        node: engine.node_id_by_uuid(uuid).unwrap(),
        uuid,
    });
    (engine, manager, states)
}

fn invoke(engine: &mut AppEngine, manager: StateGraphIdentity, edit: StateGraphEdit) -> UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: STATE_GRAPH_NAMESPACE.into(),
            operation: STATE_GRAPH_EDIT_OPERATION.into(),
            payload: serde_json::to_value(StateGraphEditRequest { manager, edit }).unwrap(),
        },
    )
}

fn value(engine: &AppEngine, owner: NodeId, declaration: &str) -> ParamValue {
    engine
        .nodes
        .get(child(engine, owner, declaration).unwrap())
        .unwrap()
        .engine_param_snapshot()
        .unwrap()
        .value
}

#[test]
fn state_graph_authoring_moves_are_one_atomic_history_operation() {
    let (mut engine, manager, states) = fixture();
    let history = engine.undo_len();
    let ack = invoke(
        &mut engine,
        manager,
        StateGraphEdit::Move {
            moves: states
                .iter()
                .enumerate()
                .map(|(index, state)| StateGraphMove {
                    state: *state,
                    position: [10.0 + index as f64, -2.5],
                })
                .collect(),
        },
    );
    assert!(ack.success, "{ack:?}");
    let result: StateGraphEditResult = serde_json::from_value(ack.result.unwrap().value).unwrap();
    assert_eq!(result.states, states);
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(value(&engine, states[0].node, "position"), ParamValue::Vec2(10.0, -2.5));
    assert!(engine.undo().unwrap());
    for state in states {
        assert_eq!(value(&engine, state.node, "position"), ParamValue::Vec2(0.0, 0.0));
    }
    assert!(engine.redo().unwrap());
    assert_eq!(value(&engine, states[1].node, "position"), ParamValue::Vec2(11.0, -2.5));
}

#[test]
fn state_graph_authoring_rejects_stale_duplicate_and_incomplete_moves_without_a_prefix() {
    let (mut engine, manager, states) = fixture();
    let history = engine.undo_len();
    for target in [
        StateGraphIdentity {
            uuid: NodeUuid::nil(),
            ..states[1]
        },
        states[0],
        manager,
    ] {
        let ack = invoke(
            &mut engine,
            manager,
            StateGraphEdit::Move {
                moves: vec![
                    StateGraphMove {
                        state: states[0],
                        position: [20.0, 30.0],
                    },
                    StateGraphMove {
                        state: target,
                        position: [40.0, 50.0],
                    },
                ],
            },
        );
        assert!(!ack.success);
        assert_eq!(value(&engine, states[0].node, "position"), ParamValue::Vec2(0.0, 0.0));
        assert_eq!(engine.undo_len(), history);
    }
    let ack = invoke(
        &mut engine,
        StateGraphIdentity {
            uuid: NodeUuid::nil(),
            ..manager
        },
        StateGraphEdit::Move {
            moves: vec![StateGraphMove {
                state: states[0],
                position: [1.0, 2.0],
            }],
        },
    );
    assert!(!ack.success);
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn state_graph_authoring_persists_size_and_metadata_through_undo() {
    let (mut engine, manager, states) = fixture();
    let state = states[0];
    assert!(
        invoke(
            &mut engine,
            manager,
            StateGraphEdit::Resize {
                state,
                size: Some([22.0, 14.0])
            }
        )
        .success
    );
    assert_eq!(value(&engine, state.node, "size"), ParamValue::Vec2(22.0, 14.0));
    let size = child(&engine, state.node, "size").unwrap();
    assert!(engine.nodes.get(size).unwrap().node_data().meta.enabled);
    assert!(invoke(&mut engine, manager, StateGraphEdit::Resize { state, size: None }).success);
    assert!(!engine.nodes.get(size).unwrap().node_data().meta.enabled);
    assert!(
        invoke(
            &mut engine,
            manager,
            StateGraphEdit::Rename {
                state,
                label: "  Edited  ".into()
            }
        )
        .success
    );
    assert_eq!(engine.nodes.get(state.node).unwrap().node_data().meta.label, "Edited");
    assert!(
        invoke(
            &mut engine,
            manager,
            StateGraphEdit::Collapse { state, collapsed: true }
        )
        .success
    );
    assert!(
        engine
            .nodes
            .get(state.node)
            .unwrap()
            .node_data()
            .meta
            .presentation
            .collapsed
    );
    assert!(engine.undo().unwrap());
    assert!(
        !engine
            .nodes
            .get(state.node)
            .unwrap()
            .node_data()
            .meta
            .presentation
            .collapsed
    );
    assert!(
        !invoke(
            &mut engine,
            manager,
            StateGraphEdit::Resize {
                state,
                size: Some([0.0, 10.0])
            }
        )
        .success
    );
}

#[test]
fn state_graph_authoring_connections_create_complete_identity_and_one_undo_step() {
    let (mut engine, manager, states) = fixture();
    let history = engine.undo_len();
    let ack = invoke(
        &mut engine,
        manager,
        StateGraphEdit::Connect {
            from: StateGraphSocket {
                state: states[0],
                socket: STATE_GRAPH_OUTPUT.into(),
            },
            to: StateGraphSocket {
                state: states[1],
                socket: STATE_GRAPH_INPUT.into(),
            },
        },
    );
    assert!(ack.success, "{ack:?}");
    let result: StateGraphEditResult = serde_json::from_value(ack.result.unwrap().value).unwrap();
    let connection = result.connection.unwrap();
    assert_eq!(engine.node_id_by_uuid(connection.uuid), Some(connection.node));
    let ParamValue::Reference(target) = value(&engine, connection.node, "target") else {
        panic!("Transition target is not a reference")
    };
    assert_eq!(target.uuid(), states[1].uuid);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert!(engine.node_id_by_uuid(connection.uuid).is_none());
    assert!(engine.redo().unwrap());
    assert_eq!(engine.node_id_by_uuid(connection.uuid), Some(connection.node));
}

#[test]
fn state_graph_authoring_rejects_invalid_sockets_before_structural_edits() {
    let (mut engine, manager, states) = fixture();
    let nodes = engine.nodes.len();
    let history = engine.undo_len();
    let ack = invoke(
        &mut engine,
        manager,
        StateGraphEdit::Connect {
            from: StateGraphSocket {
                state: states[0],
                socket: "wrong".into(),
            },
            to: StateGraphSocket {
                state: states[1],
                socket: STATE_GRAPH_INPUT.into(),
            },
        },
    );
    assert!(!ack.success);
    assert_eq!(engine.nodes.len(), nodes);
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn state_graph_authoring_readonly_later_target_rejects_the_entire_move() {
    let (mut engine, manager, states) = fixture();
    let position = child(&engine, states[1].node, "position").unwrap();
    engine
        .nodes
        .get_mut(position)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<Parameter>()
        .unwrap()
        .read_only = true;
    let history = engine.undo_len();
    let ack = invoke(
        &mut engine,
        manager,
        StateGraphEdit::Move {
            moves: states
                .into_iter()
                .map(|state| StateGraphMove {
                    state,
                    position: [10.0, 20.0],
                })
                .collect(),
        },
    );
    assert!(!ack.success);
    for state in states {
        assert_eq!(value(&engine, state.node, "position"), ParamValue::Vec2(0.0, 0.0));
    }
    assert_eq!(engine.undo_len(), history);
}
