use super::super::mouse_runtime::{DiscoveredMouseDevice, MouseInputRuntime, MouseRuntimeEvent, Provider};
use super::*;
use golden_io::{PayloadPool, RetirementPool};
use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Instant,
};

#[derive(Default)]
struct Control {
    input: Mutex<VecDeque<MouseRuntimeEvent>>,
    fail: AtomicBool,
}
struct Fake(Arc<Control>);
impl Provider for Fake {
    fn poll(&mut self) -> Result<Vec<MouseRuntimeEvent>, String> {
        if self.0.fail.load(Ordering::Acquire) {
            return Err("controlled engine input failure".into());
        }
        Ok(self.0.input.lock().unwrap().pop_front().into_iter().collect())
    }
}
fn target(engine: &mut crate::app::AppEngine, id: NodeId) -> &mut MouseModule {
    let crate::app::AppNode::MouseModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!("Mouse module");
    };
    module
}
fn until(engine: &mut crate::app::AppEngine, mut condition: impl FnMut(&crate::app::AppEngine) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition(engine) {
        assert!(Instant::now() < deadline, "engine Mouse fixture deadline");
        run_mouse_tick(engine);
        thread::sleep(Duration::from_millis(1));
    }
}
fn install(engine: &mut crate::app::AppEngine, id: NodeId) -> (Arc<Control>, RetirementPool) {
    let control = Arc::new(Control::default());
    control
        .input
        .lock()
        .unwrap()
        .push_back(MouseRuntimeEvent::DevicesChanged(vec![DiscoveredMouseDevice {
            index: 0,
            variant_id: "fake:mouse".into(),
            label: "Fixture mouse".into(),
            details: "fixture".into(),
        }]));
    let global = RetirementPool::new(NonZeroUsize::MIN);
    let module = target(engine, id);
    let notifier = module
        .wake
        .as_ref()
        .expect("ready hook registers wake")
        .notifier
        .clone();
    let state = control.clone();
    module.input_backend = Some(
        MouseInputRuntime::spawn_using(
            &module.input_owner,
            &global,
            PayloadPool::new("engine-mouse-input", 64, 8 * 1024 * 1024).unwrap(),
            notifier,
            move || Ok(Fake(state)),
        )
        .unwrap(),
    );
    (control, global)
}
fn finish(engine: &mut crate::app::AppEngine, id: NodeId, global: RetirementPool) {
    target(engine, id).stop_input_backend();
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(target(engine, id).input_owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn healthy_native_input_publishes_then_stays_idle_without_actor_polling() {
    let (mut engine, id) = create_mouse_module();
    let (_, global) = install(&mut engine, id);
    until(&mut engine, |engine| {
        bool_param_value(engine, id, "values/info/input_active") == Some(true)
    });
    for _ in 0..5 {
        run_mouse_tick(&mut engine);
    }
    for _ in 0..100 {
        run_mouse_tick(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(stats.nodes_due, 0);
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_builds, 0);
    }
    finish(&mut engine, id, global);
}
#[test]
fn worker_failure_wakes_the_engine_and_clears_held_buttons_without_periodic_service() {
    let (mut engine, id) = create_mouse_module();
    let (control, global) = install(&mut engine, id);
    until(&mut engine, |engine| {
        bool_param_value(engine, id, "values/info/input_active") == Some(true)
    });
    control.input.lock().unwrap().push_back(MouseRuntimeEvent::Input {
        device: "fake:mouse".into(),
        event: MouseInputEvent::ButtonChanged {
            button: MouseButtonKind::Left,
            pressed: true,
        },
    });
    until(&mut engine, |engine| {
        bool_param_value(engine, id, "values/buttons/left") == Some(true)
    });
    control.fail.store(true, Ordering::Release);
    target(&mut engine, id)
        .node_data_mut()
        .meta
        .set_warning(Some("mouse_input_pressure"), "fixture pressure", None);
    until(&mut engine, |engine| {
        bool_param_value(engine, id, "values/buttons/left") == Some(false)
            && bool_param_value(engine, id, "values/info/input_active") == Some(false)
    });
    assert_eq!(
        target(&mut engine, id).last_input_error.as_deref(),
        Some("controlled engine input failure")
    );
    assert_eq!(engine.tick_stats().nodes_due, 0);
    assert!(!target(&mut engine, id)
        .node_data()
        .meta
        .presentation
        .warnings
        .iter()
        .any(|warning| warning.id == "mouse_input_pressure"));
    finish(&mut engine, id, global);
}

#[test]
fn disabling_input_clears_an_existing_pressure_warning() {
    let (mut engine, id) = create_mouse_module();
    for _ in 0..5 {
        run_mouse_tick(&mut engine);
    }
    target(&mut engine, id)
        .node_data_mut()
        .meta
        .set_warning(Some("mouse_input_pressure"), "fixture pressure", None);
    let receive = find_path(&engine, id, "connection/receive_enabled").unwrap();
    engine.edits.push(golden_core::edit::Edit::SetParam {
        node: receive,
        value: ParamValue::Bool(false),
        behaviour: golden_core::parameter::ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().unwrap();
    run_mouse_tick(&mut engine);
    assert!(!target(&mut engine, id)
        .node_data()
        .meta
        .presentation
        .warnings
        .iter()
        .any(|warning| warning.id == "mouse_input_pressure"));
}
