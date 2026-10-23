use super::*;

#[derive(Clone)]
struct Input;
impl Record for Input {
    fn weight(&self) -> Result<usize, String> {
        Ok(INPUT_WEIGHT)
    }
}
#[derive(Clone)]
struct Catalog;
impl Record for Catalog {
    fn weight(&self) -> Result<usize, String> {
        Ok(CATALOG_WEIGHT / 4)
    }
}
#[test]
fn raw_item_pressure_is_nonblocking_explicit_and_keeps_all_accepted_input() {
    let (sender, receiver, failure) = channel();
    for _ in 0..64 {
        sender.send(Input).unwrap();
    }
    assert!(sender.send(Input).unwrap_err().contains("overload"));
    assert!(failure.error.lock().unwrap().as_ref().unwrap().contains("overload"));
    drop(sender);
    let mut accepted = Vec::new();
    receiver.drain_into(&mut accepted, NonZeroUsize::new(64).unwrap());
    assert_eq!(accepted.len(), 64);
}
#[test]
fn raw_catalog_bytes_fill_before_item_capacity() {
    let (sender, receiver, _) = channel();
    for _ in 0..4 {
        sender.send(Catalog).unwrap();
    }
    assert!(sender.send(Catalog).is_err());
    assert_eq!(receiver.metrics().depth, 4);
    assert_eq!(receiver.metrics().retained_weight, 2 * 1024 * 1024);
}
#[test]
fn stopped_producer_is_observable_before_its_sender_or_cleanup_is_dropped() {
    let (sender, receiver, failure) = channel::<Input>();
    drop(NativeStopped(failure.clone()));
    assert!(!receiver.is_disconnected());
    assert!(
        drain(&receiver, &mut Vec::new(), &failure, |_| KeyboardRuntimeEvent::Input {
            device: "fake".into(),
            event: KeyboardInputEvent::KeyChanged {
                key: KeyboardKey::A,
                pressed: true
            }
        })
        .is_err()
    );
    assert!(sender.send(Input).is_err());
}
#[test]
fn native_input_endpoint_cannot_open_in_test_builds() {
    assert!(create().is_err());
}
