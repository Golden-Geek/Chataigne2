use super::super::keyboard_runtime::{self, DiscoveredKeyboardDevice, KeyboardRuntimeEvent};
use super::*;
use golden_core::{
    edit::Edit, engine::EngineEditError, events::EventKind, node::NodeMetaPatch, parameter::ParameterEventBehaviour,
    process_ctx::ProcessCtx,
};
use golden_io::{PayloadPool, RetirementPool, WorkerNotifier};
use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

struct Fake(Arc<Mutex<VecDeque<KeyboardRuntimeEvent>>>);
impl keyboard_runtime::Provider for Fake {
    fn poll(&mut self) -> Result<Vec<KeyboardRuntimeEvent>, String> {
        Ok(self.0.lock().unwrap().pop_front().into_iter().collect())
    }
}
fn module(engine: &mut crate::app::AppEngine, id: NodeId) -> &mut KeyboardModule {
    let crate::app::AppNode::KeyboardModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("keyboard fixture")
    };
    module
}
fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
}
fn create(count: usize) -> (crate::app::AppEngine, NodeId, RetirementPool, PayloadPool) {
    let (mut engine, id) = create_keyboard_module();
    let mut input = VecDeque::from([KeyboardRuntimeEvent::DevicesChanged(vec![DiscoveredKeyboardDevice {
        index: 0,
        variant_id: "fake:keyboard".into(),
        label: "Fake Keyboard".into(),
        details: "fixture".into(),
    }])]);
    input.extend((0..count).map(|index| KeyboardRuntimeEvent::Input {
        device: "fake:keyboard".into(),
        event: KeyboardInputEvent::KeyChanged {
            key: KeyboardKey::A,
            pressed: index % 2 == 0,
        },
    }));
    let owner = module(&mut engine, id).input.owner.clone();
    let slots = RetirementPool::new(NonZeroUsize::MIN);
    let input_pool = PayloadPool::new("keyboard-engine-input", 64, 8 * 1024 * 1024).unwrap();
    let provider = Fake(Arc::new(Mutex::new(input)));
    let runtime = keyboard_runtime::KeyboardInputRuntime::spawn_using(
        &owner,
        &slots,
        input_pool.clone(),
        WorkerNotifier::new(|| {}),
        "auto",
        move || Ok(provider),
    )
    .unwrap();
    module(&mut engine, id).input.runtime = Some(runtime);
    until(|| module(&mut engine, id).input.runtime.as_ref().unwrap().queued() >= (count + 1).min(32));
    (engine, id, slots, input_pool)
}
pub(super) fn install_sequence(engine: &mut crate::app::AppEngine, id: NodeId, events: Vec<KeyboardInputEvent>) {
    let input = VecDeque::from([KeyboardRuntimeEvent::DevicesChanged(vec![DiscoveredKeyboardDevice {
        index: 0,
        variant_id: "fake:keyboard".into(),
        label: "Fake Keyboard".into(),
        details: "fixture".into(),
    }])]);
    let input = Arc::new(Mutex::new(input));
    input
        .lock()
        .unwrap()
        .extend(events.into_iter().map(|event| KeyboardRuntimeEvent::Input {
            device: "fake:keyboard".into(),
            event,
        }));
    let target = module(engine, id);
    let notifier = target.input.wake.as_ref().unwrap().notifier.clone();
    target.input.runtime = Some(
        keyboard_runtime::KeyboardInputRuntime::spawn_using(
            &target.input.owner,
            &RetirementPool::new(NonZeroUsize::MIN),
            PayloadPool::new("keyboard-value-fixture", 16, 4 * 1024 * 1024).unwrap(),
            notifier,
            "auto",
            move || Ok(Fake(input)),
        )
        .unwrap(),
    );
    target.request_input_service();
}
fn context(engine: &crate::app::AppEngine) -> ProcessCtx {
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    ctx
}
fn callbacks(engine: &crate::app::AppEngine) -> Vec<bool> {
    engine
        .inbox
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.payload["callback"] == "keyboardKeyChanged" => {
                event.payload["args"][1].as_bool()
            }
            _ => None,
        })
        .collect()
}
#[test]
fn native_turns_preserve_64_edges_and_cap_projection_at_sixteen_records() {
    let (mut engine, id, slots, _) = create(64);
    until(|| {
        let before = callbacks(&engine).len();
        let mut ctx = context(&engine);
        module(&mut engine, id).service_input(&mut ctx);
        engine.absorb_edits(&mut ctx).unwrap();
        engine
            .apply_edits()
            .unwrap_or_else(|error| panic!("projection failed after {before} edges: {error:?}"));
        let after = callbacks(&engine).len();
        assert!(after - before <= 16);
        after == 64
    });
    assert_eq!(
        callbacks(&engine),
        (0..64).map(|index| index % 2 == 0).collect::<Vec<_>>()
    );
    assert_eq!(int_param_value(&engine, id, "values/info/held_key_count"), Some(0));
    module(&mut engine, id).stop_input_backend();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn queued_native_retirement_rejects_values_and_callbacks_and_releases_original_pool() {
    let (mut engine, id, slots, pool) = create(1);
    let mut ctx = context(&engine);
    module(&mut engine, id).service_input(&mut ctx);
    engine.absorb_edits(&mut ctx).unwrap();
    module(&mut engine, id).stop_input_backend();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(!pool.wait_for_idle(Duration::from_millis(10)));
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert!(callbacks(&engine).is_empty());
    assert_eq!(int_param_value(&engine, id, "values/info/held_key_count"), Some(0));
    assert!(pool.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn queued_target_rename_and_receive_disable_reject_before_input_projection() {
    for rename in [true, false] {
        let (mut engine, id, slots, _) = create(1);
        let mut ctx = context(&engine);
        module(&mut engine, id).service_input(&mut ctx);
        if rename {
            let target = find_path(&engine, id, "values/info/last_key").unwrap();
            engine.edits.push(Edit::PatchMeta {
                node: target,
                patch: NodeMetaPatch {
                    short_name: Some("moved_last_key".into()),
                    ..Default::default()
                },
            });
        } else {
            let target = find_path(&engine, id, "connection/receive_enabled").unwrap();
            engine.edits.push(Edit::SetParam {
                node: target,
                value: ParamValue::Bool(false),
                behaviour: ParameterEventBehaviour::Coalesce,
            });
        }
        engine.apply_edits().unwrap();
        engine.absorb_edits(&mut ctx).unwrap();
        assert!(matches!(
            engine.apply_edits(),
            Err(EngineEditError::EditPreconditionFailed { .. })
        ));
        assert!(callbacks(&engine).is_empty());
        module(&mut engine, id).stop_input_backend();
        assert!(slots.wait_for_idle(Duration::from_secs(1)));
    }
}
