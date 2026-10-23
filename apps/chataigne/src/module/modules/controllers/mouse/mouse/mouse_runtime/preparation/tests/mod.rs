use super::*;
fn source_pool() -> &'static PayloadPool {
    static POOL: OnceLock<PayloadPool> = OnceLock::new();
    POOL.get_or_init(|| PayloadPool::new("mouse-prepared-metadata", 128, 32 * 1024 * 1024).unwrap())
}
fn device(id: &str) -> DiscoveredMouseDevice {
    DiscoveredMouseDevice {
        index: 0,
        variant_id: id.into(),
        label: format!("Mouse {id}"),
        details: "fixture".into(),
    }
}
fn resources() -> (PayloadPool, PayloadPool, NativeCallbackPreparer) {
    let domains = PayloadPool::new("mouse-test-domains", 16, 8 * 1024 * 1024).unwrap();
    let pool = PayloadPool::new("mouse-test-callbacks", 64, 16 * 1024 * 1024).unwrap();
    let callbacks = NativeCallbackPreparer::using(pool.clone());
    (domains, pool, callbacks)
}
fn finish(mut draft: Draft, domains: &PayloadPool, callbacks: &NativeCallbackPreparer) -> Prepared {
    draft.try_finish(source_pool(), domains, callbacks).unwrap()
}
fn catalog(
    preparer: &mut Preparer,
    domains: &PayloadPool,
    callbacks: &NativeCallbackPreparer,
    ids: &[&str],
    selection: &str,
) -> Prepared {
    finish(
        preparer
            .event(
                MouseRuntimeEvent::DevicesChanged(ids.iter().map(|id| device(id)).collect()),
                selection,
            )
            .unwrap()
            .unwrap(),
        domains,
        callbacks,
    )
}
fn input(preparer: &mut Preparer, event: MouseInputEvent) -> Draft {
    preparer
        .event(
            MouseRuntimeEvent::Input {
                device: "a".into(),
                event,
            },
            "auto",
        )
        .unwrap()
        .unwrap()
}
fn edge(preparer: &mut Preparer, pressed: bool) -> Draft {
    input(
        preparer,
        MouseInputEvent::ButtonChanged {
            button: MouseButtonKind::Left,
            pressed,
        },
    )
}
fn frame(row: &Prepared) -> &Frame {
    let Row::Input(frame) = &row.row else {
        panic!("input frame");
    };
    frame
}
#[test]
fn callbacks_freeze_position_delta_buttons_device_and_both_output_states() {
    let (domains, _, callbacks) = resources();
    let mut preparer = Preparer::new(true);
    catalog(&mut preparer, &domains, &callbacks, &["a", "b"], "auto");
    let moved = finish(
        input(
            &mut preparer,
            MouseInputEvent::Moved {
                x: 100,
                y: 200,
                dx: 4,
                dy: -3,
            },
        ),
        &domains,
        &callbacks,
    );
    let pressed = finish(edge(&mut preparer, true), &domains, &callbacks);
    let released = finish(edge(&mut preparer, false), &domains, &callbacks);
    assert_eq!(frame(&moved).position, (100, 200));
    assert!(frame(&pressed).buttons[0]);
    assert!(!frame(&released).buttons[0]);
    for output in [0, 1] {
        let movement = frame(&moved).callbacks[output][0].event(NodeId(1));
        assert_eq!(movement.payload["callback"], MOUSE_MOVED_CALLBACK);
        assert_eq!(movement.payload["args"][1]["x"], 4);
        let changed = frame(&pressed).callbacks[output][0].event(NodeId(1));
        let down = frame(&pressed).callbacks[output][1].event(NodeId(1));
        let state = &changed.payload["args"][2];
        assert_eq!(state["buttons"]["left"], true);
        assert_eq!(state["position"]["y"], 200);
        assert_eq!(state["delta"]["y"], -3);
        assert_eq!(state["device"]["id"], "a");
        assert_eq!(state["device"]["connectedDevices"], 2);
        assert_eq!(state["inputActive"], true);
        assert_eq!(state["outputActive"], output == 1);
        assert_eq!(down.payload["args"][1], *state);
        assert_eq!(down.payload["callback"], MOUSE_BUTTON_PRESSED_CALLBACK);
    }
}
#[test]
fn delta_expiry_is_a_source_fenced_value_row_without_extra_callbacks() {
    let (domains, _, callbacks) = resources();
    let mut preparer = Preparer::new(false);
    catalog(&mut preparer, &domains, &callbacks, &["a"], "auto");
    let moved = finish(
        input(
            &mut preparer,
            MouseInputEvent::Moved {
                x: 10,
                y: 20,
                dx: 2,
                dy: 3,
            },
        ),
        &domains,
        &callbacks,
    );
    let expired = finish(
        preparer.expire_delta(Instant::now() + Duration::from_secs(1)).unwrap(),
        &domains,
        &callbacks,
    );
    assert!(matches!(expired.row, Row::DeltaReset));
    assert!(preparer.expire_delta(Instant::now() + Duration::from_secs(2)).is_none());
    let button = finish(edge(&mut preparer, true), &domains, &callbacks);
    assert_eq!(frame(&moved).delta, (2, 3));
    assert_eq!(frame(&button).delta, (0, 0));
    assert_eq!(frame(&button).position, (10, 20));
}
#[test]
fn unselected_input_and_device_or_selection_changes_reset_worker_state() {
    let (domains, _, callbacks) = resources();
    let mut preparer = Preparer::new(false);
    catalog(&mut preparer, &domains, &callbacks, &["a", "b"], "auto");
    assert!(preparer
        .event(
            MouseRuntimeEvent::Input {
                device: "b".into(),
                event: MouseInputEvent::ButtonChanged {
                    button: MouseButtonKind::Right,
                    pressed: true
                }
            },
            "auto"
        )
        .unwrap()
        .is_none());
    let held = finish(edge(&mut preparer, true), &domains, &callbacks);
    let lost = catalog(&mut preparer, &domains, &callbacks, &["b"], "auto");
    assert!(matches!(lost.row, Row::Catalog { reset: true }));
    catalog(&mut preparer, &domains, &callbacks, &["a"], "auto");
    let after = finish(
        input(
            &mut preparer,
            MouseInputEvent::ButtonChanged {
                button: MouseButtonKind::Right,
                pressed: true,
            },
        ),
        &domains,
        &callbacks,
    );
    assert!(!frame(&after).buttons[0]);
    assert!(frame(&held).buttons[0]);
    let configured = finish(preparer.configure("a").unwrap().unwrap(), &domains, &callbacks);
    assert!(matches!(configured.row, Row::Catalog { reset: true }));
    assert!(preparer.expire_delta(Instant::now() + Duration::from_secs(1)).is_none());
}
#[test]
fn choices_share_canonical_storage_and_preserve_missing_and_label_suffixed_selection() {
    let (domains, _, callbacks) = resources();
    let mut preparer = Preparer::new(false);
    let first = catalog(&mut preparer, &domains, &callbacks, &["a"], "a|old label");
    let repeated = catalog(&mut preparer, &domains, &callbacks, &["a"], "a|old label");
    assert_eq!(first.catalog.selection, "a");
    assert!(first.catalog.options.same_storage(&repeated.catalog.options));
    let missing = catalog(&mut preparer, &domains, &callbacks, &[], "a");
    assert!(missing.catalog.selected.is_none());
    assert!(missing.catalog.options.contains_variant("a"));
}
#[test]
fn callback_batch_admission_is_atomic_and_exported_events_keep_the_original_quota() {
    let (domains, _, _) = resources();
    let pool = PayloadPool::new("mouse-four-callback-slots", 4, 4 * 1024 * 1024).unwrap();
    let callbacks = NativeCallbackPreparer::using(pool.clone());
    let held_draft = NativeCallbackDraft::new("held", vec![]).unwrap();
    let held = callbacks.try_retain(&held_draft).unwrap();
    let mut preparer = Preparer::new(false);
    catalog(&mut preparer, &domains, &callbacks, &["a"], "auto");
    let mut draft = edge(&mut preparer, true);
    assert!(draft.try_finish(source_pool(), &domains, &callbacks).is_none());
    let room = [
        pool.try_retain(1u8, 1).unwrap(),
        pool.try_retain(1u8, 1).unwrap(),
        pool.try_retain(1u8, 1).unwrap(),
    ];
    drop((room, held, held_draft));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let prepared = draft.try_finish(source_pool(), &domains, &callbacks).unwrap();
    let event = frame(&prepared).callbacks[0][0].event(NodeId(1));
    let replay = event.clone();
    assert!(Arc::ptr_eq(&event.payload, &replay.payload));
    drop((prepared, draft, preparer, event));
    assert!(!pool.wait_for_idle(Duration::from_millis(10)));
    drop(replay);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
