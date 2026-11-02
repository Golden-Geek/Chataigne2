extern crate golden_engine as golden_core;

use golden_engine::{
    edit::Edit,
    engine::Engine,
    events::EventKind,
    node::{Folder, Node, NodeId, ParameterValidationContext},
    parameter::{ParamValue, ParameterEventBehaviour},
    ui_sync::UiEditIntent,
};

#[golden_engine::node("vectors")]
#[children(
    pair: golden_engine::parameter::Vec2 = (0.0, 0.0) [(0.0, 0.0)..(1.0, 1.0)];
    triple: golden_engine::parameter::Vec3 = (1.0, 2.0, 3.0);
    scalar: f64 = 0.0;
    fixed: golden_engine::parameter::Vec2 = (0.0, 0.0) (read_only = true);
)]
struct Vectors {}

#[golden_engine::node("vectors", from_struct)]
impl Node for Vectors {
    fn normalize_parameter_change(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Option<ParamValue>, String> {
        if change.parameter == self.pair.id()
            && let ParamValue::Vec2(x, y) = change.value
        {
            let accepted = x.min(1.0 - y);
            if accepted != *x {
                return Ok(Some(ParamValue::Vec2(accepted, *y)));
            }
        }
        Ok(None)
    }

    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if change.parameter == self.triple.id()
            && let ParamValue::Vec3(_, _, z) = change.value
            && *z < 0.0
        {
            return Err("negative z rejected by owner".into());
        }
        Ok(())
    }
}

golden_engine::define_node_enum!(
    enum TestNode {
        Vectors(Vectors),
    }
);

fn fixture() -> (Engine<TestNode>, [NodeId; 4]) {
    let mut engine = Engine::<TestNode>::new(Folder::new("Root").into());
    engine.add_node(Vectors::new().into(), Some(engine.root));
    engine.apply_edits().unwrap();
    let owner = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let TestNode::Vectors(owner) = engine.nodes.get(owner).unwrap() else {
        panic!()
    };
    let ids = [owner.pair.id(), owner.triple.id(), owner.scalar.id(), owner.fixed.id()];
    engine.clear_history();
    engine.clear_ui_event_log();
    (engine, ids)
}

fn component(node: NodeId, axis: u8, value: f64) -> Edit {
    Edit::SetParamComponent {
        node,
        component: axis,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    }
}
fn value(engine: &Engine<TestNode>, node: NodeId) -> ParamValue {
    engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value
}

#[test]
fn queued_components_compose_at_apply_time_preserve_other_axes_and_replay_exactly() {
    let (mut engine, [pair, triple, ..]) = fixture();
    engine.edits.push_from(engine.root, component(pair, 0, 0.02));
    engine.edits.push_from(engine.root, component(pair, 1, 0.9));
    engine.edits.push(component(triple, 2, 7.0));
    engine.edits.push(component(triple, 0, 4.0));
    engine.apply_edits().unwrap();
    assert_eq!(value(&engine, pair), ParamValue::Vec2(0.02, 0.9));
    assert_eq!(value(&engine, triple), ParamValue::Vec3(4.0, 2.0, 7.0));
    assert!(engine.ui_event_log().iter().any(|event| matches!(event.kind, EventKind::ParamChanged { param, source: Some(source), .. } if param == pair && source == engine.root)));
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(value(&engine, pair), ParamValue::Vec2(0.0, 0.0));
    assert_eq!(value(&engine, triple), ParamValue::Vec3(1.0, 2.0, 3.0));
    assert!(engine.redo().unwrap());
    assert_eq!(value(&engine, pair), ParamValue::Vec2(0.02, 0.9));
    assert_eq!(value(&engine, triple), ParamValue::Vec3(4.0, 2.0, 7.0));
}

#[test]
fn component_edits_observe_normalized_canonical_siblings_and_keep_ordering_boundaries() {
    let (mut engine, [pair, ..]) = fixture();
    engine.edits.push(component(pair, 1, 0.9));
    engine.apply_edits().unwrap();
    engine.edits.push(component(pair, 0, 0.8));
    engine.edits.push(component(pair, 1, 0.2));
    engine.edits.push(component(pair, 0, 0.7));
    assert_eq!(engine.edits.pending.len(), 3);
    engine.apply_edits().unwrap();
    assert_eq!(value(&engine, pair), ParamValue::Vec2(0.7, 0.2));
    engine.edits.push(component(pair, 0, 0.1));
    engine.edits.push(Edit::SetParam {
        node: pair,
        value: ParamValue::Vec2(0.4, 0.5),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.edits.push(component(pair, 0, 0.2));
    engine.apply_edits().unwrap();
    assert_eq!(value(&engine, pair), ParamValue::Vec2(0.2, 0.5));
    engine.edits.push(component(pair, 0, 0.3));
    engine.edits.push(component(pair, 0, 0.4));
    assert_eq!(engine.edits.pending.len(), 1);
    engine.apply_edits().unwrap();
    assert_eq!(value(&engine, pair), ParamValue::Vec2(0.4, 0.5));
}

#[test]
fn invalid_components_reject_before_values_events_or_history_change() {
    let (mut engine, [pair, triple, scalar, fixed]) = fixture();
    for (node, axis, requested) in [
        (pair, 2, 0.1),
        (triple, 3, 0.1),
        (scalar, 0, 0.1),
        (pair, 0, f64::NAN),
        (pair, 0, f64::INFINITY),
        (triple, 2, -1.0),
    ] {
        engine.edits.push(component(node, axis, requested));
        assert!(engine.apply_edits().is_err());
        assert_eq!(value(&engine, pair), ParamValue::Vec2(0.0, 0.0));
        assert_eq!(value(&engine, triple), ParamValue::Vec3(1.0, 2.0, 3.0));
        assert_eq!(engine.undo_len(), 0);
        assert!(engine.ui_event_log().is_empty());
    }
    let ack = engine.apply_ui_intent(UiEditIntent::SetParamComponent {
        node: fixed,
        component: 0,
        value: 0.5,
        behaviour: ParameterEventBehaviour::Append,
    });
    assert!(!ack.success);
    assert_eq!(value(&engine, fixed), ParamValue::Vec2(0.0, 0.0));
}

#[test]
fn transport_component_intents_use_the_same_authoritative_composition_boundary() {
    let (mut engine, [pair, ..]) = fixture();
    for (axis, requested) in [(0, 0.02), (1, 0.9), (0, 0.0)] {
        let intent = UiEditIntent::SetParamComponent {
            node: pair,
            component: axis,
            value: requested,
            behaviour: ParameterEventBehaviour::Append,
        };
        let encoded = serde_json::to_value(&intent).unwrap();
        assert_eq!(encoded["kind"], "setParamComponent");
        assert!(engine.apply_ui_intent(serde_json::from_value(encoded).unwrap()).success);
    }
    assert_eq!(
        value(&engine, pair),
        ParamValue::Vec2(0.0, 0.9),
        "pending cancellations supersede prior axis values"
    );
}
