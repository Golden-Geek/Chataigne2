extern crate golden_engine as golden_core;

use golden_engine::{
    edit::Edit,
    engine::Engine,
    node::{Folder, Node, NodeId, ParameterValidationContext},
    parameter::{ParamValue, ParameterEventBehaviour},
};

#[golden_engine::node("bounded_pair")]
#[children(folder(parameters) { a: i32 = 2 [0..100]; b: i32 = 3 [0..100]; })]
struct BoundedPair {}

#[golden_engine::node("bounded_pair", from_struct)]
impl Node for BoundedPair {
    fn normalize_parameter_change(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Option<ParamValue>, String> {
        if change.parameter != self.a.id() {
            return Ok(None);
        }
        let (ParamValue::Int(value), Some(ParamValue::Int(ceiling))) =
            (change.value, change.parameter_value(self.b.id()))
        else {
            return Err("expected integer pair".into());
        };
        Ok((*value > ceiling).then_some(ParamValue::Int(ceiling)))
    }

    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        let (Some(ParamValue::Int(a)), Some(ParamValue::Int(b))) =
            (change.parameter_value(self.a.id()), change.parameter_value(self.b.id()))
        else {
            return Err("expected integer pair".into());
        };
        if a > b {
            return Err("a must not exceed b".into());
        }
        Ok(())
    }
}

#[golden_engine::node("invalid_normalizer")]
#[children(value: i32 = 0;)]
struct InvalidNormalizer {}

#[golden_engine::node("invalid_normalizer", from_struct)]
impl Node for InvalidNormalizer {
    fn normalize_parameter_change(&self, _: &ParameterValidationContext<'_>) -> Result<Option<ParamValue>, String> {
        Ok(Some(ParamValue::Str("wrong type".into())))
    }
}

golden_engine::define_node_enum!(
    enum TestNode {
        BoundedPair(BoundedPair),
        InvalidNormalizer(InvalidNormalizer),
    }
);

fn fixture() -> (Engine<TestNode>, NodeId, NodeId) {
    let mut engine = Engine::<TestNode>::new(Folder::new("Root").into());
    engine.add_node(BoundedPair::new().into(), Some(engine.root));
    engine.apply_edits().unwrap();
    let owner = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let TestNode::BoundedPair(pair) = engine.nodes.get(owner).unwrap() else {
        panic!()
    };
    let ids = (pair.a.id(), pair.b.id());
    engine.clear_history();
    engine.clear_ui_event_log();
    (engine, ids.0, ids.1)
}

fn value(engine: &Engine<TestNode>, node: NodeId) -> ParamValue {
    engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value
}

#[test]
fn scalar_owner_normalization_precedes_validation_events_history_and_noop_detection() {
    let (mut engine, a, _) = fixture();
    for requested in [20, 30] {
        engine.edits.push(Edit::SetParam {
            node: a,
            value: ParamValue::Int(requested),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().unwrap();
        assert_eq!(value(&engine, a), ParamValue::Int(3));
        assert_eq!(engine.undo_len(), 1);
    }
    assert!(engine.undo().unwrap());
    assert_eq!(value(&engine, a), ParamValue::Int(2));
    assert!(engine.redo().unwrap());
    assert_eq!(value(&engine, a), ParamValue::Int(3));
}

#[test]
fn atomic_normalization_uses_all_prepared_siblings_independently_of_request_order() {
    for reverse in [false, true] {
        let (mut engine, a, b) = fixture();
        let mut values = vec![(a, ParamValue::Int(20)), (b, ParamValue::Int(8))];
        if reverse {
            values.reverse();
        }
        engine.edits.push(Edit::SetParamsAtomic {
            values,
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().unwrap();
        assert_eq!(
            (value(&engine, a), value(&engine, b)),
            (ParamValue::Int(8), ParamValue::Int(8))
        );
        assert_eq!(engine.undo_len(), 1);
        assert!(engine.undo().unwrap());
        assert_eq!(
            (value(&engine, a), value(&engine, b)),
            (ParamValue::Int(2), ParamValue::Int(3))
        );
        assert!(engine.redo().unwrap());
        assert_eq!(
            (value(&engine, a), value(&engine, b)),
            (ParamValue::Int(8), ParamValue::Int(8))
        );
        let before = engine.ui_event_log().len();
        engine.edits.push(Edit::SetParam {
            node: b,
            value: ParamValue::Int(0),
            behaviour: ParameterEventBehaviour::Append,
        });
        assert!(engine.apply_edits().is_err());
        assert_eq!(
            (value(&engine, a), value(&engine, b)),
            (ParamValue::Int(8), ParamValue::Int(8))
        );
        assert_eq!(engine.undo_len(), 1);
        assert_eq!(engine.ui_event_log().len(), before);
    }
}

#[test]
fn owner_normalization_cannot_bypass_the_parameter_type_boundary() {
    let mut engine = Engine::<TestNode>::new(Folder::new("Root").into());
    engine.add_node(InvalidNormalizer::new().into(), Some(engine.root));
    engine.apply_edits().unwrap();
    let owner = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let TestNode::InvalidNormalizer(owner) = engine.nodes.get(owner).unwrap() else {
        panic!()
    };
    let node = owner.value.id();
    engine.clear_history();
    engine.clear_ui_event_log();
    engine.edits.push(Edit::SetParam {
        node,
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Append,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(value(&engine, node), ParamValue::Int(0));
    assert_eq!(engine.undo_len(), 0);
    assert!(engine.ui_event_log().is_empty());
}
