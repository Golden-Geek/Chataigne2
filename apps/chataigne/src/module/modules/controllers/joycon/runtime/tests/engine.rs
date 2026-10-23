use super::super::super::JoyConModule;
use super::super::*;
use crate::app::{AppEngine, AppNode};
use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use std::{collections::VecDeque, thread};

struct Fake(Arc<Mutex<VecDeque<JoyConWorkerEvent>>>);
impl Provider for Fake {
    fn execute(&mut self, _: &JoyConWorkerCommand) -> Result<String, String> {
        Ok("sent".into())
    }
    fn poll(&mut self) -> Vec<JoyConWorkerEvent> {
        self.0.lock().unwrap().pop_front().into_iter().collect()
    }
}
fn module(engine: &mut AppEngine, id: NodeId) -> &mut JoyConModule {
    let AppNode::JoyConModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("Joy-Con fixture")
    };
    module
}
fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(Instant::now() < deadline, "Joy-Con engine deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn create() -> (
    AppEngine,
    NodeId,
    RetirementPool,
    Arc<Mutex<VecDeque<JoyConWorkerEvent>>>,
) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    engine.add_node(JoyConModule::create().into(), None);
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.resolve().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let slots = RetirementPool::new(NonZeroUsize::MIN);
    let input = Arc::new(Mutex::new(VecDeque::new()));
    let provider = Fake(input.clone());
    let wake = module(&mut engine, id).service.wake.as_ref().unwrap().notifier.clone();
    let permit = module(&mut engine, id).service.owner.try_reserve().unwrap();
    let commands = PayloadPool::new("joycon-engine-output", 128, 128 * COMMAND_WEIGHT).unwrap();
    let events = PayloadPool::new("joycon-engine-input", 64, 64 * EVENT_WEIGHT).unwrap();
    let handle = JoyConRuntimeHandle::spawn_using(wake, &slots, commands, events, move || {
        Ok(Exclusive {
            provider,
            _permit: permit,
        })
    })
    .unwrap();
    let module = module(&mut engine, id);
    module.runtime = Some(Box::new(handle));
    module.runtime_dirty = false;
    module.service.retry_at = None;
    module.request_service();
    (engine, id, slots, input)
}
fn context(engine: &AppEngine) -> ProcessCtx {
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    ctx
}
fn flush(engine: &mut AppEngine, ctx: &mut ProcessCtx) {
    engine.absorb_edits(ctx).unwrap();
    engine.apply_edits().unwrap();
}
fn service(engine: &mut AppEngine, id: NodeId) {
    let mut ctx = context(engine);
    module(engine, id).service_io(&mut ctx);
    flush(engine, &mut ctx);
}
fn state(pressed: bool, x: f64) -> JoyConWorkerEvent {
    let mut state = JoyConRuntimeState::disconnected();
    state.left.connected = true;
    state.left.stick_x = x;
    if pressed {
        state.left.left_buttons.push(Buttons::Down);
    }
    JoyConWorkerEvent::State(Box::new(state))
}
fn queued(engine: &mut AppEngine, id: NodeId) -> usize {
    module(engine, id)
        .runtime
        .as_ref()
        .unwrap()
        .worker
        .as_ref()
        .unwrap()
        .event_metrics()
        .depth
}
fn buttons(engine: &AppEngine) -> Vec<bool> {
    engine
        .inbox
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            golden_core::events::EventKind::Custom(event)
                if event.payload.get("callback").and_then(serde_json::Value::as_str) == Some("joyConButtonChanged") =>
            {
                event.payload.get("args")?.get(2)?.as_bool()
            }
            _ => None,
        })
        .collect()
}
fn path(engine: &AppEngine, id: NodeId, path: &str) -> NodeId {
    let snapshot = engine.process_tree_snapshot();
    path.split('/')
        .fold(id, |parent, key| snapshot.find_child_by_decl_id(parent, key).unwrap())
}
fn value(engine: &AppEngine, id: NodeId) -> ParamValue {
    engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value
}
fn finish(engine: &mut AppEngine, id: NodeId, slots: &RetirementPool) {
    module(engine, id).stop_runtime();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn actor_turn_keeps_press_release_occurrences_and_projects_only_two_input_records() {
    let (mut engine, id, slots, input) = create();
    input
        .lock()
        .unwrap()
        .extend([state(true, 0.0), state(false, 0.0), state(true, 0.0)]);
    until(|| queued(&mut engine, id) == 3);
    service(&mut engine, id);
    assert_eq!(buttons(&engine), [true, false]);
    assert_eq!(queued(&mut engine, id), 1);
    service(&mut engine, id);
    assert_eq!(buttons(&engine), [true, false, true]);
    finish(&mut engine, id, &slots);
}

#[test]
fn snapshot_free_wake_keeps_native_publication_for_later_service() {
    let (mut engine, id, slots, input) = create();
    input.lock().unwrap().push_back(state(true, 0.0));
    until(|| queued(&mut engine, id) == 1);
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service_io(&mut ctx);
    assert!(ctx.edits.pending.is_empty());
    assert_eq!(queued(&mut engine, id), 1);
    service(&mut engine, id);
    assert_eq!(buttons(&engine), [true]);
    finish(&mut engine, id, &slots);
}

#[test]
fn queued_input_retirement_rejects_values_and_callbacks_before_application() {
    let (mut engine, id, slots, input) = create();
    let connected = path(&engine, id, "values/info/left_controller_connected");
    input.lock().unwrap().push_back(state(true, 0.0));
    until(|| queued(&mut engine, id) == 1);
    let mut ctx = context(&engine);
    module(&mut engine, id).service_io(&mut ctx);
    engine.absorb_edits(&mut ctx).unwrap();
    finish(&mut engine, id, &slots);
    assert!(matches!(
        engine.apply_edits(),
        Err(golden_core::engine::EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(value(&engine, connected), ParamValue::Bool(false));
    assert!(buttons(&engine).is_empty());
}

#[test]
fn idle_worker_has_no_periodic_actor_callbacks_or_snapshot_rebuilds() {
    let (mut engine, id, slots, _) = create();
    for _ in 0..20 {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        engine.apply_edits().unwrap();
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
    }
    for _ in 0..100 {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        let stats = engine.tick_stats();
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_rebuilds, 0);
    }
    finish(&mut engine, id, &slots);
}

#[test]
fn activity_after_long_idle_uses_its_own_deadline_and_processing_cap_does_not_accumulate_debt() {
    let (mut engine, id, slots, input) = create();
    let activity = path(&engine, id, "values/info/activity");
    module(&mut engine, id).service.last_turn = Instant::now() - Duration::from_secs(10);
    input.lock().unwrap().push_back(state(true, 0.0));
    until(|| queued(&mut engine, id) == 1);
    service(&mut engine, id);
    assert_eq!(value(&engine, activity), ParamValue::Bool(true));
    assert!(module(&mut engine, id).service.activity_until.unwrap() > Instant::now());
    let cap = path(&engine, id, "connection/processing_fps_cap");
    engine.edits.push(Edit::SetParam {
        node: cap,
        value: ParamValue::Int(1),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    module(&mut engine, id).service.last_turn = Instant::now() - Duration::from_secs(10);
    input.lock().unwrap().push_back(state(true, 0.5));
    until(|| queued(&mut engine, id) == 1);
    service(&mut engine, id);
    assert_eq!(module(&mut engine, id).last_state.left.stick_x, 0.5);
    input.lock().unwrap().push_back(state(true, 0.8));
    until(|| queued(&mut engine, id) == 1);
    service(&mut engine, id);
    assert_eq!(module(&mut engine, id).last_state.left.stick_x, 0.5);
    assert_eq!(
        module(&mut engine, id).pending_state.as_ref().unwrap().left.stick_x,
        0.8
    );
    module(&mut engine, id).service.activity_until = Some(Instant::now() - Duration::from_millis(1));
    service(&mut engine, id);
    assert_eq!(value(&engine, activity), ParamValue::Bool(false));
    finish(&mut engine, id, &slots);
}

#[test]
fn occupied_native_lease_uses_absolute_escalating_recovery_deadlines() {
    let (mut engine, id, slots, _) = create();
    finish(&mut engine, id, &slots);
    module(&mut engine, id).runtime_dirty = true;
    let permit = module(&mut engine, id).service.owner.try_reserve().unwrap();
    for delay in [250, 500, 1000] {
        if delay != 250 {
            module(&mut engine, id).service.retry_at = Some(Instant::now() - Duration::from_millis(1));
        }
        let before = Instant::now();
        service(&mut engine, id);
        let retry = module(&mut engine, id).service.retry_at.unwrap();
        assert!(retry >= before + Duration::from_millis(delay));
        assert!(retry < before + Duration::from_millis(delay + 100));
        for _ in 0..20 {
            service(&mut engine, id);
        }
        assert_eq!(module(&mut engine, id).service.retry_at, Some(retry));
    }
    drop(permit);
}
