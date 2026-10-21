use super::*;
use crate::{
    edit::Edit,
    events::{Event, EventFrame, EventKind},
    node::Node,
    parameter::{
        ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, ParameterEventBehaviour, RangeConstraint,
    },
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use std::sync::Arc;

fn fixture() -> Engine<Parameter> {
    Engine::new(Parameter::new(
        "value",
        ParamValue::Int(0),
        ParameterChangeCheck::ValueChange,
    ))
}
fn write(node: NodeId, value: i32) -> Edit {
    Edit::SetParam {
        node,
        value: ParamValue::Int(value),
        behaviour: ParameterEventBehaviour::Append,
    }
}
fn changes(engine: &Engine<Parameter>) -> Vec<(i32, Option<NodeId>)> {
    engine
        .inbox
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::ParamChanged {
                new_value: ParamValue::Int(value),
                source,
                ..
            } => Some((*value, *source)),
            _ => None,
        })
        .collect()
}

#[test]
fn unchanged_internal_write_cannot_suppress_or_attribute_a_later_authored_change() {
    let mut engine = fixture();
    let root = engine.root;
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_param_from(root, root, ParamValue::Int(0), ParameterEventBehaviour::Append);
    ctx.edits.push(write(root, 1));
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(changes(&engine), vec![(1, None)]);
}

#[test]
fn appended_changes_keep_their_own_values_and_producers_after_ingress() {
    let mut engine = fixture();
    let root = engine.root;
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.edits.push_from(root, write(root, 1));
    ctx.edits.push(write(root, 2));
    ctx.edits.push_from(NodeId(999), write(root, 3));
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(
        changes(&engine),
        vec![(1, Some(root)), (2, None), (3, Some(NodeId(999)))]
    );
}

#[test]
fn coalescing_replaces_provenance_together_with_the_superseded_value() {
    let mut engine = fixture();
    let root = engine.root;
    engine.edits.push_from(
        root,
        Edit::SetParam {
            node: root,
            value: ParamValue::Int(1),
            behaviour: ParameterEventBehaviour::Coalesce,
        },
    );
    let old = engine.edits.completion();
    engine.edits.push(Edit::SetParam {
        node: root,
        value: ParamValue::Int(2),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    assert_eq!(old.status(), crate::edit::EditCompletionStatus::Failed);
    assert_eq!(changes(&engine), vec![(2, None)]);
}

#[test]
fn rejected_suffix_cannot_leave_attribution_on_future_edits() {
    let mut engine = fixture();
    let root = engine.root;
    engine.edits.push_from(root, write(root, 1));
    engine.edits.push_from(root, write(NodeId(u64::MAX), 2));
    engine.edits.push_from(root, write(root, 3));
    assert!(engine.apply_edits().is_err());
    engine.edits.push(write(root, 4));
    engine.apply_edits().unwrap();
    assert_eq!(changes(&engine), vec![(1, Some(root)), (4, None)]);
}

#[test]
fn source_is_runtime_only_and_cannot_be_forged_by_serialized_event_data() {
    let event = EventKind::ParamChanged {
        param: NodeId(1),
        old_value: ParamValue::Int(0),
        new_value: ParamValue::Int(1),
        source: Some(NodeId(2)),
    };
    let mut wire = serde_json::to_value(&event).unwrap();
    assert!(wire["ParamChanged"].get("source").is_none());
    wire["ParamChanged"]["source"] = serde_json::json!(2);
    let decoded: EventKind = serde_json::from_value(wire).unwrap();
    assert!(matches!(decoded, EventKind::ParamChanged { source: None, .. }));
}

#[test]
fn undo_and_redo_emit_new_authored_changes_without_runtime_source() {
    let mut engine: Engine<ProvenanceTestNode> = Engine::new(Probe::new(Vec::new()).into());
    let root = engine.root;
    engine.add_node(
        Parameter::new("value", ParamValue::Int(0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let value = engine.nodes.get(root).unwrap().node_data().first_child.unwrap();
    engine.edits.push_from(root, write(value, 1));
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine
        .nodes
        .get_mut(root)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<Probe>()
        .unwrap()
        .seen
        .clear();
    assert!(engine.undo().unwrap());
    assert!(engine.redo().unwrap());
    assert_eq!(
        engine
            .nodes
            .get(root)
            .unwrap()
            .as_any()
            .downcast_ref::<Probe>()
            .unwrap()
            .seen,
        vec![(0, None), (1, None)]
    );
}

#[test]
fn constraint_clamping_carries_the_request_source() {
    let mut engine = fixture();
    let root = engine.root;
    engine.edits.push(write(root, 100));
    engine.apply_edits().unwrap();
    engine.inbox.clear();
    engine.edits.push_from(
        root,
        Edit::SetParamConstraints {
            node: root,
            constraints: ParameterConstraints {
                range: Some(RangeConstraint::Uniform {
                    min: Some(0.0),
                    max: Some(10.0),
                }),
                ..Default::default()
            },
        },
    );
    engine.apply_edits().unwrap();
    assert_eq!(changes(&engine), vec![(10, Some(root))]);
}

#[crate::node("provenance_probe")]
struct Probe {
    seen: Vec<(i32, Option<NodeId>)>,
}
crate::define_node_enum!(
    enum ProvenanceTestNode {
        Probe,
    }
);
#[crate::node("provenance_probe", from_struct)]
impl Node for Probe {
    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, _param: NodeId, _old: ParamValue) {
        let Some(Event {
            kind:
                EventKind::ParamChanged {
                    new_value: ParamValue::Int(value),
                    source,
                    ..
                },
            ..
        }) = ctx.current_event()
        else {
            panic!("current parameter event")
        };
        self.seen.push((*value, *source));
    }
}
#[test]
fn default_callbacks_observe_the_exact_event_and_restore_the_context_scope() {
    let engine = fixture();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.events = EventFrame::from_shared(
        (1..=3)
            .map(|value| {
                Arc::new(Event {
                    time: engine.time,
                    kind: EventKind::ParamChanged {
                        param: engine.root,
                        old_value: ParamValue::Int(value - 1),
                        new_value: ParamValue::Int(value),
                        source: (value != 2).then_some(engine.root),
                    },
                })
            })
            .collect(),
    );
    let mut probe = Probe::new(Vec::new());
    probe.dispatch_inbox(&mut ctx);
    assert_eq!(
        probe.seen,
        vec![(1, Some(engine.root)), (2, None), (3, Some(engine.root))]
    );
    assert!(ctx.current_event().is_none());
}
