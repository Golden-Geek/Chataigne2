use super::*;

#[test]
fn reserved_capacity_is_charged_before_materialization_and_cancelled_on_drop() {
    let pool = PayloadPool::new("reserved-capacity", 2, 64).unwrap();
    let reservation = pool.try_reserve(64).unwrap();
    assert!(pool.try_reserve(1).is_err());
    assert!(pool.try_retain((), 1).is_err());
    assert!(!pool.wait_for_idle(Duration::ZERO));
    drop(reservation);
    assert!(pool.wait_for_idle(Duration::ZERO));
    let first = pool.try_reserve(0).unwrap();
    let second = pool.try_reserve(0).unwrap();
    assert!(pool.try_reserve(0).is_err(), "zero-byte values still reserve items");
    drop((first, second));
    assert!(pool.try_reserve(usize::MAX).is_err());
    assert!(pool.wait_for_idle(Duration::ZERO));
}

#[test]
fn sealing_releases_unused_bytes_but_keeps_the_used_item_until_disposal() {
    let pool = PayloadPool::new("reserved-shrink", 2, 64).unwrap();
    let reservation = pool.try_reserve(64).unwrap();
    let mut original = Vec::with_capacity(16);
    original.extend([1_u8, 2, 3]);
    let pointer = original.as_ptr();
    let retained = reservation.retain(original, 32).unwrap();
    assert_eq!(retained.as_ptr(), pointer);
    let spare = pool.try_reserve(32).unwrap();
    assert!(pool.try_reserve(0).is_err());
    let other = retained.clone();
    drop((spare, retained));
    assert!(!pool.wait_for_idle(Duration::ZERO));
    drop(other);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(pool.try_reserve(64).is_ok());
}

#[test]
fn oversized_sealing_returns_original_storage_and_cancels_the_reservation() {
    let pool = PayloadPool::new("reserved-oversize", 1, 64).unwrap();
    let reservation = pool.try_reserve(32).unwrap();
    let original = vec![1_u8; 48];
    let pointer = original.as_ptr();
    let returned = reservation.retain(original, 48).unwrap_err().into_value();
    assert_eq!(returned.as_ptr(), pointer);
    assert!(pool.wait_for_idle(Duration::ZERO));
    assert!(pool.try_reserve(64).is_ok());
}

#[test]
fn a_reservation_outlives_the_last_pool_handle_and_preserves_background_disposal() {
    let pool = PayloadPool::new("reserved-last-owner", 1, 64).unwrap();
    let reservation = pool.try_reserve(64).unwrap();
    drop(pool);
    let (tx, rx) = mpsc::channel();
    let value = reservation.retain(Payload(tx), 32).unwrap();
    let other = value.clone();
    drop(value);
    assert!(rx.try_recv().is_err());
    drop(other);
    assert_eq!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), "reserved-last-owner");
}
