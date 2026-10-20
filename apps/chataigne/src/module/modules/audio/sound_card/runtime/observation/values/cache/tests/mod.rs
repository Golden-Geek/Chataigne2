use super::*;

fn bindings(nodes: impl IntoIterator<Item = NodeId>) -> SoundCardValueBindings {
    SoundCardValueBindings {
        runtime_values: nodes.into_iter().collect(),
        ..Default::default()
    }
}

#[test]
fn prepared_slots_are_fixed_under_bound_and_unknown_node_turnover() {
    let bindings = bindings((1..=150).rev().map(|index| NodeId(index * 7)));
    let mut cache = ValueCache::prepare(&bindings).unwrap();
    let bytes = cache.retained_heap_bytes();
    let epoch = ConfigGeneration::INITIAL;
    for node in &bindings.runtime_values {
        cache.store(*node, epoch, &ParamValue::Int(node.0 as i32));
    }
    for node in 2000..10000 {
        cache.store(NodeId(node), epoch, &ParamValue::Bool(true));
    }
    assert_eq!(cache.applied_count(epoch), 150);
    assert_eq!(cache.retained_heap_bytes(), bytes);
    assert!(bindings
        .runtime_values
        .iter()
        .all(|node| cache.matches(*node, epoch, &ParamValue::Int(node.0 as i32))));
}

#[test]
fn cache_generations_change_without_clearing_storage_or_accepting_stale_values() {
    let node = NodeId(9);
    let mut cache = ValueCache::prepare(&bindings([node])).unwrap();
    let bytes = cache.retained_heap_bytes();
    let old = ConfigGeneration::INITIAL;
    let next = old.next();
    cache.store(node, old, &ParamValue::Float(0.25));
    assert!(cache.matches(node, old, &ParamValue::Float(0.25 + OBSERVATION_EPSILON / 2.0)));
    assert!(!cache.matches(node, old, &ParamValue::Float(0.25 + 2.0 * OBSERVATION_EPSILON)));
    assert!(!cache.matches(node, next, &ParamValue::Float(0.25)));
    assert_eq!(cache.applied_count(next), 0);
    cache.store(node, next, &ParamValue::Bool(true));
    assert!(!cache.matches(node, old, &ParamValue::Float(0.25)));
    assert!(cache.matches(node, next, &ParamValue::Bool(true)));
    assert_eq!(cache.retained_heap_bytes(), bytes);
}

#[test]
fn text_is_inline_utf8_and_cannot_retain_large_source_capacity_or_unsupported_values() {
    let node = NodeId(7);
    let mut cache = ValueCache::prepare(&bindings([node])).unwrap();
    let bytes = cache.retained_heap_bytes();
    let mut text = String::with_capacity(1024 * 1024);
    text.push_str(&"é".repeat(32));
    let value = ParamValue::Str(text);
    let epoch = ConfigGeneration::INITIAL;
    cache.store(node, epoch, &value);
    assert!(cache.matches(node, epoch, &value));
    drop(value);
    assert!(cache.matches(node, epoch, &ParamValue::Str("é".repeat(32))));
    cache.store(node, epoch.next(), &ParamValue::Str("x".repeat(65)));
    cache.store(node, epoch.next(), &ParamValue::Vec2(1.0, 2.0));
    assert_eq!(cache.applied_count(epoch.next()), 0);
    assert_eq!(cache.retained_heap_bytes(), bytes);
}

#[test]
fn oversize_slot_sets_reject_before_allocating_cache_storage() {
    let oversized = bindings((0..=MAX_ENTRIES).map(|index| NodeId(index as u64)));
    assert!(ValueCache::prepare(&oversized).is_err());
}

#[test]
fn retained_cache_quota_and_final_destruction_follow_its_last_exported_owner() {
    use std::{
        sync::{mpsc, Mutex},
        time::Duration,
    };
    let bindings = bindings((0..20000).map(NodeId));
    let mut cache = ValueCache::prepare(&bindings).unwrap();
    let (sender, receiver) = mpsc::channel();
    cache.drop_thread = Some(sender);
    let bytes = size_of::<Mutex<ValueCache>>() + 2 * size_of::<usize>() + cache.retained_heap_bytes();
    let pool = golden_io::PayloadPool::new("value-cache-test", 1, bytes).unwrap();
    let owned = pool.try_retain(Mutex::new(cache), bytes).unwrap();
    let exported = owned.clone();
    drop(owned);
    assert!(pool.try_reserve(1).is_err());
    let caller = std::thread::current().id();
    drop(exported);
    assert_ne!(receiver.recv_timeout(Duration::from_secs(2)).unwrap(), caller);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(pool.try_reserve(bytes).is_ok());
}
