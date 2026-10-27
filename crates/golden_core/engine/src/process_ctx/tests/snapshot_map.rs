use crate::{
    node::NodeId,
    process_ctx::snapshot_map::{SnapshotMap, SnapshotValue},
};
use std::collections::HashMap;

// Test-only payload makes sharing and compaction observable independently of node capture.
#[derive(Clone, Debug, PartialEq)]
struct Value(String);
impl SnapshotValue for Value {
    fn heap_bytes(&self) -> usize {
        self.0.capacity()
    }
}

#[test]
fn changed_keys_preserve_old_views_and_share_untouched_records() {
    let first = SnapshotMap::new(HashMap::from([
        (NodeId(1), Value("first".into())),
        (NodeId(2), Value("second".into())),
    ]));
    let next = first.changed(HashMap::from([
        (NodeId(1), Some(Value("new".into()))),
        (NodeId(3), Some(Value("third".into()))),
    ]));
    assert_eq!(first.get(&NodeId(1)).unwrap().0, "first");
    assert_eq!(next.get(&NodeId(1)).unwrap().0, "new");
    assert!(std::ptr::eq(
        first.get(&NodeId(2)).unwrap(),
        next.get(&NodeId(2)).unwrap()
    ));
    assert_eq!(next.len(), 3);
    assert!(next.retained_bytes() > first.retained_bytes());
}

#[test]
fn deletion_reinsertion_and_repeated_compaction_keep_exact_membership() {
    let original = SnapshotMap::new(HashMap::from([(NodeId(1), Value("original".into()))]));
    let mut map = original.clone();
    for index in 0..40 {
        map = map.changed(HashMap::from([
            (NodeId(1), None),
            (NodeId(2), Some(Value(index.to_string()))),
        ]));
        assert_eq!(map.iter().count(), 1);
        assert!(map.get(&NodeId(1)).is_none());
        map = map.changed(HashMap::from([
            (NodeId(1), Some(Value("restored".into()))),
            (NodeId(2), None),
        ]));
        assert_eq!(map.len(), 1);
        assert_eq!(map.keys().copied().collect::<Vec<_>>(), [NodeId(1)]);
        assert_eq!(map.values().next().unwrap().0, "restored");
    }
    assert_eq!(original.get(&NodeId(1)).unwrap().0, "original");
}

#[test]
fn leases_deduplicate_shared_layers_and_recover_after_all_views_release() {
    use std::time::Duration;
    let pool = golden_io::PayloadPool::new("snapshot-layer-test", 2, 1024 * 1024).unwrap();
    let base = SnapshotMap::new(HashMap::from([(NodeId(1), Value("base".into()))]));
    let next = base.changed(HashMap::from([(NodeId(2), Some(Value("delta".into())))]));
    let base_lease = base.try_lease(&pool).unwrap();
    let next_lease = next.try_lease(&pool).unwrap();
    assert_eq!(base_lease.len(), 1);
    assert_eq!(next_lease.len(), 2);
    let overflow = next.changed(HashMap::from([(NodeId(3), Some(Value("overflow".into())))]));
    assert!(overflow.try_lease(&pool).is_err());
    drop((base_lease, next_lease, base, next, overflow));
    assert!(pool.wait_for_idle(Duration::from_secs(5)));
    let recovered = SnapshotMap::new(HashMap::from([(NodeId(4), Value("recovered".into()))]));
    assert!(recovered.try_lease(&pool).is_ok());
}
