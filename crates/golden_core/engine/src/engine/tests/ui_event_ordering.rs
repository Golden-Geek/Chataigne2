use std::collections::HashSet;

use super::*;
use crate::events::EventKind;
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck};
use crate::ui_sync::UiGraphOp;

#[test]
fn loaded_node_patches_wait_for_their_materializing_transaction() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    engine.add_node(
        Parameter::new("inserted", ParamValue::Int(0), ParameterChangeCheck::None),
        None,
    );
    engine.apply_edits().expect("child should be added");
    let inserted = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("child should exist");
    engine.clear_ui_event_log();

    engine.emit_event(EventKind::ParamChanged {
        param: engine.root,
        old_value: ParamValue::Int(0),
        new_value: ParamValue::Int(1),
    });
    engine.emit_event(EventKind::ParamChanged {
        param: inserted,
        old_value: ParamValue::Int(0),
        new_value: ParamValue::Int(2),
    });
    engine.emit_event(EventKind::ParamChanged {
        param: inserted,
        old_value: ParamValue::Trigger(),
        new_value: ParamValue::Trigger(),
    });

    let deferred = engine.squash_pre_materialization_ui_events(&HashSet::from([inserted]));
    engine.push_ui_graph_transaction(vec![UiGraphOp::ChildrenReordered {
        parent: engine.root,
        children: vec![inserted],
    }]);
    for trigger in deferred {
        engine.push_ui_event_kind(trigger);
    }

    let events = engine.ui_event_log();
    assert_eq!(events.len(), 3);
    assert!(matches!(&events[0].kind, EventKind::ParamChanged { param, .. } if *param == engine.root));
    assert!(matches!(&events[1].kind, EventKind::GraphTransaction { .. }));
    assert!(matches!(
        &events[2].kind,
        EventKind::ParamChanged {
            param,
            new_value: ParamValue::Trigger(),
            ..
        } if *param == inserted
    ));
    assert!(events.windows(2).all(|pair| pair[0].time < pair[1].time));
}
