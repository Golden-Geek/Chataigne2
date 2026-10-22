use super::*;
use golden_core::{
    edit::NodeTree,
    engine::EngineEditError,
    node::{Folder, Node, NodeMetaPatch},
    parameter::{Parameter, ParameterChangeCheck},
};

fn parameter(label: &str, value: i32) -> NodeTree {
    let mut parameter = Parameter::new(label, ParamValue::Int(value), ParameterChangeCheck::ValueChange);
    parameter.node_data_mut().meta.can_be_disabled = true;
    NodeTree::new(parameter)
}

fn fixture() -> (crate::app::AppEngine, NodeId, NodeId, NodeId, NodeId) {
    let (mut engine, id) = create_mqtt_module();
    let root = find_path(&engine, id, "values").unwrap();
    let tree = NodeTree::new(Folder::new("yard"))
        .with_child(
            NodeTree::new(Folder::new("left"))
                .with_child(NodeTree::new(Folder::new("branch")).with_child(parameter("value", 7))),
        )
        .with_child(NodeTree::new(Folder::new("right")));
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    let value = find_path(&engine, id, "values/yard/left/branch/value").unwrap();
    let branch = find_path(&engine, id, "values/yard/left/branch").unwrap();
    let right = find_path(&engine, id, "values/yard/right").unwrap();
    (engine, id, value, branch, right)
}

fn finish_failed_prefix(engine: &mut crate::app::AppEngine, delivery: &mut Delivery) {
    for turn in 0..10 {
        let (done, mut ctx) = tick(engine, delivery);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        if done {
            return;
        }
        assert!(turn < 9, "failed input must not retry its graph prefix forever");
    }
}

fn value(engine: &crate::app::AppEngine, node: NodeId) -> ParamValue {
    engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value
}

#[test]
fn guard_storage_is_admitted_before_actor_delivery_and_the_draft_survives_byte_pressure() {
    let (engine, id) = create_mqtt_module();
    let lifetime = RevisionSource::default();
    let mut draft = build::Draft::new(raw("value", b"42"), source(&engine, id, &lifetime), false).unwrap();
    let callbacks = NativeCallbackPreparer::new().unwrap();
    let small = PayloadPool::new("mqtt-guard-byte-pressure", 1, 8 * 1024).unwrap();
    assert!(draft.retain(&small, &callbacks).is_none());
    let identity = draft.prepared_identity().unwrap();
    let large = PayloadPool::new("mqtt-guard-byte-recovery", 1, 2 * 1024 * 1024).unwrap();
    let input = draft.retain(&large, &callbacks).unwrap();
    assert_eq!(draft.prepared_identity(), Some(identity));
    assert_eq!(input.values[0].path_segments, ["value"]);
}

#[test]
fn moving_a_target_or_deep_ancestor_rejects_the_prefix_before_any_value_write() {
    for move_ancestor in [false, true] {
        let (mut engine, id, target, branch, right) = fixture();
        let lifetime = RevisionSource::default();
        let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "yard/left/branch/value", b"42"));
        let (_, mut ctx) = tick(&mut engine, &mut delivery);
        engine.edits.push(Edit::MoveNode {
            node: if move_ancestor { branch } else { target },
            new_parent: right,
            new_prev_sibling: None,
        });
        engine.absorb_edits(&mut ctx).unwrap();
        let result = engine.apply_edits();
        assert!(
            matches!(&result, Err(EngineEditError::EditPreconditionFailed { .. })),
            "{result:?}"
        );
        assert_eq!(
            engine
                .nodes
                .get(if move_ancestor { branch } else { target })
                .unwrap()
                .node_data()
                .parent,
            Some(right)
        );
        assert_eq!(value(&engine, target), ParamValue::Int(7));
        finish_failed_prefix(&mut engine, &mut delivery);
        assert_eq!(value(&engine, target), ParamValue::Int(7));
        assert_eq!(callbacks(&engine), 1);
    }
}

#[test]
fn replacement_and_named_path_changes_reject_queued_writes() {
    for replace in [false, true] {
        let (mut engine, id, target, _, _) = fixture();
        let lifetime = RevisionSource::default();
        let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "yard/left/branch/value", b"42"));
        let (_, mut ctx) = tick(&mut engine, &mut delivery);
        if replace {
            engine.replace_node(
                target,
                Parameter::new("value", ParamValue::Int(99), ParameterChangeCheck::ValueChange).into(),
            );
        } else {
            engine.edits.push(Edit::PatchMeta {
                node: target,
                patch: NodeMetaPatch {
                    label: Some("renamed".into()),
                    ..Default::default()
                },
            });
        }
        engine.absorb_edits(&mut ctx).unwrap();
        let result = engine.apply_edits();
        assert!(
            matches!(&result, Err(EngineEditError::EditPreconditionFailed { .. })),
            "{result:?}"
        );
        assert_eq!(value(&engine, target), ParamValue::Int(if replace { 99 } else { 7 }));
        finish_failed_prefix(&mut engine, &mut delivery);
        assert_eq!(callbacks(&engine), 1);
    }
}

#[test]
fn scalar_changes_preserve_a_valid_queued_target_binding() {
    let (mut engine, id, target, _, _) = fixture();
    let lifetime = RevisionSource::default();
    let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "yard/left/branch/value", b"42"));
    let (_, mut ctx) = tick(&mut engine, &mut delivery);
    engine.edits.push(Edit::SetParam {
        node: target,
        value: ParamValue::Int(99),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(value(&engine, target), ParamValue::Int(42));
    finish_failed_prefix(&mut engine, &mut delivery);
    assert_eq!(callbacks(&engine), 1);
}

fn wide_fixture() -> (crate::app::AppEngine, NodeId, Vec<u8>) {
    let (mut engine, id) = create_mqtt_module();
    let root = find_path(&engine, id, "values").unwrap();
    let mut tree = NodeTree::new(Folder::new("batch"));
    let mut json = serde_json::Map::new();
    for index in 0..50 {
        let label = format!("branch{index}");
        tree.push_child(parameter(&label, -1));
        json.insert(label, serde_json::json!({"value": index}));
    }
    engine.edits.push(Edit::AddNodeTree {
        tree,
        parent: root,
        prev_sibling: None,
    });
    engine.apply_edits().unwrap();
    (engine, id, serde_json::to_vec(&json).unwrap())
}

#[test]
fn multiple_read_groups_all_precede_structural_writes_and_preserve_their_receipt() {
    let (mut engine, id, bytes) = wide_fixture();
    let lifetime = RevisionSource::default();
    let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "batch", &bytes));
    let (_, mut ctx) = tick(&mut engine, &mut delivery);
    let checks = ctx
        .edits
        .pending
        .iter()
        .take_while(|request| matches!(request.edit, Edit::CheckPreconditions { .. }))
        .count();
    assert!(checks >= 3, "the prefix must exercise multiple 32-read groups");
    assert!(
        ctx.edits.pending[checks..]
            .iter()
            .any(|request| matches!(request.edit, Edit::ReplaceNode { .. })),
        "exercise replacement of an existing parent before adding its children"
    );
    assert!(ctx.edits.pending[checks..]
        .iter()
        .all(|request| !matches!(request.edit, Edit::CheckPreconditions { .. })));
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    for turn in 0..20 {
        let (done, mut ctx) = tick(&mut engine, &mut delivery);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        if done {
            break;
        }
        assert!(turn < 19);
    }
    for index in 0..50 {
        let target = find_path(&engine, id, &format!("values/batch/branch{index}/value")).unwrap();
        assert_eq!(value(&engine, target), ParamValue::Int(index));
    }
    assert_eq!(callbacks(&engine), 1);
}

#[test]
fn a_stale_target_in_the_last_read_group_prevents_every_prefix_mutation() {
    let (mut engine, id, bytes) = wide_fixture();
    let lifetime = RevisionSource::default();
    let mut delivery = Delivery::new(prepared(&engine, id, &lifetime, "batch", &bytes));
    let (_, mut ctx) = tick(&mut engine, &mut delivery);
    let target = ctx
        .edits
        .pending
        .iter()
        .rev()
        .find_map(|request| match request.edit {
            Edit::ReplaceNode { node, .. } => Some(node),
            _ => None,
        })
        .unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: target,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.absorb_edits(&mut ctx).unwrap();
    let result = engine.apply_edits();
    assert!(
        matches!(&result, Err(EngineEditError::EditPreconditionFailed { edit_index }) if *edit_index >= 3),
        "{result:?}"
    );
    for index in 0..50 {
        let target = find_path(&engine, id, &format!("values/batch/branch{index}")).unwrap();
        assert_eq!(value(&engine, target), ParamValue::Int(-1));
    }
    finish_failed_prefix(&mut engine, &mut delivery);
    assert_eq!(callbacks(&engine), 1);
}
