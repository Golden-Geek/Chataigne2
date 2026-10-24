use super::*;

#[test]
fn removed_ordinals_restore_order_independently_of_removal_order() {
    let mut records = OrderedRecords::default();
    for key in 0..8 {
        records.insert(None, key, key * 10);
    }
    let removed = [5, 1, 6, 0].map(|key| (key, records.remove(&key).unwrap()));
    assert_eq!(records.values().copied().collect::<Vec<_>>(), [20, 30, 40, 70]);
    for (key, (order, value)) in removed {
        records.insert(Some(order), key, value);
    }
    assert_eq!(
        records.values().copied().collect::<Vec<_>>(),
        (0..8).map(|n| n * 10).collect::<Vec<_>>()
    );
}

#[test]
fn equality_preserves_identity_and_order_but_ignores_retired_ordinals() {
    let mut original = OrderedRecords::default();
    original.insert(None, 1, "one");
    original.insert(None, 2, "two");
    original.insert(None, 3, "three");
    original.remove(&2);
    let mut loaded = OrderedRecords::default();
    loaded.insert(None, 1, "one");
    loaded.insert(None, 3, "three");
    assert_eq!(original, loaded);
    *loaded.get_mut(&3).unwrap() = "changed";
    assert_ne!(original, loaded);
    *loaded.get_mut(&3).unwrap() = "three";
    let (order, value) = loaded.remove(&1).unwrap();
    loaded.insert(None, 1, value);
    assert_ne!(original, loaded);
    loaded.remove(&1);
    loaded.insert(Some(order), 1, value);
    assert_eq!(original, loaded);
}

#[test]
fn rollback_can_reuse_then_restore_the_last_authored_position() {
    let mut records = OrderedRecords::default();
    records.insert(None, 1, "one");
    records.insert(None, 2, "two");
    let before = records.clone();
    let (order, value) = records.remove(&2).unwrap();
    records.insert(None, 3, "temporary");
    records.remove(&3);
    records.insert(Some(order), 2, value);
    assert_eq!(records, before);
}
