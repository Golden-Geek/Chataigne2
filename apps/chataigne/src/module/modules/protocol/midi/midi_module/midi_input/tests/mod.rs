use super::*;
use std::time::Duration;

fn isolated(values: usize, callbacks: usize) -> (InputPreparer, PayloadPool) {
    let pool = PayloadPool::new("midi-test-callbacks", callbacks, 32 * 1024 * 1024).unwrap();
    (
        InputPreparer {
            values: PayloadPool::new("midi-test-input", values, 4 * 1024 * 1024).unwrap(),
            callbacks: NativeCallbackPreparer::using(pool.clone()),
        },
        pool,
    )
}
fn note(velocity: u8) -> InputDraft {
    InputDraft::new(
        MidiMessage::NoteOn {
            channel: 2,
            note: 60,
            velocity,
        },
        None,
        None,
        false,
    )
    .unwrap()
}
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn maximum_sysex_prepares_hex_and_both_callbacks_with_a_bounded_weight() {
    let (preparer, _) = isolated(1, 2);
    let mut draft = InputDraft::new(
        MidiMessage::System(MidiSystemMessage::Sysex {
            bytes: vec![127; 65534],
        }),
        None,
        None,
        true,
    )
    .unwrap();
    let input = preparer.retain(&mut draft).unwrap();
    assert_eq!(input.hex.as_ref().unwrap().as_str().unwrap().len(), 65534 * 3 - 1);
    assert_eq!(input.callbacks.len(), 2);
    assert_eq!(
        input.callbacks[0].payload()["args"][0]["bytes"]
            .as_array()
            .unwrap()
            .len(),
        65534
    );
    assert_eq!(input.callbacks[1].payload()["args"][0].as_array().unwrap().len(), 65534);
    assert!(input.weight < INPUT_EVENT_BYTES);
    assert!(input.log.as_ref().unwrap().len() < 100);
}

#[test]
fn zero_velocity_keeps_generic_note_on_and_specific_note_off_meaning() {
    let (preparer, _) = isolated(1, 2);
    let input = preparer.retain(&mut note(0)).unwrap();
    assert_eq!(input.callbacks[0].payload()["args"][0]["type"], "noteOn");
    assert_eq!(input.callbacks[1].payload()["callback"], "noteOffReceived");
    assert_eq!(input.callbacks[1].payload()["args"], serde_json::json!([2, 60, 0]));
}

#[test]
fn spare_sysex_capacity_is_rejected_before_json_preparation() {
    let mut bytes = Vec::with_capacity(65537);
    bytes.push(1);
    assert!(InputDraft::new(
        MidiMessage::System(MidiSystemMessage::Sysex { bytes }),
        None,
        None,
        false
    )
    .is_err());
}

#[test]
fn full_value_pool_retries_the_same_envelope_without_rebuilding_callbacks() {
    let (preparer, _) = isolated(1, 4);
    let first = preparer.retain(&mut note(1)).unwrap();
    let mut next = note(2);
    assert!(preparer.retain(&mut next).is_none());
    let pointer = Arc::as_ptr(next.prepared.as_ref().unwrap());
    for _ in 0..10 {
        assert!(preparer.retain(&mut next).is_none());
        assert_eq!(Arc::as_ptr(next.prepared.as_ref().unwrap()), pointer);
    }
    drop(first);
    until(|| preparer.retain(&mut next).is_some());
}

#[test]
fn partial_callback_admission_retains_prefix_and_external_event_lifetime() {
    let (preparer, callbacks) = isolated(2, 2);
    // Occupy one slot in this isolated canonical pool.
    let held = preparer
        .callbacks
        .try_retain(&NativeCallbackDraft::new("held", vec![]).unwrap())
        .unwrap();
    let mut draft = note(3);
    assert!(preparer.retain(&mut draft).is_none());
    assert_eq!(draft.retained.len(), 1);
    let prefix = draft.retained[0].payload() as *const _;
    for _ in 0..10 {
        assert!(preparer.retain(&mut draft).is_none());
        assert_eq!(draft.retained[0].payload() as *const _, prefix);
    }
    drop(held);
    let mut input = None;
    until(|| {
        input = preparer.retain(&mut draft);
        input.is_some()
    });
    let input = input.unwrap();
    let events = input.callbacks.iter().map(|c| c.event(NodeId(1))).collect::<Vec<_>>();
    drop(input);
    drop(draft);
    assert!(!callbacks.wait_for_idle(Duration::from_millis(10)));
    drop(events);
    assert!(callbacks.wait_for_idle(Duration::from_secs(3)));
}
