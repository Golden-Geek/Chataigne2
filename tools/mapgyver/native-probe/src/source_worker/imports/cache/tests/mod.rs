use super::WeakCache;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

struct DropCounter(Rc<Cell<usize>>);

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn cached_identity_never_adds_a_strong_reader() {
    let identity = Arc::new(7);
    let mut cache = WeakCache::new(3);
    assert_eq!(Arc::strong_count(&identity), 1);
    assert_eq!(cache.insert(&identity, 10), None);
    assert_eq!(Arc::strong_count(&identity), 1);
    assert_eq!(cache.get(&identity), Some(10));
    assert_eq!(Arc::strong_count(&identity), 1);
    assert_eq!(cache.prune(), 0);
    assert_eq!(Arc::strong_count(&identity), 1);
    assert_eq!(cache.clear(), 1);
    assert_eq!(Arc::strong_count(&identity), 1);
}

#[test]
fn equal_metadata_does_not_alias_distinct_allocations() {
    let first = Arc::new([1920, 1080]);
    let second = Arc::new([1920, 1080]);
    let mut cache = WeakCache::new(3);
    cache.insert(&first, "first");
    assert_eq!(cache.get(&second), None);
    cache.insert(&second, "second");
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&first), Some("first"));
    assert_eq!(cache.get(&second), Some("second"));
}

#[test]
fn hits_and_replacements_promote_entries_before_bounded_eviction() {
    let (first, second, third) = (Arc::new(1), Arc::new(2), Arc::new(3));
    let mut cache = WeakCache::new(2);
    assert_eq!(cache.capacity(), 2);
    assert_eq!(cache.insert(&first, 10), None);
    assert_eq!(cache.insert(&second, 20), None);
    assert_eq!(cache.get(&first), Some(10));
    assert_eq!(cache.get(&first), Some(10));
    assert_eq!(cache.insert(&third, 30), Some(20));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&second), None);
    assert_eq!(cache.insert(&first, 11), Some(10));
    assert_eq!(cache.insert(&second, 21), Some(30));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&third), None);
    assert_eq!(cache.get(&first), Some(11));
    assert_eq!(cache.get(&second), Some(21));
}

#[test]
fn expired_identities_wait_for_explicit_pruning_and_do_not_retain_keys() {
    let expired = Arc::new(1);
    let live = Arc::new(2);
    let weak = Arc::downgrade(&expired);
    let mut cache = WeakCache::new(3);
    cache.insert(&expired, 10);
    cache.insert(&live, 20);
    drop(expired);
    assert_eq!(weak.strong_count(), 0);
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&live), Some(20));
    assert_eq!(cache.len(), 2, "get must not implicitly prune");
    let newer = Arc::new(3);
    assert_eq!(cache.insert(&newer, 30), None);
    assert_eq!(cache.len(), 3, "insert must not implicitly prune");
    assert_eq!(cache.prune(), 1);
    assert_eq!(cache.prune(), 0);
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&live), Some(20));
    assert_eq!(cache.get(&newer), Some(30));
    assert!(weak.upgrade().is_none());
}

#[test]
fn clear_drops_cached_values_but_an_active_clone_can_survive() {
    let drops = Rc::new(Cell::new(0));
    let identity = Arc::new(1);
    let cached = Rc::new(DropCounter(drops.clone()));
    let mut cache = WeakCache::new(3);
    cache.insert(&identity, cached.clone());
    let active = cache.get(&identity).unwrap();
    drop(cached);
    assert_eq!(cache.clear(), 1);
    assert_eq!(cache.clear(), 0);
    assert_eq!(cache.len(), 0);
    assert_eq!(drops.get(), 0);
    drop(active);
    assert_eq!(drops.get(), 1);
}

#[test]
#[should_panic(expected = "weak cache capacity must be positive")]
fn zero_capacity_cannot_construct_an_unbounded_or_unusable_cache() {
    let _ = WeakCache::<(), ()>::new(0);
}
