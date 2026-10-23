use super::*;
fn device() -> DiscoveredGamepad {
    DiscoveredGamepad {
        index: 0,
        variant_id: "gamepad:0|Pad".into(),
        label: "Pad".into(),
        details: "fixture".into(),
    }
}
fn resources() -> (PayloadPool, PayloadPool, PayloadPool, NativeCallbackPreparer) {
    let source = PayloadPool::new("gamepad-prep-source", 64, 32 * 1024 * 1024).unwrap();
    let domains = PayloadPool::new("gamepad-prep-domains", 16, 16 * 1024 * 1024).unwrap();
    let callbacks = PayloadPool::new("gamepad-prep-callbacks", 64, 4 * 1024 * 1024).unwrap();
    (
        source,
        domains,
        callbacks.clone(),
        NativeCallbackPreparer::using(callbacks),
    )
}
fn edge(button: GamepadButton, pressed: bool) -> GamepadRuntimeEvent {
    if pressed {
        GamepadRuntimeEvent::ButtonPressed {
            device: device(),
            button,
        }
    } else {
        GamepadRuntimeEvent::ButtonReleased {
            device: device(),
            button,
        }
    }
}
#[test]
fn worker_calibration_seeds_native_state_and_freezes_callback_arguments() {
    let (sources, domains, _, callbacks) = resources();
    let mut prep = Preparer::new();
    let mut cfg = Settings::default();
    cfg.axes[0] = AxisProcessing {
        dead_zone: 0.2,
        offset: -0.1,
    };
    let mut state = GamepadStateSnapshot {
        device: device(),
        axes: [0.0; 8],
        buttons: [(false, 0.0); 19],
    };
    state.axes[0] = 0.7;
    let mut catalog = prep.catalog(vec![device()], &cfg, Some(state)).unwrap();
    let first = catalog.try_finish(&sources, &domains, &callbacks).unwrap();
    assert!((first.axes[0] - 0.5).abs() < 1e-9);
    let mut draft = prep
        .event(
            GamepadRuntimeEvent::AxisChanged {
                device: device(),
                axis: GamepadAxis::LeftStickX,
                value: 0.7,
            },
            &cfg,
            None,
        )
        .unwrap()
        .unwrap();
    let row = draft.try_finish(&sources, &domains, &callbacks).unwrap();
    let event = row.callbacks[0].event(golden_core::node::NodeId(1));
    assert_eq!(event.payload["callback"], GAMEPAD_AXIS_CHANGED_CALLBACK);
    assert_eq!(event.payload["args"][0], "left_stick_x");
    assert!((event.payload["args"][1].as_f64().unwrap() - 0.5).abs() < 1e-9);
    assert_eq!(event.payload["args"][2], 0.7);
    assert_eq!(event.payload["args"][3]["id"], device().variant_id);
}
#[test]
fn dpad_precedence_and_analog_button_callbacks_preserve_existing_semantics() {
    let (sources, domains, _, callbacks) = resources();
    let mut prep = Preparer::new();
    let cfg = Settings::default();
    prep.catalog(vec![device()], &cfg, None)
        .unwrap()
        .try_finish(&sources, &domains, &callbacks)
        .unwrap();
    for (event, expected) in [
        (edge(GamepadButton::DPadLeft, true), -1.0),
        (edge(GamepadButton::DPadRight, true), 1.0),
        (edge(GamepadButton::DPadRight, false), -1.0),
    ] {
        let mut draft = prep.event(event, &cfg, None).unwrap().unwrap();
        let row = draft.try_finish(&sources, &domains, &callbacks).unwrap();
        assert_eq!(row.axes[6], expected);
    }
    let mut draft = prep
        .event(
            GamepadRuntimeEvent::ButtonChanged {
                device: device(),
                button: GamepadButton::South,
                value: 0.25,
            },
            &cfg,
            None,
        )
        .unwrap()
        .unwrap();
    let row = draft.try_finish(&sources, &domains, &callbacks).unwrap();
    let event = row.callbacks[0].event(golden_core::node::NodeId(1));
    assert_eq!(event.payload["args"][1], 0.25);
    assert_eq!(event.payload["args"][2], false);
}
#[test]
fn no_input_filters_values_but_preserves_connection_callbacks_and_missing_identity() {
    let (sources, domains, _, callbacks) = resources();
    let mut prep = Preparer::new();
    let mut cfg = Settings {
        requested: "none".into(),
        ..Default::default()
    };
    prep.catalog(vec![device()], &cfg, None)
        .unwrap()
        .try_finish(&sources, &domains, &callbacks)
        .unwrap();
    assert!(prep
        .event(edge(GamepadButton::South, true), &cfg, None)
        .unwrap()
        .is_none());
    let mut connected = prep
        .event(GamepadRuntimeEvent::Connected { device: device() }, &cfg, None)
        .unwrap()
        .unwrap();
    let row = connected.try_finish(&sources, &domains, &callbacks).unwrap();
    assert_eq!(
        row.callbacks[0].event(golden_core::node::NodeId(1)).payload["callback"],
        GAMEPAD_CONNECTED_CALLBACK
    );
    cfg.requested = "gamepad:9|Missing".into();
    let mut draft = prep.catalog(vec![device()], &cfg, None).unwrap();
    let row = draft.try_finish(&sources, &domains, &callbacks).unwrap();
    assert!(row.catalog.options.contains_variant(&cfg.requested));
    assert_eq!(row.catalog.settings.requested, cfg.requested);
    assert!(row.catalog.selected.is_none());
}
#[test]
fn catalog_domains_reuse_storage_and_exported_callbacks_retain_original_quota() {
    let (sources, domains, pool, callbacks) = resources();
    let mut prep = Preparer::new();
    let mut cfg = Settings::default();
    let mut draft = prep.catalog(vec![device()], &cfg, None).unwrap();
    let first = draft.try_finish(&sources, &domains, &callbacks).unwrap();
    cfg.axes[0].dead_zone = 0.5;
    let mut next = prep.catalog(vec![device()], &cfg, None).unwrap();
    let second = next.try_finish(&sources, &domains, &callbacks).unwrap();
    assert!(first.catalog.options.same_storage(&second.catalog.options));
    let mut draft = prep
        .event(edge(GamepadButton::South, true), &cfg, None)
        .unwrap()
        .unwrap();
    let row = draft.try_finish(&sources, &domains, &callbacks).unwrap();
    let event = row.callbacks[0].event(golden_core::node::NodeId(1));
    let raw = event.payload.clone();
    drop((event, row, draft, first, second, prep, next));
    assert!(!pool.wait_for_idle(Duration::from_millis(10)));
    drop(raw);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn oversized_backing_storage_and_nonfinite_input_reject_before_actor_publication() {
    let mut invalid = device();
    invalid.label = String::with_capacity(1025);
    assert!(validate_device(&invalid).is_err());
    let devices = Vec::with_capacity(65);
    assert!(validate_devices(&devices).is_err());
    assert!(validate_event(&GamepadRuntimeEvent::AxisChanged {
        device: device(),
        axis: GamepadAxis::LeftStickX,
        value: f64::NAN
    })
    .is_err());
    assert!(Settings {
        requested: "x".repeat(1025),
        ..Default::default()
    }
    .validate()
    .is_err());
}
