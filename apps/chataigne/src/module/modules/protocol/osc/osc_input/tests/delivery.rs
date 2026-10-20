use super::super::super::osc_input_delivery::InputDelivery;
use super::*;
use golden_core::{
    edit::Edit,
    events::EventKind,
    node::Folder,
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use std::collections::HashMap;

fn service(engine: &mut crate::app::AppEngine, delivery: &mut InputDelivery) -> (bool, ProcessCtx) {
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    let done = delivery.service(&mut ctx, Some(&snapshot), Some(engine.root), true, &mut HashMap::new());
    (done, ctx)
}

#[test]
fn repeated_trigger_messages_preserve_parameter_and_callback_occurrences() {
    use golden_core::parameter::{Parameter, ParameterChangeCheck};
    let mut engine = crate::app::AppEngine::new(Folder::new("values").into());
    engine.add_node(
        Parameter::new("trigger", ParamValue::Trigger(), ParameterChangeCheck::None).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let mut preparer = isolated(4, 4);
    preparer.plan.module = engine.root;
    let messages = (0..2)
        .map(|_| {
            let mut message = message("/trigger");
            message.payload = OscValuePayload::Single(ParamValue::Trigger());
            message
        })
        .collect();
    let mut draft = preparer.draft_batch(messages).unwrap();
    let mut delivery = InputDelivery::new(preparer.retain(&mut draft).unwrap());
    drop(draft);
    for turn in 0..10 {
        let (done, mut ctx) = service(&mut engine, &mut delivery);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        if done {
            break;
        }
        assert!(turn < 9);
    }
    assert_eq!(
        engine
            .inbox
            .events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::ParamChanged { .. }))
            .count(),
        2
    );
    assert_eq!(engine.inbox.events.iter().filter(|event|
        matches!(&event.kind, EventKind::Custom(custom) if custom.topic == crate::app::module::script_api::MODULE_SCRIPT_CALLBACK_TOPIC)).count(), 2);
}

#[test]
fn later_edit_failure_does_not_replay_a_committed_input_callback() {
    use golden_core::{node::NodeId, parameter::ParameterEventBehaviour};
    let mut engine = crate::app::AppEngine::new(Folder::new("values").into());
    let mut preparer = isolated(1, 1);
    preparer.plan.module = engine.root;
    let mut draft = preparer.draft(message("/value")).unwrap();
    let mut delivery = InputDelivery::new(preparer.retain(&mut draft).unwrap());
    drop(draft);
    for turn in 0..10 {
        let (done, mut ctx) = service(&mut engine, &mut delivery);
        assert!(!done);
        let has_callback = ctx
            .edits
            .pending
            .iter()
            .any(|request| matches!(request.edit, Edit::EmitCustomEvent { .. }));
        if has_callback {
            ctx.edits.push(Edit::SetParam {
                node: NodeId(u64::MAX),
                value: ParamValue::Int(1),
                behaviour: ParameterEventBehaviour::Append,
            });
        }
        engine.absorb_edits(&mut ctx).unwrap();
        if has_callback {
            assert!(engine.apply_edits().is_err());
            let (done, ctx) = service(&mut engine, &mut delivery);
            assert!(done);
            assert!(ctx.edits.pending.is_empty());
            assert_eq!(engine.inbox.events.iter().filter(|event|
                matches!(&event.kind, EventKind::Custom(custom) if custom.topic == crate::app::module::script_api::MODULE_SCRIPT_CALLBACK_TOPIC)).count(), 1);
            return;
        }
        engine.apply_edits().unwrap();
        assert!(turn < 9);
    }
}

#[test]
fn large_tuple_yields_and_publishes_exactly_one_callback_after_value_completion() {
    let mut engine = crate::app::AppEngine::new(Folder::new("values").into());
    let mut preparer = isolated(1, 1);
    preparer.plan.module = engine.root;
    let mut message = message("/tuple");
    message.payload = OscValuePayload::Multi((0..700).map(ParamValue::Int).collect());
    let mut draft = preparer.draft(message).unwrap();
    let prepared = preparer.retain(&mut draft).unwrap();
    drop(draft);
    let mut delivery = InputDelivery::new(prepared);
    let mut ignored = HashMap::new();
    let mut complete = false;
    let mut turns = 0;
    while !complete {
        let snapshot = engine.process_tree_snapshot();
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        complete = delivery.service(&mut ctx, Some(&snapshot), Some(engine.root), true, &mut ignored);
        assert!(ctx.edits.pending.len() <= 256);
        if turns == 0 {
            assert!(!complete);
            assert!(ctx
                .edits
                .pending
                .iter()
                .all(|request| !matches!(request.edit, Edit::EmitCustomEvent { .. })));
            let mut trees = ctx
                .edits
                .pending
                .iter()
                .filter_map(|request| match &request.edit {
                    Edit::AddNodeTree { tree, .. } => Some(tree),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut materialized = 0;
            while let Some(tree) = trees.pop() {
                materialized += 1;
                trees.extend(&tree.children);
            }
            assert!(materialized <= 256, "a detached tree must also respect the turn budget");
        }
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        turns += 1;
        assert!(turns < 30);
    }
    assert!(turns > 2, "700 values must be split across bounded service turns");
    let snapshot = engine.process_tree_snapshot();
    let tuple = snapshot.find_child(engine.root, "tuple").unwrap();
    assert_eq!(snapshot.child_ids_slice(tuple).len(), 700);
    for index in [1, 256, 700] {
        let child = snapshot.find_child(tuple, &format!("value {index}")).unwrap();
        assert_eq!(
            snapshot.node(child).unwrap().param_value,
            Some(ParamValue::Int(index - 1))
        );
    }
    let callbacks = engine.inbox.events.iter().filter(|event|
        matches!(&event.kind, EventKind::Custom(custom) if custom.topic == crate::app::module::script_api::MODULE_SCRIPT_CALLBACK_TOPIC)
    ).count();
    assert_eq!(callbacks, 1);
}
