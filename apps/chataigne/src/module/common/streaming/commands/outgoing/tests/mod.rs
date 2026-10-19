use super::*;
use crate::app::module::common::streaming::commands::preparation::tests::native_payload;
use golden_io::{BoundedWorker, PayloadPool, PendingSendError, RetirementPool, WorkerLimits};
use std::{
    num::NonZeroUsize,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

// Raw byte inputs are fixture conveniences. Production queues accept only the
// retained command/script contracts, preventing accidental actor-side copies.
impl<'a> From<&'a [u8]> for SendInput<'a> {
    fn from(value: &'a [u8]) -> Self {
        Self::Borrowed(value)
    }
}
impl<'a, const N: usize> From<&'a [u8; N]> for SendInput<'a> {
    fn from(value: &'a [u8; N]) -> Self {
        Self::Borrowed(value)
    }
}
impl<'a> From<&'a Vec<u8>> for SendInput<'a> {
    fn from(value: &'a Vec<u8>) -> Self {
        Self::Borrowed(value)
    }
}
impl From<Vec<u8>> for SendInput<'_> {
    fn from(value: Vec<u8>) -> Self {
        Self::Owned(value)
    }
}

fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(Instant::now() < deadline, "canonical outgoing fixture did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn canonical_outgoing_admission_shares_storage_and_keeps_its_one_full_reservation() {
    let pool = PayloadPool::new("outgoing-shared", 2, 128 * 1024).unwrap();
    let mut data = Vec::with_capacity(64 * 1024);
    data.extend_from_slice(b"abc");
    let pointer = data.as_ptr();
    let payload = native_payload(&pool, data);
    let mut bytes = OutgoingBytes::new(SendInput::Prepared(&payload), 3).unwrap();
    assert_eq!(bytes.as_ptr(), pointer);
    assert_eq!(bytes.weight(), 3, "canonical storage has its separate pool reservation");
    assert_eq!(bytes.encoded_len(), Some(3));
    assert!(
        !payload.is_live(),
        "native acceptance outlives the original engine guard"
    );
    drop(payload);
    bytes.prepare().unwrap();
    assert_eq!(bytes.as_ptr(), pointer, "slice writers never copy the canonical buffer");
    assert_eq!(&*bytes, b"abc");
    assert!(
        pool.try_retain((), 70 * 1024).is_err(),
        "spare canonical byte capacity is still charged even with a free item slot"
    );
    drop(bytes);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn oversized_shared_output_is_rejected_before_cloning_and_owned_conversion_is_native() {
    let pool = PayloadPool::new("outgoing-owned", 1, 128 * 1024).unwrap();
    let payload = native_payload(&pool, vec![7; 64 * 1024]);
    assert!(OutgoingBytes::new(SendInput::Prepared(&payload), 1).is_err());
    let pointer = payload.request.bytes.as_ptr() as usize;
    let bytes = OutgoingBytes::new(SendInput::Prepared(&payload), 64 * 1024).unwrap();
    assert_eq!(bytes.as_ptr() as usize, pointer);
    drop(payload);
    let output = thread::spawn(move || {
        let output = bytes.into_vec().unwrap();
        assert_ne!(
            output.as_ptr() as usize,
            pointer,
            "ownership-consuming APIs copy on this native owner"
        );
        output
    })
    .join()
    .unwrap();
    assert_eq!(output, vec![7; 64 * 1024]);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn full_native_queue_rejection_retires_its_lease_while_the_accepted_prefix_stays_owned() {
    let pool = PayloadPool::new("outgoing-pressure", 2, 256 * 1024).unwrap();
    let lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let (started, observed) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let (delivered, received) = mpsc::channel();
    let mut worker = BoundedWorker::<OutgoingBytes, ()>::spawn(
        "outgoing-pressure",
        &lifetimes,
        WorkerLimits {
            command_items: 1,
            command_weight: 128 * 1024,
            event_items: 1,
            event_weight: 1,
        },
        move |context| {
            started.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(3)).unwrap();
            while let Some(mut bytes) = context.receive() {
                bytes.prepare().unwrap();
                delivered.send((bytes.as_ptr() as usize, bytes.len())).unwrap();
            }
        },
    )
    .unwrap();
    observed.recv_timeout(Duration::from_secs(2)).unwrap();
    let first = native_payload(&pool, vec![1; 64 * 1024]);
    let pointer = first.request.bytes.as_ptr() as usize;
    let bytes = OutgoingBytes::new(SendInput::Prepared(&first), 64 * 1024).unwrap();
    worker.send_weighted(bytes, 64 * 1024).unwrap();
    drop(first);
    let second = native_payload(&pool, vec![2; 64 * 1024]);
    let rejected = OutgoingBytes::new(SendInput::Prepared(&second), 64 * 1024).unwrap();
    let error = worker.send_weighted(rejected, 64 * 1024).unwrap_err();
    assert!(matches!(error, PendingSendError::Full(_)));
    drop(error);
    drop(second);
    // A freed slot proves the rejected lease retired independently of the
    // blocked, still-admitted original. The IO pool owns final destruction.
    wait(|| pool.try_retain((), 1).is_ok());
    assert!(!pool.wait_for_idle(Duration::from_millis(10)));
    worker.finish_admission();
    release.send(()).unwrap();
    assert_eq!(
        received.recv_timeout(Duration::from_secs(2)).unwrap(),
        (pointer, 64 * 1024)
    );
    assert!(lifetimes.wait_for_idle(Duration::from_secs(2)));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
