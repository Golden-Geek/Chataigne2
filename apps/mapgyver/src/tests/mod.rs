//! Shared assertions for app-level contracts crossing the public Golden input boundary.
use crate::app::AppEngine;
use golden_core::{
    application::apply_project_intent,
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiEditIntent,
};

pub(crate) fn clamps_and_undo(engine: &mut AppEngine, node: NodeId, input: ParamValue, expected: ParamValue) {
    let before = golden_core::app::to_sparse_project_json_pretty(engine).unwrap();
    let history = engine.undo_len();
    let previous = engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value;
    let ack = apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value: input,
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    let snapshot = engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap();
    assert_eq!(snapshot.value, expected);
    assert_eq!(snapshot.constraints.normalize(expected.clone()).unwrap(), expected);
    if previous != expected {
        assert_eq!(engine.undo_len(), history + 1);
        assert!(engine.undo().unwrap());
    }
    assert_eq!(engine.undo_len(), history);
    assert_eq!(golden_core::app::to_sparse_project_json_pretty(engine).unwrap(), before);
}
