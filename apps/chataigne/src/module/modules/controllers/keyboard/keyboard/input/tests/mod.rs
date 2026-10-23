use super::*;
use crate::app::module::common::keyboard::KeyboardKey;
use crate::app::module::script_api::NativeCallbackPreparer;
use crate::app::{AppEngine, AppNode};
use golden_core::{events::EventKind, node::Folder, process_ctx::ExecutionPhase};
use golden_io::PayloadPool;
use keyboard_runtime::{KeyboardInputEvent, KeyboardRuntimeEvent};
use std::{
    collections::{HashSet, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
};

#[derive(Default)]
struct Control {
    records: Mutex<VecDeque<KeyboardRuntimeEvent>>,
    panic: AtomicBool,
    opens: AtomicUsize,
    cleanup: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
}
struct Fake(Arc<Control>);
impl Drop for Fake {
    fn drop(&mut self) {
        let cleanup = self.0.cleanup.lock().unwrap().take();
        if let Some(release) = cleanup {
            let _ = release.recv();
        }
    }
}
impl keyboard_runtime::Provider for Fake {
    fn poll(&mut self) -> Result<Vec<KeyboardRuntimeEvent>, String> {
        assert!(
            !self.0.panic.swap(false, Ordering::AcqRel),
            "controlled Keyboard input failure"
        );
        let mut records = self.0.records.lock().unwrap();
        let count = records.len().min(16);
        Ok(records.drain(..count).collect())
    }
}
fn catalog() -> KeyboardRuntimeEvent {
    KeyboardRuntimeEvent::DevicesChanged(vec![DiscoveredKeyboardDevice {
        index: 0,
        variant_id: "fake:keyboard".into(),
        label: "Fake Keyboard".into(),
        details: "fixture".into(),
    }])
}
fn event(pressed: bool) -> KeyboardRuntimeEvent {
    KeyboardRuntimeEvent::Input {
        device: "fake:keyboard".into(),
        event: KeyboardInputEvent::KeyChanged {
            key: KeyboardKey::A,
            pressed,
        },
    }
}
fn module(engine: &mut AppEngine, id: NodeId) -> &mut KeyboardModule {
    let AppNode::KeyboardModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("Keyboard fixture")
    };
    module
}
fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !predicate() {
        assert!(Instant::now() < deadline, "Keyboard engine fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn tick(engine: &mut AppEngine) {
    engine.apply_edits().unwrap();
    engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
    engine.apply_edits().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    engine.apply_edits().unwrap();
}
fn create() -> (AppEngine, NodeId, Arc<Control>, RetirementPool) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    let mut node = KeyboardModule::create();
    node.disable_backends_for_test();
    engine.add_node(node.into(), None);
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.resolve().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let control = Arc::new(Control::default());
    let shared = control.clone();
    let slots = RetirementPool::new(NonZeroUsize::MIN);
    let native = slots.clone();
    let sources = PayloadPool::new("keyboard-engine-prepared", 64, 16 * 1024 * 1024).unwrap();
    let domains = PayloadPool::new("keyboard-engine-domains", 32, 16 * 1024 * 1024).unwrap();
    let callbacks = PayloadPool::new("keyboard-engine-callbacks", 4096, 256 * 1024 * 1024).unwrap();
    let target = module(&mut engine, id);
    target.input.factory = Some(Box::new(move |owner, notifier, selection| {
        let control = shared.clone();
        keyboard_runtime::KeyboardInputRuntime::spawn_prepared_using(
            owner,
            &native,
            sources.clone(),
            notifier,
            selection,
            Some(keyboard_runtime::Resources {
                domains: domains.clone(),
                callbacks: NativeCallbackPreparer::using(callbacks.clone()),
            }),
            move || {
                control.opens.fetch_add(1, Ordering::AcqRel);
                control.records.lock().unwrap().push_front(catalog());
                Ok(Fake(control))
            },
        )
    }));
    target.suppress_input_backend = false;
    target.request_input_service();
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).input.catalog.is_some()
    });
    (engine, id, control, slots)
}
fn quiet(engine: &mut AppEngine) {
    let mut silent = 0;
    until(|| {
        tick(engine);
        if engine.tick_stats().wake_callbacks_fired == 0 {
            silent += 1;
        } else {
            silent = 0;
        }
        silent >= 20
    });
}
fn finish(engine: &mut AppEngine, id: NodeId, native: &RetirementPool) {
    module(engine, id).stop_input_backend();
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn healthy_input_is_passive_with_no_idle_actor_updates_snapshots_or_worker_wakes() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    for _ in 0..100 {
        tick(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.nodes_due, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_builds, 0);
    }
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}

#[test]
fn no_input_selection_keeps_discovery_choices_and_filters_edges_without_actor_work() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let device = module(&mut engine, id).device.id();
    engine.edits.push(Edit::SetParam {
        node: device,
        value: ParamValue::Enum("none".into()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    until(|| {
        tick(&mut engine);
        module(&mut engine, id)
            .input
            .catalog
            .as_ref()
            .is_some_and(|publication| publication.current() && publication.prepared.catalog.selection == "none")
    });
    let target = module(&mut engine, id);
    assert!(target
        .input
        .catalog
        .as_ref()
        .unwrap()
        .prepared
        .catalog
        .options
        .contains_variant("fake:keyboard"));
    assert!(!target.input_active.get());
    assert!(target.input.runtime.is_some());
    quiet(&mut engine);
    control.records.lock().unwrap().push_back(event(true));
    until(|| control.records.lock().unwrap().is_empty());
    for _ in 0..100 {
        tick(&mut engine);
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
        assert_eq!(engine.tick_stats().snapshot_builds, 0);
    }
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}

#[test]
fn worker_notifications_deliver_512_ordered_edges_in_bounded_actor_turns() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control
        .records
        .lock()
        .unwrap()
        .extend((0..512).map(|index| event(index % 2 == 0)));
    let mut seen = HashSet::new();
    let mut observed = Vec::new();
    until(|| {
        tick(&mut engine);
        let before = observed.len();
        for event in engine.ui_events_since(None) {
            if let EventKind::Custom(event) = &event.kind {
                let payload = &event.payload;
                if payload["callback"] == KEYBOARD_KEY_CHANGED_CALLBACK && seen.insert(Arc::as_ptr(payload) as usize) {
                    observed.push(payload["args"][1].as_bool().unwrap());
                }
            }
        }
        assert!(observed.len() - before <= 16 * engine.tick_stats().wake_callbacks_fired.max(1));
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        observed.len() == 512
    });
    assert_eq!(observed, (0..512).map(|index| index % 2 == 0).collect::<Vec<_>>());
    assert_eq!(module(&mut engine, id).held_key_count.get(), 0);
    finish(&mut engine, id, &native);
}

#[test]
fn native_failure_recovers_through_an_absolute_deadline_without_periodic_actor_service() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.panic.store(true, Ordering::Release);
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).input.retry_at.is_some()
    });
    let retry = module(&mut engine, id).input.retry_at.unwrap();
    assert!(retry > Instant::now());
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    until(|| {
        tick(&mut engine);
        assert_eq!(engine.tick_stats().callbacks_fired, 0);
        control.opens.load(Ordering::Acquire) == 2 && module(&mut engine, id).input.catalog.is_some()
    });
    assert!(module(&mut engine, id).input.retry_at.is_none());
    quiet(&mut engine);
    finish(&mut engine, id, &native);
}

#[test]
fn snapshot_free_input_service_preserves_queued_publications_until_a_real_projection_turn() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.records.lock().unwrap().push_back(event(true));
    until(|| module(&mut engine, id).input.runtime.as_ref().unwrap().has_pending());
    let mut missing = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service_input(&mut missing);
    assert!(module(&mut engine, id).input.runtime.as_ref().unwrap().has_pending());
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).held_key_count.get() == 1
    });
    finish(&mut engine, id, &native);
}

#[test]
fn current_authored_selection_rejects_an_old_frame_before_its_param_notification_is_dispatched() {
    for selection in [true, false] {
        let (mut engine, id, control, native) = create();
        quiet(&mut engine);
        control.records.lock().unwrap().push_back(event(true));
        until(|| module(&mut engine, id).input.runtime.as_ref().unwrap().has_pending());
        let publications = module(&mut engine, id)
            .input
            .runtime
            .as_mut()
            .unwrap()
            .poll_events()
            .unwrap();
        let target = module(&mut engine, id);
        let device = if selection {
            target.device.id()
        } else {
            target.receive_enabled.id()
        };
        engine.edits.push(Edit::SetParam {
            node: device,
            value: if selection {
                ParamValue::Enum("none".into())
            } else {
                ParamValue::Bool(false)
            },
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        engine.apply_edits().unwrap();
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        ctx.set_tree_snapshot(engine.process_tree_snapshot());
        assert!(publications[0].current());
        assert!(!module(&mut engine, id).guard_input(&mut ctx, &publications[0]));
        assert_eq!(module(&mut engine, id).held_key_count.get(), 0);
        finish(&mut engine, id, &native);
    }
}

#[test]
fn blocked_native_cleanup_keeps_its_slot_while_actor_edits_and_backoff_recovery_continue() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let (release, cleanup) = std::sync::mpsc::channel();
    *control.cleanup.lock().unwrap() = Some(cleanup);
    control.panic.store(true, Ordering::Release);
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).input.retry_at.is_some()
    });
    let first_retry = module(&mut engine, id).input.retry_at.unwrap();
    let root = engine.root;
    engine.edits.push(Edit::PatchMeta {
        node: root,
        patch: golden_core::node::NodeMetaPatch {
            short_name: Some("still_running".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert_eq!(
        engine.nodes.get(root).unwrap().node_data().meta.short_name,
        "still_running"
    );
    until(|| {
        tick(&mut engine);
        module(&mut engine, id)
            .input
            .retry_at
            .is_some_and(|at| at > first_retry)
    });
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    assert_eq!(native.metrics().active, 1);
    assert_eq!(module(&mut engine, id).input.owner.metrics().active, 1);
    assert_eq!(engine.tick_stats().callbacks_fired, 0);
    release.send(()).unwrap();
    until(|| {
        tick(&mut engine);
        control.opens.load(Ordering::Acquire) == 2 && module(&mut engine, id).input.catalog.is_some()
    });
    finish(&mut engine, id, &native);
}
