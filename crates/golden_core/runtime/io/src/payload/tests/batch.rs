use super::*;

#[test]
fn insufficient_items_or_bytes_registers_no_partial_batch() {
    for (items, bytes) in [(1, 8), (2, 3)] {
        let pool = PayloadPool::new("batch-refusal", items, bytes).unwrap();
        let a = Arc::new(1u8);
        let b = Arc::new(2u8);
        let rejected = pool.try_retain_arcs(vec![(a.clone(), 2), (b.clone(), 2)]).unwrap_err();
        drop(rejected.into_value());
        assert_eq!(Arc::strong_count(&a), 1);
        assert_eq!(Arc::strong_count(&b), 1);
        assert!(pool.try_reserve(bytes).is_ok());
    }
}

#[test]
fn raw_forwarded_arcs_keep_batch_admission_after_all_handles_drop() {
    let pool = PayloadPool::new("batch-canonical", 2, 8).unwrap();
    let a = Arc::new(1u8);
    let b = Arc::new(2u8);
    let handles = pool.try_retain_arcs(vec![(a.clone(), 4), (b.clone(), 4)]).unwrap();
    let forwarded = b.clone();
    drop(handles);
    drop(a);
    drop(b);
    assert!(!pool.wait_for_idle(Duration::from_millis(20)));
    assert!(pool.try_reserve(8).is_err());
    drop(forwarded);
    assert!(pool.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn duplicate_allocations_use_one_largest_reservation_and_preserve_input_order() {
    let pool = PayloadPool::new("batch-duplicates", 1, 8).unwrap();
    let a = Arc::new(3u8);
    let handles = pool.try_retain_arcs(vec![(a.clone(), 1), (a.clone(), 8)]).unwrap();
    assert_eq!(handles.len(), 2);
    assert_eq!(*handles[0], 3);
    assert_eq!(*handles[1], 3);
    assert!(pool.try_retain_arc(a.clone(), 9).is_err());
    drop(handles);
    drop(a);
    assert!(pool.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn existing_registration_is_reused_and_incompatible_weight_cannot_register_a_suffix() {
    let pool = PayloadPool::new("batch-existing", 2, 8).unwrap();
    let a = Arc::new(1u8);
    let first = pool.try_retain_arc(a.clone(), 4).unwrap();
    let b = Arc::new(2u8);
    let rejected = pool.try_retain_arcs(vec![(a.clone(), 5), (b.clone(), 1)]).unwrap_err();
    drop(rejected.into_value());
    assert_eq!(Arc::strong_count(&b), 1);
    let handles = pool.try_retain_arcs(vec![(b.clone(), 4), (a.clone(), 4)]).unwrap();
    assert_eq!(*handles[0], 2);
    assert_eq!(*handles[1], 1);
    drop((handles, first, a, b));
    assert!(pool.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn empty_or_overflowing_batches_leave_capacity_unchanged() {
    let pool = PayloadPool::new("batch-overflow", 2, 8).unwrap();
    assert!(pool.try_retain_arcs::<u8>(vec![]).unwrap().is_empty());
    let error = pool
        .try_retain_arcs(vec![(Arc::new(1u8), usize::MAX), (Arc::new(2u8), 1)])
        .unwrap_err();
    drop(error.into_value());
    assert!(pool.try_reserve(8).is_ok());
}
