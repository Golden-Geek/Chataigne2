use super::*;

fn sources() -> &'static PayloadPool {
    static SOURCES: OnceLock<PayloadPool> = OnceLock::new();
    SOURCES.get_or_init(|| PayloadPool::new("keyboard-preparation-metadata", 128, 32 * 1024 * 1024).unwrap())
}
fn device(id: &str) -> DiscoveredKeyboardDevice {
    DiscoveredKeyboardDevice {
        index: 0,
        variant_id: id.into(),
        label: format!("Keyboard {id}"),
        details: "fixture".into(),
    }
}
fn resources() -> (PayloadPool, PayloadPool, NativeCallbackPreparer) {
    let domains = PayloadPool::new("keyboard-preparation-domains", 16, 8 * 1024 * 1024).unwrap();
    let pool = PayloadPool::new("keyboard-preparation-callbacks", 64, 16 * 1024 * 1024).unwrap();
    let callbacks = NativeCallbackPreparer::using(pool.clone());
    (domains, pool, callbacks)
}
fn finish(mut draft: Draft, domains: &PayloadPool, callbacks: &NativeCallbackPreparer) -> Prepared {
    draft.try_finish(sources(), domains, callbacks).unwrap()
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
                KeyboardRuntimeEvent::DevicesChanged(ids.iter().map(|id| device(id)).collect()),
                selection,
            )
            .unwrap()
            .unwrap(),
        domains,
        callbacks,
    )
}
fn edge(preparer: &mut Preparer, key: KeyboardKey, pressed: bool) -> Draft {
    preparer
        .event(
            KeyboardRuntimeEvent::Input {
                device: "a".into(),
                event: KeyboardInputEvent::KeyChanged { key, pressed },
            },
            "auto",
        )
        .unwrap()
        .unwrap()
}
fn frame(row: &Prepared) -> &Frame {
    let Row::Input(frame) = &row.row else {
        panic!("input frame")
    };
    frame
}

#[test]
fn callback_candidates_freeze_current_modifiers_keys_device_and_output_state() {
    let (domains, _, callbacks) = resources();
    let mut preparer = Preparer::new();
    catalog(&mut preparer, &domains, &callbacks, &["a", "b"], "auto");
    let shift = finish(edge(&mut preparer, KeyboardKey::LeftShift, true), &domains, &callbacks);
    let key = finish(edge(&mut preparer, KeyboardKey::A, true), &domains, &callbacks);
    let released = finish(edge(&mut preparer, KeyboardKey::LeftShift, false), &domains, &callbacks);
    assert_eq!(frame(&shift).count, 1);
    assert!(frame(&shift).modifiers[0]);
    assert_eq!(frame(&key).count, 2);
    assert!(!frame(&released).modifiers[0]);
    for output in [0, 1] {
        let changed = frame(&key).callbacks[output][0].event(NodeId(1));
        let pressed = frame(&key).callbacks[output][1].event(NodeId(1));
        let state = &changed.payload["args"][2];
        assert_eq!(state["lastKey"], "a");
        assert_eq!(state["lastPressed"], true);
        assert_eq!(state["modifiers"]["leftShift"], true);
        assert_eq!(state["heldKeys"].as_array().unwrap().len(), 2);
        assert_eq!(state["device"]["id"], "a");
        assert_eq!(state["device"]["connectedDevices"], 2);
        assert_eq!(state["inputActive"], true);
        assert_eq!(state["outputActive"], output == 1);
        assert_eq!(pressed.payload["args"][1], *state);
        assert_eq!(pressed.payload["callback"], KEYBOARD_KEY_PRESSED_CALLBACK);
    }
}

#[test]
fn unselected_edges_are_filtered_and_selection_or_device_loss_resets_held_state() {
    let (domains, _, callbacks) = resources();
    let mut preparer = Preparer::new();
    catalog(&mut preparer, &domains, &callbacks, &["a", "b"], "auto");
    assert!(preparer
        .event(
            KeyboardRuntimeEvent::Input {
                device: "b".into(),
                event: KeyboardInputEvent::KeyChanged {
                    key: KeyboardKey::B,
                    pressed: true
                }
            },
            "auto"
        )
        .unwrap()
        .is_none());
    let held = finish(edge(&mut preparer, KeyboardKey::LeftShift, true), &domains, &callbacks);
    let removed = catalog(&mut preparer, &domains, &callbacks, &["b"], "auto");
    assert!(matches!(removed.row, Row::Catalog { reset: true }));
    let restored = catalog(&mut preparer, &domains, &callbacks, &["a", "b"], "auto");
    assert!(matches!(restored.row, Row::Catalog { reset: true }));
    let key = finish(edge(&mut preparer, KeyboardKey::A, true), &domains, &callbacks);
    assert_eq!(frame(&key).count, 1);
    assert!(!frame(&key).modifiers[0]);
    assert!(frame(&held).modifiers[0]);
    let selection = finish(preparer.configure("a").unwrap().unwrap(), &domains, &callbacks);
    assert!(matches!(selection.row, Row::Catalog { reset: true }));
    let after = finish(edge(&mut preparer, KeyboardKey::B, true), &domains, &callbacks);
    assert_eq!(frame(&after).count, 1);
}

#[test]
fn canonical_choices_reuse_storage_and_missing_or_legacy_selection_keeps_its_meaning() {
    let (domains, _, callbacks) = resources();
    let mut preparer = Preparer::new();
    let first = catalog(&mut preparer, &domains, &callbacks, &["a"], "a|old label");
    assert_eq!(first.catalog.selection, "a");
    let repeated = catalog(&mut preparer, &domains, &callbacks, &["a"], "a|old label");
    assert!(first.catalog.options.same_storage(&repeated.catalog.options));
    let missing = catalog(&mut preparer, &domains, &callbacks, &[], "a");
    assert_eq!(missing.catalog.selection, "a");
    assert_eq!(missing.catalog.options.len(), 3);
    assert!(missing
        .catalog
        .options
        .iter()
        .any(|row| row.variant_id == "a" && row.tags == ["missing"]));
    assert!(missing.catalog.selected.is_none());
}

#[test]
fn exported_choice_domain_retains_original_quota_after_preparation_is_gone() {
    let domains = PayloadPool::new("keyboard-domain-export", 1, 2 * 1024 * 1024).unwrap();
    let (_, _, callbacks) = resources();
    let mut preparer = Preparer::new();
    let prepared = catalog(&mut preparer, &domains, &callbacks, &["a"], "auto");
    let exported = prepared.catalog.options.clone();
    drop((prepared, preparer));
    assert!(!domains.wait_for_idle(Duration::from_millis(10)));
    assert!(domains.try_reserve(1).is_err());
    drop(exported);
    assert!(domains.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn callback_batch_admission_is_atomic_and_replay_copies_keep_the_original_quota() {
    let domains = PayloadPool::new("keyboard-batch-domains", 4, 8 * 1024 * 1024).unwrap();
    let pool = PayloadPool::new("keyboard-batch-callbacks", 4, 4 * 1024 * 1024).unwrap();
    let callbacks = NativeCallbackPreparer::using(pool.clone());
    let held_draft = NativeCallbackDraft::new("held", vec![]).unwrap();
    let held = callbacks.try_retain(&held_draft).unwrap();
    let mut preparer = Preparer::new();
    catalog(&mut preparer, &domains, &callbacks, &["a"], "auto");
    let mut draft = edge(&mut preparer, KeyboardKey::A, true);
    assert!(draft.try_finish(sources(), &domains, &callbacks).is_none());
    // The refused four-payload batch did not occupy the three remaining slots.
    let room = [
        pool.try_retain(1u8, 1).unwrap(),
        pool.try_retain(1u8, 1).unwrap(),
        pool.try_retain(1u8, 1).unwrap(),
    ];
    drop((room, held, held_draft));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let prepared = draft.try_finish(sources(), &domains, &callbacks).unwrap();
    let event = frame(&prepared).callbacks[0][0].event(NodeId(1));
    let replay = event.clone();
    assert!(Arc::ptr_eq(&event.payload, &replay.payload));
    drop((prepared, draft, preparer, event));
    assert!(!pool.wait_for_idle(Duration::from_millis(10)));
    drop(replay);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
