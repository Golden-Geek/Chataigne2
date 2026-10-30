use super::*;
use crate::{
    edit::Edit,
    events::EventKind,
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour},
};

#[test]
fn repeated_trigger_pulses_emit_events_without_empty_undo_steps_or_clearing_redo() {
    let mut engine = Engine::new(Parameter::new(
        "Pulse",
        ParamValue::Trigger(),
        ParameterChangeCheck::ValueChange,
    ));
    let root = engine.root;
    engine.edits.push(Edit::PatchMeta {
        node: root,
        patch: crate::node::NodeMetaPatch {
            label: Some("Renamed".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    let history = engine.undo_len();
    let redo = engine.redo_len();
    for _ in 0..2 {
        engine.edits.push(Edit::SetParam {
            node: root,
            value: ParamValue::Trigger(),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().unwrap();
    }
    assert_eq!(engine.undo_len(), history);
    assert_eq!(engine.redo_len(), redo);
    assert_eq!(
        engine
            .inbox
            .events
            .iter()
            .filter(|event| matches!(&event.kind,
                EventKind::ParamChanged { param, new_value: ParamValue::Trigger(), .. } if *param == root
            ))
            .count(),
        2
    );
    assert!(engine.redo().unwrap());
    assert_eq!(engine.nodes.get(root).unwrap().node_data().meta.label, "Renamed");
}
