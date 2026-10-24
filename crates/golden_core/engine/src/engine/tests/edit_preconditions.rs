use super::*;
use crate::{
    edit::{Edit, EditCompletionStatus, EditPreconditions, NodeReadGuard},
    events::EventKind,
    node::Folder,
    parameter::{Parameter, ParameterChangeCheck, ParameterConstraints, ParameterEventBehaviour, RangeConstraint},
};

crate::define_node_enum!(
    enum GuardNode {}
);

fn fixture() -> (Engine<GuardNode>, NodeId, NodeId) {
    let mut engine = Engine::new(GuardNode::from(Folder::new("root")));
    let mut ids = Vec::new();
    for label in ["source", "target"] {
        engine.add_node(
            Parameter::new(label, ParamValue::Float(2.0), ParameterChangeCheck::ValueChange).into(),
            None,
        );
        engine.apply_edits().unwrap();
        ids.push(engine.nodes.get(engine.root).unwrap().node_data().last_child.unwrap());
    }
    (engine, ids[0], ids[1])
}

fn capture(engine: &Engine<GuardNode>, node: NodeId) -> NodeReadGuard {
    NodeReadGuard::capture(&engine.build_process_tree_snapshot(), node).unwrap()
}

fn check(engine: &mut Engine<GuardNode>, nodes: Vec<NodeReadGuard>, native: Vec<golden_io::RevisionGuard>) {
    engine.edits.push(Edit::CheckPreconditions {
        preconditions: EditPreconditions::new(nodes, native).unwrap(),
    });
}

fn write(engine: &mut Engine<GuardNode>, node: NodeId, value: f64) {
    engine.edits.push(Edit::SetParam {
        node,
        value: ParamValue::Float(value),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

fn value(engine: &Engine<GuardNode>, node: NodeId) -> ParamValue {
    engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value
}

#[test]
fn stale_read_rejects_suffix_before_callbacks_and_settles_receipt_with_applied_prefix() {
    let (mut engine, source, target) = fixture();
    let guard = capture(&engine, source);
    write(&mut engine, source, 3.0);
    check(&mut engine, vec![guard], vec![]);
    engine.edits.push(Edit::SetParamConstraints {
        node: target,
        constraints: ParameterConstraints {
            range: RangeConstraint::uniform(None, Some(1.0)),
            ..Default::default()
        },
    });
    let completion = engine.edits.completion();
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { edit_index: 1 })
    ));
    assert_eq!(completion.status(), EditCompletionStatus::Failed);
    assert_eq!(value(&engine, source), ParamValue::Float(3.0));
    assert_eq!(value(&engine, target), ParamValue::Float(2.0));
    assert!(engine.undo().unwrap());
    assert_eq!(value(&engine, source), ParamValue::Float(2.0));
    // Returning to the same value through history does not revive a stale read.
    check(&mut engine, vec![guard], vec![]);
    assert!(engine.apply_edits().is_err());
}

#[test]
fn current_reads_preserve_normal_constraint_events_history_and_redo() {
    let (mut engine, source, target) = fixture();
    let guard = capture(&engine, source);
    // A disjoint write after capture must not make the source guard stale.
    write(&mut engine, target, 4.0);
    engine.apply_edits().unwrap();
    check(&mut engine, vec![guard], vec![]);
    engine.edits.push(Edit::SetParamConstraints {
        node: target,
        constraints: ParameterConstraints {
            range: RangeConstraint::uniform(None, Some(1.0)),
            ..Default::default()
        },
    });
    let completion = engine.edits.completion();
    engine.apply_edits().unwrap();
    assert_eq!(completion.status(), EditCompletionStatus::Applied);
    assert_eq!(value(&engine, target), ParamValue::Float(1.0));
    assert!(
        engine
            .inbox
            .events
            .iter()
            .any(|event| matches!(event.kind, EventKind::ParamConstraintsChanged { param, .. } if param == target))
    );
    assert!(engine.undo().unwrap());
    assert_eq!(value(&engine, target), ParamValue::Float(4.0));
    check(&mut engine, vec![guard], vec![]);
    engine.apply_edits().unwrap();
    assert!(
        engine.redo().unwrap(),
        "a fence has no undo step and does not clear redo"
    );
    assert_eq!(value(&engine, target), ParamValue::Float(1.0));
}

#[test]
fn node_reads_do_not_guard_descendant_values_but_do_guard_child_membership() {
    let (mut engine, source, target) = fixture();
    let root = engine.root;
    let guard = capture(&engine, root);
    write(&mut engine, source, 3.0);
    engine.apply_edits().unwrap();
    check(&mut engine, vec![guard], vec![]);
    engine.apply_edits().unwrap();
    // Root subtree sequence exceeds its local sequence at capture. A later
    // root-local topology write still has to fail the upper-bound comparison.
    let guard = capture(&engine, root);
    engine.edits.push(Edit::RemoveNode { node: source });
    check(&mut engine, vec![guard], vec![]);
    write(&mut engine, target, 5.0);
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(value(&engine, target), ParamValue::Float(2.0));
}

#[test]
fn guards_reject_synthetic_overlays_other_engine_lifetimes_and_removed_nodes() {
    let (mut engine, source, _) = fixture();
    let snapshot = engine.build_process_tree_snapshot();
    let overlay = snapshot.with_param_values([(source, ParamValue::Float(2.0))]);
    check(
        &mut engine,
        vec![NodeReadGuard::capture(&overlay, source).unwrap()],
        vec![],
    );
    assert!(engine.apply_edits().is_err());
    let guard = capture(&engine, source);
    let (mut other, other_source, _) = fixture();
    // Equal ids/UUIDs and values still cannot cross engine epochs.
    assert_eq!(source, other_source);
    other.nodes.get_mut(other_source).unwrap().node_data_mut().meta.uuid = guard.uuid;
    check(&mut other, vec![guard], vec![]);
    assert!(other.apply_edits().is_err());
    engine.edits.push(Edit::RemoveNode { node: source });
    engine.apply_edits().unwrap();
    check(&mut engine, vec![guard], vec![]);
    assert!(engine.apply_edits().is_err());
}

#[test]
fn inherited_disable_fences_an_unchanged_child_read() {
    let (mut engine, source, target) = fixture();
    let guard = capture(&engine, source);
    engine.edits.push(Edit::PatchMeta {
        node: engine.root,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    check(&mut engine, vec![guard], vec![]);
    write(&mut engine, target, 5.0);
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(value(&engine, target), ParamValue::Float(2.0));
}

#[test]
fn native_advance_and_retirement_reject_admitted_writes_without_node_changes() {
    for retire in [false, true] {
        let (mut engine, source, target) = fixture();
        let native = golden_io::RevisionSource::default();
        let guard = capture(&engine, source);
        check(&mut engine, vec![guard], vec![native.reader().guard_at(0)]);
        write(&mut engine, target, 5.0);
        let completion = engine.edits.completion();
        if retire {
            drop(native);
        } else {
            native.advance();
        }
        assert!(matches!(
            engine.apply_edits(),
            Err(EngineEditError::EditPreconditionFailed { edit_index: 0 })
        ));
        assert_eq!(completion.status(), EditCompletionStatus::Failed);
        assert_eq!(value(&engine, target), ParamValue::Float(2.0));
    }
}

#[test]
fn precondition_storage_has_a_fixed_admission_bound() {
    assert!(EditPreconditions::new(Vec::with_capacity(33), vec![]).is_none());
    assert!(EditPreconditions::new(vec![], Vec::with_capacity(9)).is_none());
    assert!(EditPreconditions::new(Vec::with_capacity(32), Vec::with_capacity(8)).is_some());
}

#[test]
fn subtree_reads_fence_descendant_writes_before_callback_delivery() {
    let (mut engine, source, target) = fixture();
    let root = engine.root;
    let guard = NodeReadGuard::capture_subtree(&engine.build_process_tree_snapshot(), root).unwrap();
    write(&mut engine, source, 3.0);
    check(&mut engine, vec![guard], vec![]);
    write(&mut engine, target, 5.0);
    let completion = engine.edits.completion();
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { edit_index: 1 })
    ));
    assert_eq!(value(&engine, source), ParamValue::Float(3.0));
    assert_eq!(value(&engine, target), ParamValue::Float(2.0));
    assert_eq!(completion.status(), EditCompletionStatus::Failed);
    assert!(engine.undo().unwrap());
    check(&mut engine, vec![guard], vec![]);
    assert!(engine.apply_edits().is_err(), "undo does not revive a prepared subtree");
}

#[test]
fn subtree_reads_ignore_disjoint_changes_but_fence_descendant_metadata_and_removal() {
    for remove in [false, true] {
        let (mut engine, source, target) = fixture();
        let guard = NodeReadGuard::capture_subtree(&engine.build_process_tree_snapshot(), source).unwrap();
        write(&mut engine, target, 4.0);
        engine.apply_edits().unwrap();
        check(&mut engine, vec![guard], vec![]);
        engine.apply_edits().unwrap();
        let guard = NodeReadGuard::capture_subtree(&engine.build_process_tree_snapshot(), engine.root).unwrap();
        engine.edits.push(if remove {
            Edit::RemoveNode { node: source }
        } else {
            Edit::PatchMeta {
                node: source,
                patch: crate::node::NodeMetaPatch {
                    label: Some("new source".into()),
                    ..Default::default()
                },
            }
        });
        check(&mut engine, vec![guard], vec![]);
        assert!(matches!(
            engine.apply_edits(),
            Err(EngineEditError::EditPreconditionFailed { .. })
        ));
    }
}

#[test]
fn subtree_reads_reject_overlay_epochs_and_inherited_disable() {
    let (mut engine, source, _) = fixture();
    let snapshot = engine.build_process_tree_snapshot();
    let overlay = snapshot.with_param_values([(source, ParamValue::Float(2.0))]);
    check(
        &mut engine,
        vec![NodeReadGuard::capture_subtree(&overlay, source).unwrap()],
        vec![],
    );
    assert!(engine.apply_edits().is_err());
    let guard = NodeReadGuard::capture_subtree(&snapshot, source).unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: engine.root,
        patch: crate::node::NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    check(&mut engine, vec![guard], vec![]);
    assert!(engine.apply_edits().is_err());
}

#[test]
fn structural_reads_survive_values_constraints_and_warnings_but_fence_aliases() {
    let (mut engine, source, _) = fixture();
    let guard = NodeReadGuard::capture_structure(&engine.build_process_tree_snapshot(), source).unwrap();
    write(&mut engine, source, 3.0);
    engine.edits.push(Edit::SetParamConstraints {
        node: source,
        constraints: ParameterConstraints {
            range: RangeConstraint::uniform(Some(0.0), Some(10.0)),
            ..Default::default()
        },
    });
    engine.edits.push(Edit::SetNodeWarning {
        node: source,
        warning: crate::node::NodeWarning {
            id: "status".into(),
            message: "retrying".into(),
            detail: None,
        },
    });
    check(&mut engine, vec![guard], vec![]);
    engine.apply_edits().unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: source,
        patch: NodeMetaPatch {
            short_name: Some("different".into()),
            ..Default::default()
        },
    });
    check(&mut engine, vec![guard], vec![]);
    assert!(engine.apply_edits().is_err());
}

#[test]
fn structural_reads_fence_moves_lifetimes_and_overlay_epochs() {
    let (mut engine, source, target) = fixture();
    let snapshot = engine.build_process_tree_snapshot();
    let guard = NodeReadGuard::capture_structure(&snapshot, source).unwrap();
    let overlay = snapshot.with_param_values([(source, ParamValue::Float(2.0))]);
    check(
        &mut engine,
        vec![NodeReadGuard::capture_structure(&overlay, source).unwrap()],
        vec![],
    );
    assert!(engine.apply_edits().is_err());
    engine.edits.push(Edit::MoveNode {
        node: source,
        new_parent: target,
        new_prev_sibling: None,
    });
    check(&mut engine, vec![guard], vec![]);
    assert!(engine.apply_edits().is_err());
    let fresh = NodeReadGuard::capture_structure(&engine.build_process_tree_snapshot(), source).unwrap();
    engine.edits.push(Edit::RemoveNode { node: source });
    check(&mut engine, vec![fresh], vec![]);
    assert!(engine.apply_edits().is_err());
}

#[test]
fn identity_reads_tolerate_presentation_writes_but_still_fence_epoch_uuid_and_enable() {
    let (mut engine, source, _) = fixture();
    let snapshot = engine.build_process_tree_snapshot();
    let guard = NodeReadGuard::capture_identity(&snapshot, source).unwrap();
    engine.edits.push(Edit::SetNodeWarning {
        node: source,
        warning: crate::node::NodeWarning {
            id: "status".into(),
            message: "retrying".into(),
            detail: None,
        },
    });
    check(&mut engine, vec![guard], vec![]);
    engine.apply_edits().unwrap();
    let authored = NodeReadGuard::capture(&snapshot, source).unwrap();
    check(&mut engine, vec![authored], vec![]);
    assert!(engine.apply_edits().is_err());
    let overlay = snapshot.with_param_values([(source, ParamValue::Float(2.0))]);
    check(
        &mut engine,
        vec![NodeReadGuard::capture_identity(&overlay, source).unwrap()],
        vec![],
    );
    assert!(engine.apply_edits().is_err());
    let current = engine.nodes.get(source).unwrap().node_data().meta.uuid;
    engine.nodes.get_mut(source).unwrap().node_data_mut().meta.uuid = crate::node::NodeUuid(uuid::Uuid::new_v4());
    check(&mut engine, vec![guard], vec![]);
    assert!(engine.apply_edits().is_err());
    engine.nodes.get_mut(source).unwrap().node_data_mut().meta.uuid = current;
    engine.edits.push(Edit::PatchMeta {
        node: engine.root,
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    check(&mut engine, vec![guard], vec![]);
    assert!(engine.apply_edits().is_err());
}
