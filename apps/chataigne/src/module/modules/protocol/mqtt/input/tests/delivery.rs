use super::*;

#[test]
fn processing_churn_after_a_committed_large_prefix_cancels_only_the_suffix_and_new_input_recovers() {
    use golden_core::{
        edit::NodeTree,
        node::Folder,
        parameter::{Parameter, ParameterChangeCheck},
    };
    let (mut engine, id) = create_mqtt_module();
    let root = find_path(&engine, id, "values").unwrap();
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(Folder::new("packet")).with_child(NodeTree::new(Parameter::new(
            "pulse",
            ParamValue::Trigger(),
            ParameterChangeCheck::None,
        ))),
        parent: root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let pulse = find_path(&engine, id, "values/packet/pulse").unwrap();
    let count = |engine: &crate::app::AppEngine| {
        engine
            .inbox
            .events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::ParamChanged { param, .. } if param == pulse))
            .count()
    };
    let before = count(&engine);
    let bytes =
        serde_json::to_vec(&serde_json::json!({ "pulse": null, "tuple": (0..700).collect::<Vec<_>>() })).unwrap();
    let lifetime = RevisionSource::default();
    let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "packet", &bytes));
    let mut partial = None;
    for _ in 0..20 {
        let (done, mut ctx) = tick(&mut engine, &mut delivery);
        assert!(!done);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        if let Some(tuple) = find_path(&engine, id, "values/packet/tuple") {
            let children = engine.process_tree_snapshot().child_ids_slice(tuple).len();
            if children > 0 && children < 700 {
                partial = Some((tuple, children));
                break;
            }
        }
    }
    let (tuple, children) = partial.expect("fixture must commit a proper prefix before source churn");
    assert_eq!(count(&engine) - before, 1);
    let auto_add = find_path(&engine, id, "parameters/processing/auto_add").unwrap();
    for value in [false, true, false, true] {
        engine.edits.push(Edit::SetParam {
            node: auto_add,
            value: ParamValue::Bool(value),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().unwrap();
    }
    let (done, ctx) = tick(&mut engine, &mut delivery);
    assert!(done && ctx.edits.pending.is_empty());
    assert_eq!(engine.process_tree_snapshot().child_ids_slice(tuple).len(), children);
    assert_eq!(callbacks(&engine), 0);
    let mut current = Delivery::new(prepared(&engine, id, &lifetime, "packet", &bytes));
    for turn in 0..100 {
        let (done, mut ctx) = tick(&mut engine, &mut current);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        if done {
            break;
        }
        assert!(turn < 99);
    }
    assert_eq!(engine.process_tree_snapshot().child_ids_slice(tuple).len(), 700);
    assert_eq!(
        count(&engine) - before,
        2,
        "old applied occurrence plus one new occurrence"
    );
    assert_eq!(callbacks(&engine), 1);
}

#[test]
fn trigger_before_a_missing_json_field_is_not_replayed_during_materialization() {
    use golden_core::{
        edit::NodeTree,
        node::Folder,
        parameter::{Parameter, ParameterChangeCheck},
    };
    let (mut engine, id) = create_mqtt_module();
    let root = find_path(&engine, id, "values").unwrap();
    engine.edits.push(Edit::AddNodeTree {
        tree: NodeTree::new(Folder::new("packet")).with_child(NodeTree::new(Parameter::new(
            "a",
            ParamValue::Trigger(),
            ParameterChangeCheck::None,
        ))),
        parent: root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let trigger = find_path(&engine, id, "values/packet/a").unwrap();
    let count = |engine: &crate::app::AppEngine| {
        engine
            .inbox
            .events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::ParamChanged { param, .. } if param == trigger))
            .count()
    };
    let before = count(&engine);
    let lifetime = RevisionSource::default();
    let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "packet", br#"{"a":null,"b":42}"#));
    for turn in 0..30 {
        let (done, mut ctx) = tick(&mut engine, &mut delivery);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        if done {
            break;
        }
        assert!(turn < 29);
    }
    assert_eq!(count(&engine) - before, 1);
    assert!(find_path(&engine, id, "values/packet/b").is_some());
}

#[test]
fn partial_value_failure_abandons_the_suffix_without_replaying_the_applied_trigger() {
    use golden_core::{
        edit::{EditRequest, NodeTree},
        node::Folder,
        parameter::{Parameter, ParameterChangeCheck},
    };
    let (mut engine, id) = create_mqtt_module();
    let root = find_path(&engine, id, "values").unwrap();
    let tree = NodeTree::new(Folder::new("packet"))
        .with_child(NodeTree::new(Parameter::new(
            "a",
            ParamValue::Trigger(),
            ParameterChangeCheck::None,
        )))
        .with_child(NodeTree::new(Parameter::new(
            "b",
            ParamValue::Int(0),
            ParameterChangeCheck::ValueChange,
        )));
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let trigger = find_path(&engine, id, "values/packet/a").unwrap();
    let before = engine
        .inbox
        .events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::ParamChanged { param, .. } if param == trigger))
        .count();
    let lifetime = RevisionSource::default();
    let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "packet", br#"{"a":null,"b":42}"#));
    let (_, mut ctx) = tick(&mut engine, &mut delivery);
    let first = ctx
        .edits
        .pending
        .iter()
        .position(|request| matches!(request.edit, Edit::SetParam { node, .. } if node == trigger))
        .unwrap();
    ctx.edits.pending.insert(
        first + 1,
        EditRequest::new(Edit::SetParam {
            node: NodeId(u64::MAX),
            value: ParamValue::Int(1),
            behaviour: ParameterEventBehaviour::Append,
        }),
    );
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    for turn in 0..10 {
        let (done, mut ctx) = tick(&mut engine, &mut delivery);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        if done {
            break;
        }
        assert!(turn < 9);
    }
    let after = engine
        .inbox
        .events
        .iter()
        .filter(|event| matches!(event.kind, EventKind::ParamChanged { param, .. } if param == trigger))
        .count();
    assert_eq!(after - before, 1);
    assert_eq!(callbacks(&engine), 1);
}

#[test]
fn large_tuple_yields_and_publishes_one_callback_after_all_value_receipts() {
    let (mut engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    let bytes = serde_json::to_vec(&(0..700).collect::<Vec<_>>()).unwrap();
    let input = prepared(&engine, id, &lifetime, "tuple", &bytes);
    let mut delivery = Delivery::new(input);
    let initial = callbacks(&engine);
    let mut turns = 0;
    loop {
        let (done, mut ctx) = tick(&mut engine, &mut delivery);
        assert!(ctx.edits.pending.len() <= 257);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        turns += 1;
        if done {
            break;
        }
        assert!(turns < 100);
    }
    assert!(turns > 3);
    let tuple = find_path(&engine, id, "values/tuple").unwrap();
    assert_eq!(engine.process_tree_snapshot().child_ids_slice(tuple).len(), 700);
    assert_eq!(callbacks(&engine), initial + 1);
}

#[test]
fn retired_source_rejects_already_queued_rows_after_delivery_is_destroyed() {
    let (mut engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "late/value", b"42"));
    let (_, mut ctx) = tick(&mut engine, &mut delivery);
    assert!(!ctx.edits.pending.is_empty());
    drop(lifetime);
    drop(delivery);
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    assert!(find_path(&engine, id, "values/late").is_none());
}

#[test]
fn processing_change_before_application_rejects_the_old_prefix() {
    let (mut engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "old/value", b"42"));
    let (_, mut ctx) = tick(&mut engine, &mut delivery);
    let auto_add = find_path(&engine, id, "parameters/processing/auto_add").unwrap();
    engine.edits.push(Edit::SetParam {
        node: auto_add,
        value: ParamValue::Bool(false),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    let (done, ctx) = tick(&mut engine, &mut delivery);
    assert!(done);
    assert!(ctx.edits.pending.is_empty());
    assert!(find_path(&engine, id, "values/old").is_none());
}

#[test]
fn processing_change_back_to_the_same_value_still_cancels_the_old_source_without_retrying_callbacks() {
    let (mut engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "old/value", b"42"));
    let (_, mut ctx) = tick(&mut engine, &mut delivery);
    let auto_add = find_path(&engine, id, "parameters/processing/auto_add").unwrap();
    for value in [false, true] {
        engine.edits.push(Edit::SetParam {
            node: auto_add,
            value: ParamValue::Bool(value),
            behaviour: ParameterEventBehaviour::Append,
        });
    }
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    let (done, ctx) = tick(&mut engine, &mut delivery);
    assert!(done);
    assert!(ctx.edits.pending.is_empty());
    assert_eq!(callbacks(&engine), 0);
}

#[test]
fn later_failure_does_not_replay_a_committed_callback() {
    let (mut engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "value", b"42"));
    let before = callbacks(&engine);
    for _ in 0..20 {
        let (_, mut ctx) = tick(&mut engine, &mut delivery);
        let callback = ctx
            .edits
            .pending
            .iter()
            .any(|request| matches!(request.edit, Edit::EmitCustomEvent { .. }));
        if callback {
            ctx.edits.push(Edit::SetParam {
                node: NodeId(u64::MAX),
                value: ParamValue::Int(1),
                behaviour: ParameterEventBehaviour::Append,
            });
        }
        engine.absorb_edits(&mut ctx).unwrap();
        if callback {
            assert!(engine.apply_edits().is_err());
            let (done, ctx) = tick(&mut engine, &mut delivery);
            assert!(done);
            assert!(ctx.edits.pending.is_empty());
            assert_eq!(callbacks(&engine), before + 1);
            return;
        }
        engine.apply_edits().unwrap();
    }
    panic!("callback was not published");
}
