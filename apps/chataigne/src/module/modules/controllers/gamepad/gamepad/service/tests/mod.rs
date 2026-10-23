use super::*;
use crate::app::{AppEngine, AppNode};
use gamepad_runtime::{GamepadButton, GamepadRuntimeEvent, GamepadStateSnapshot, Record};
use golden_core::{edit::Edit, node::Folder, process_ctx::ExecutionPhase};
use golden_io::PayloadPool;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
};
mod fixture;
use fixture::*;
mod projection;

pub(in super::super) fn assert_selected_event_values() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.rows.lock().unwrap().extend([
        Record::Event(GamepadRuntimeEvent::Connected { device: device() }),
        Record::Event(GamepadRuntimeEvent::AxisChanged {
            device: device(),
            axis: GamepadAxis::LeftStickX,
            value: 0.55,
        }),
        edge(true),
    ]);
    until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().queued() >= 3);
    let axis = path(&engine, id, "values/axes/left_stick");
    let button = path(&engine, id, "values/buttons/south");
    until(|| {
        tick(&mut engine);
        value(&engine, button) == ParamValue::Bool(true)
    });
    assert_eq!(value(&engine, axis), ParamValue::Vec2(0.55, 0.0));
    finish(&mut engine, id, &native);
}
#[test]
fn healthy_service_needs_no_idle_actor_callbacks_wakes_or_snapshots() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    for _ in 0..100 {
        tick(&mut engine);
        let stats = engine.tick_stats();
        assert_eq!(
            (
                stats.nodes_due,
                stats.callbacks_fired,
                stats.wake_callbacks_fired,
                stats.snapshot_builds
            ),
            (0, 0, 0, 0)
        );
    }
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}
#[test]
fn missing_snapshot_preserves_pending_input() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.rows.lock().unwrap().push_back(edge(true));
    until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().has_pending());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    module(&mut engine, id).service_runtime(&mut ctx);
    assert!(module(&mut engine, id).service.runtime.as_ref().unwrap().has_pending());
    let button = path(&engine, id, "values/buttons/south");
    until(|| {
        tick(&mut engine);
        value(&engine, button) == ParamValue::Bool(true)
    });
    finish(&mut engine, id, &native);
}
#[test]
fn continuous_input_preserves_512_callback_occurrences_and_unrelated_actor_progress() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    control.rows.lock().unwrap().extend((0..512).map(|i| edge(i % 2 == 0)));
    let mut count = 0;
    until(|| {
        let mut ctx = project(&mut engine, id);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        let before = count;
        for event in &engine.inbox.events {
            if let golden_core::events::EventKind::Custom(event) = &event.kind {
                let callback = &event.payload["callback"];
                if callback == GAMEPAD_BUTTON_PRESSED_CALLBACK || callback == GAMEPAD_BUTTON_RELEASED_CALLBACK {
                    assert_eq!(
                        callback,
                        if count % 2 == 0 {
                            GAMEPAD_BUTTON_PRESSED_CALLBACK
                        } else {
                            GAMEPAD_BUTTON_RELEASED_CALLBACK
                        }
                    );
                    assert_eq!(event.payload["args"][0], "south");
                    count += 1;
                }
            }
        }
        assert!(count - before <= 16);
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
        engine.edits.push(Edit::PatchMeta {
            node: engine.root,
            patch: golden_core::node::NodeMetaPatch {
                short_name: Some(format!("progress_{count}")),
                ..Default::default()
            },
        });
        engine.apply_edits().unwrap();
        count == 512
    });
    assert_eq!(
        engine.nodes.get(engine.root).unwrap().node_data().meta.short_name,
        "progress_512"
    );
    finish(&mut engine, id, &native);
}
#[test]
fn failure_recovers_on_backoff_without_waiting_for_blocked_cleanup() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let (release, rx) = std::sync::mpsc::channel();
    *control.cleanup.lock().unwrap() = Some(rx);
    control.panic.store(true, Ordering::Release);
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).service.retry_at.is_some()
    });
    let first = module(&mut engine, id).service.retry_at.unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: engine.root,
        patch: golden_core::node::NodeMetaPatch {
            short_name: Some("progress".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    until(|| {
        tick(&mut engine);
        module(&mut engine, id).service.retry_at.is_some_and(|at| at > first)
    });
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    assert_eq!(native.metrics().active, 1);
    release.send(()).unwrap();
    until(|| {
        tick(&mut engine);
        control.opens.load(Ordering::Acquire) == 2
    });
    quiet(&mut engine);
    finish(&mut engine, id, &native);
}
#[test]
fn calibration_reseeds_held_native_state_without_reopening_the_owner() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let mut state = GamepadStateSnapshot {
        device: device(),
        axes: [0.0; 8],
        buttons: [(false, 0.0); 19],
    };
    state.axes[0] = 0.7;
    *control.state.lock().unwrap() = Some(state);
    let offset = path(
        &engine,
        id,
        "parameters/axis_processing/left_stick_x/left_stick_x_offset",
    );
    set(&mut engine, offset, ParamValue::Float(-0.2));
    let axis = path(&engine, id, "values/axes/left_stick");
    until(|| {
        tick(&mut engine);
        matches!(value(&engine,axis),ParamValue::Vec2(x,_) if (x-0.5).abs()<1e-9)
    });
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    assert_eq!(native.metrics().active, 1);
    finish(&mut engine, id, &native);
}
#[test]
fn missing_and_no_input_selection_preserve_parameter_identity_and_choice_metadata() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let selected = path(&engine, id, "connection/device");
    let uuid = engine.nodes.get(selected).unwrap().node_data().meta.uuid;
    // Persisted missing identities arrive with their retained enum choice.
    let snapshot = engine.process_tree_snapshot();
    let node = snapshot.node(selected).unwrap();
    let mut options = gamepad_device_options(&[device()]);
    options.push(missing_gamepad_option("gamepad:9|Missing"));
    engine.edits.push(Edit::SetParamEnumOptions {
        node: selected,
        expected_uuid: node.uuid,
        expected_value: node.param_value.clone().unwrap(),
        selected_value: None,
        options: options.into(),
    });
    engine.apply_edits().unwrap();
    for selection in ["gamepad:9|Missing", "none", "auto"] {
        set(&mut engine, selected, ParamValue::Enum(selection.into()));
        until(|| {
            tick(&mut engine);
            module(&mut engine, id)
                .service
                .catalog
                .as_ref()
                .is_some_and(|r| r.current() && r.prepared.catalog.selection == selection)
        });
        assert_eq!(value(&engine, selected), ParamValue::Enum(selection.into()));
        assert_eq!(engine.nodes.get(selected).unwrap().node_data().meta.uuid, uuid);
        assert!(module(&mut engine, id)
            .service
            .catalog
            .as_ref()
            .unwrap()
            .prepared
            .catalog
            .options
            .contains_variant(selection));
    }
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}

#[test]
fn disconnect_and_reconnect_preserve_requested_identity_and_ordered_callbacks() {
    let (mut engine, id, control, native) = create();
    quiet(&mut engine);
    let selected = path(&engine, id, "connection/device");
    set(&mut engine, selected, ParamValue::Enum(device().variant_id));
    until(|| {
        tick(&mut engine);
        module(&mut engine, id)
            .service
            .catalog
            .as_ref()
            .is_some_and(|r| r.current() && r.prepared.catalog.selection == device().variant_id)
    });
    quiet(&mut engine);
    for (connected, callback) in [
        (false, GAMEPAD_DISCONNECTED_CALLBACK),
        (true, GAMEPAD_CONNECTED_CALLBACK),
    ] {
        control.rows.lock().unwrap().push_back(Record::Event(if connected {
            GamepadRuntimeEvent::Connected { device: device() }
        } else {
            GamepadRuntimeEvent::Disconnected { device: device() }
        }));
        until(|| module(&mut engine, id).service.runtime.as_ref().unwrap().has_pending());
        let mut ctx = project(&mut engine, id);
        engine.absorb_edits(&mut ctx).unwrap();
        engine.apply_edits().unwrap();
        assert_eq!(value(&engine, selected), ParamValue::Enum(device().variant_id));
        assert_eq!(
            value(&engine, path(&engine, id, "connection/connected")),
            ParamValue::Bool(connected)
        );
        let callbacks = engine
            .inbox
            .events
            .iter()
            .filter(|e| matches!(&e.kind,golden_core::events::EventKind::Custom(c) if c.payload["callback"]==callback))
            .count();
        assert_eq!(callbacks, 1);
        engine.dispatch_inbox(ExecutionPhase::EngineTick).unwrap();
        engine.apply_edits().unwrap();
    }
    assert_eq!(control.opens.load(Ordering::Acquire), 1);
    finish(&mut engine, id, &native);
}
