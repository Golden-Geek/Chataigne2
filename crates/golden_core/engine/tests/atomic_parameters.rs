extern crate golden_engine as golden_core;

use golden_engine::{
    edit::Edit,
    engine::Engine,
    node::{Folder, Node, NodeId, ParameterValidationContext},
    parameter::{
        ParamValue, ParameterConstraintPolicy, ParameterConstraints, ParameterEventBehaviour, RangeConstraint,
    },
    process_ctx::ExecutionPhase,
};

#[golden_engine::node("atomic_pair")]
#[children(folder(parameters) { a: i32 = 0; b: i32 = 1; })]
struct Pair {}

#[golden_engine::node("atomic_pair", from_struct)]
impl Node for Pair {
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        let a = change.parameter_value(self.a.id());
        let b = change.parameter_value(self.b.id());
        if a == b {
            return Err("pair values must differ".into());
        }
        Ok(())
    }
}

golden_engine::define_node_enum!(
    enum TestNode {
        Pair(Pair),
    }
);

fn fixture() -> (Engine<TestNode>, NodeId, NodeId) {
    let mut engine = Engine::<TestNode>::new(Folder::new("Root").into());
    engine.add_node(Pair::new().into(), Some(engine.root));
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EndOfTickStabilization).unwrap();
    let owner = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let TestNode::Pair(pair) = engine.nodes.get(owner).unwrap() else {
        panic!()
    };
    let ids = (pair.a.id(), pair.b.id());
    engine.clear_history();
    engine.clear_ui_event_log();
    (engine, ids.0, ids.1)
}

fn value(engine: &Engine<TestNode>, id: NodeId) -> ParamValue {
    engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value
}

fn single(id: NodeId, v: i32) -> Edit {
    Edit::SetParam {
        node: id,
        value: ParamValue::Int(v),
        behaviour: ParameterEventBehaviour::Coalesce,
    }
}

fn batch(values: &[(NodeId, i32)]) -> Edit {
    Edit::SetParamsAtomic {
        values: values.iter().map(|(id, v)| (*id, ParamValue::Int(*v))).collect(),
        behaviour: ParameterEventBehaviour::Append,
    }
}

#[test]
fn final_proposal_is_validated_once_before_any_value_events_or_history_are_committed() {
    let (mut engine, a, b) = fixture();
    engine.edits.push(single(a, 1));
    assert!(engine.apply_edits().is_err());
    assert_eq!(engine.undo_len(), 0);
    assert!(engine.ui_event_log().is_empty());
    engine.edits.push_from(engine.root, batch(&[(a, 1), (b, 2)]));
    engine.apply_edits().unwrap();
    assert_eq!(
        (value(&engine, a), value(&engine, b)),
        (ParamValue::Int(1), ParamValue::Int(2))
    );
    assert_eq!(engine.undo_len(), 1);
    let changed = engine
        .ui_event_log()
        .iter()
        .filter(|event| {
            matches!(event.kind,
        golden_engine::events::EventKind::ParamChanged { source: Some(source), .. } if source == engine.root)
        })
        .count();
    assert_eq!(changed, 2);
    assert!(engine.undo().unwrap());
    assert_eq!(
        (value(&engine, a), value(&engine, b)),
        (ParamValue::Int(0), ParamValue::Int(1))
    );
    assert!(engine.redo().unwrap());
    let before = (engine.undo_len(), engine.ui_event_log().len());
    for edit in [
        batch(&[(a, 3), (b, 3)]),
        batch(&[(a, 4), (a, 5)]),
        Edit::SetParamsAtomic {
            values: vec![(a, ParamValue::Int(4)), (b, ParamValue::Str("invalid".into()))],
            behaviour: ParameterEventBehaviour::Append,
        },
    ] {
        engine.edits.push(edit);
        assert!(engine.apply_edits().is_err());
        assert_eq!(
            (value(&engine, a), value(&engine, b)),
            (ParamValue::Int(1), ParamValue::Int(2))
        );
        assert_eq!((engine.undo_len(), engine.ui_event_log().len()), before);
    }
}

#[test]
fn atomic_boundaries_fence_queue_and_both_cross_drain_history_coalescing_directions() {
    for split in [0, 1, 2] {
        let (mut engine, a, b) = fixture();
        let edits = [single(a, 2), batch(&[(a, 3), (b, 0)]), single(a, 4)];
        for (index, edit) in edits.into_iter().enumerate() {
            engine.edits.push(edit);
            if index == split {
                engine.apply_edits().unwrap();
            }
        }
        engine.apply_edits().unwrap();
        assert_eq!(value(&engine, a), ParamValue::Int(4));
        let count = engine.undo_len();
        for _ in 0..count {
            assert!(engine.undo().unwrap());
        }
        assert_eq!(
            (value(&engine, a), value(&engine, b)),
            (ParamValue::Int(0), ParamValue::Int(1))
        );
        for _ in 0..count {
            assert!(engine.redo().unwrap());
        }
        assert_eq!(
            (value(&engine, a), value(&engine, b)),
            (ParamValue::Int(4), ParamValue::Int(0))
        );
    }
}

#[test]
fn owner_invariants_observe_normalized_values_for_the_entire_proposal() {
    let (mut engine, a, b) = fixture();
    engine
        .nodes
        .get_mut(b)
        .unwrap()
        .engine_set_param_constraints(ParameterConstraints {
            range: RangeConstraint::uniform(Some(0.0), Some(5.0)),
            policy: ParameterConstraintPolicy::ClampAdapt,
            ..Default::default()
        })
        .unwrap();
    engine.edits.push(batch(&[(a, 5), (b, 100)]));
    assert!(engine.apply_edits().is_err());
    assert_eq!(
        (value(&engine, a), value(&engine, b)),
        (ParamValue::Int(0), ParamValue::Int(1))
    );
    assert_eq!(engine.undo_len(), 0);
    assert!(engine.ui_event_log().is_empty());
    engine.edits.push(batch(&[(a, 4), (b, 100)]));
    engine.apply_edits().unwrap();
    assert_eq!(
        (value(&engine, a), value(&engine, b)),
        (ParamValue::Int(4), ParamValue::Int(5))
    );
}

#[test]
fn failed_atomic_redo_preserves_all_values_history_and_events() {
    let (mut engine, a, b) = fixture();
    engine.edits.push(batch(&[(a, 10), (b, 20)]));
    engine.apply_edits().unwrap();
    engine.undo().unwrap();
    let constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(5.0)),
        policy: ParameterConstraintPolicy::Reject,
        ..Default::default()
    };
    engine
        .nodes
        .get_mut(a)
        .unwrap()
        .engine_set_param_constraints(constraints)
        .unwrap();
    let before = (engine.undo_len(), engine.redo_len(), engine.ui_event_log().len());
    assert!(engine.redo().is_err());
    assert_eq!(
        (value(&engine, a), value(&engine, b)),
        (ParamValue::Int(0), ParamValue::Int(1))
    );
    assert_eq!(
        (engine.undo_len(), engine.redo_len(), engine.ui_event_log().len()),
        before
    );
}
