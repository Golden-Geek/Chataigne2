use super::*;
use crate::app::{AppEngine, AppNode, MouseButtonCommand};
use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
    script::ScriptMethodCall,
};

pub(super) fn module(engine: &mut AppEngine, id: NodeId) -> &mut MouseModule {
    let AppNode::MouseModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("Mouse fixture")
    };
    module
}
pub(super) fn create(control: Control) -> (AppEngine, NodeId, RetirementPool) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    let mut node = MouseModule::create();
    node.disable_backends_for_test();
    engine.add_node(node.into(), None);
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.resolve().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let slots = slots(1);
    install(&mut engine, id, control, &slots);
    quiet(&mut engine, id);
    (engine, id, slots)
}
pub(super) fn install(engine: &mut AppEngine, id: NodeId, control: Control, slots: &RetirementPool) {
    let module = module(engine, id);
    let notifier = module.output.wake.as_ref().unwrap().notifier.clone();
    let runtime =
        Runtime::spawn_using(&module.output.owner, slots, pool(), notifier, move || Ok(Fake(control))).unwrap();
    module.output.runtime = Some(runtime);
    module.output.retry_at = None;
    module.request_output_service();
}
pub(super) fn tick(engine: &mut AppEngine) {
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    engine.apply_edits().unwrap();
}
pub(super) fn quiet(engine: &mut AppEngine, id: NodeId) {
    let mut silent = 0;
    until(|| {
        tick(engine);
        let module = module(engine, id);
        if module.output.outgoing.is_empty()
            && module.output.receipts.is_empty()
            && engine.tick_stats().wake_callbacks_fired == 0
        {
            silent += 1;
        } else {
            silent = 0;
        }
        silent >= 20
    });
}
pub(super) fn invoke(engine: &mut AppEngine, id: NodeId, method: &str, args: Vec<ParamValue>) {
    engine.edits.push(Edit::CallNodeScriptMethod {
        node: id,
        call: ScriptMethodCall::prepare(method, args).unwrap(),
    });
    engine.apply_edits().unwrap();
}
pub(super) fn set(engine: &mut AppEngine, id: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node: id,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
}
pub(super) fn path(engine: &AppEngine, id: NodeId, path: &str) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    path.split('/')
        .fold(id, |id, part| snapshot.find_child_by_decl_id(id, part).unwrap())
}
pub(super) fn value(engine: &AppEngine, id: NodeId) -> ParamValue {
    engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value
}
pub(super) fn context(engine: &AppEngine) -> ProcessCtx {
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    ctx
}
pub(super) fn service(engine: &mut AppEngine, id: NodeId) {
    let snapshot = engine.process_tree_snapshot();
    module(engine, id).engine_sync_bound_param_handles(&mut |id| snapshot.node(id)?.param_value.clone());
    let mut ctx = context(engine);
    module(engine, id).service_output(&mut ctx);
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
}
pub(super) fn command(engine: &mut AppEngine, id: NodeId) -> NodeId {
    let tester = path(engine, id, "command_tester");
    engine.add_user_item(MouseButtonCommand::create().into(), Some(tester));
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.nodes.get(tester).unwrap().node_data().first_child.unwrap()
}
pub(super) fn finish(engine: &mut AppEngine, id: NodeId, slots: &RetirementPool) {
    module(engine, id).stop_output_backend();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn mouse_is_passive_while_input_and_output_are_idle() {
    let (mut engine, id, slots) = create(Control::default());
    for _ in 0..100 {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        engine.apply_edits().unwrap();
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        assert_eq!(engine.tick_stats().snapshot_builds, 0);
    }
    finish(&mut engine, id, &slots);
}
#[test]
fn completed_receipt_survives_snapshot_free_service_and_runtime_retirement() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    invoke(&mut engine, id, "click", vec![ParamValue::Str("left".into())]);
    for _ in 0..2 {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
    }
    service(&mut engine, id);
    until(|| module(&mut engine, id).output.receipts.iter().all(Receipt::complete));
    let mut missing = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service_output(&mut missing);
    assert_eq!(module(&mut engine, id).output.receipts.len(), 1);
    assert!(missing.edits.pending.is_empty());
    finish(&mut engine, id, &slots);
    service(&mut engine, id);
    let last = path(&engine, id, "values/info/last_event");
    assert_eq!(
        value(&engine, last),
        ParamValue::Str("Clicked left mouse button".into())
    );
    assert_eq!(control.calls.lock().unwrap().len(), 1);
}
#[test]
fn queued_result_rename_rejects_before_value_or_traffic_and_retains_source_quota() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    invoke(&mut engine, id, "click", vec![ParamValue::Str("left".into())]);
    for _ in 0..2 {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
    }
    service(&mut engine, id);
    until(|| module(&mut engine, id).output.receipts.iter().all(Receipt::complete));
    let mut ctx = context(&engine);
    module(&mut engine, id).service_output(&mut ctx);
    let last = path(&engine, id, "values/info/last_event");
    let before = value(&engine, last);
    engine.edits.push(Edit::PatchMeta {
        node: last,
        patch: golden_core::node::NodeMetaPatch {
            short_name: Some("moved_result".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    engine.absorb_edits(&mut ctx).unwrap();
    assert_eq!(module(&mut engine, id).output.admission.metrics().active, 1);
    assert!(matches!(
        engine.apply_edits(),
        Err(golden_core::engine::EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(value(&engine, last), before);
    until(|| module(&mut engine, id).output.admission.metrics().active == 0);
    finish(&mut engine, id, &slots);
}

#[test]
fn retirement_rejects_queued_readiness_after_preserving_a_completed_message_prefix() {
    let control = Control::default();
    let (mut engine, id, slots) = create(control.clone());
    invoke(&mut engine, id, "click", vec![ParamValue::Str("left".into())]);
    for _ in 0..2 {
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
    }
    service(&mut engine, id);
    until(|| module(&mut engine, id).output.receipts.iter().all(Receipt::complete));
    let mut ctx = context(&engine);
    module(&mut engine, id).service_output(&mut ctx);
    engine.absorb_edits(&mut ctx).unwrap();
    finish(&mut engine, id, &slots);
    assert!(matches!(
        engine.apply_edits(),
        Err(golden_core::engine::EngineEditError::EditPreconditionFailed { .. })
    ));
    let last = path(&engine, id, "values/info/last_event");
    assert_eq!(
        value(&engine, last),
        ParamValue::Str("Clicked left mouse button".into())
    );
    assert_eq!(control.calls.lock().unwrap().len(), 1);
}
