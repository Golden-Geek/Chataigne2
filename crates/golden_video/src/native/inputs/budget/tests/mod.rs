use super::*;

#[test]
fn actual_pool_capacity_is_shared_and_released_only_with_its_last_owner() {
    let budget = Budget::new(100);
    let first = budget.claim(60).unwrap();
    assert!(budget.claim(41).is_none());
    let retained = std::sync::Arc::new(budget.claim(40).unwrap());
    let retiring = retained.clone();
    drop(retained);
    assert!(budget.claim(1).is_none());
    drop(first);
    let replacement = budget.claim(60).unwrap();
    assert!(budget.claim(u64::MAX).is_none());
    drop(retiring);
    assert!(budget.claim(40).is_some());
    drop(replacement);
    assert!(budget.claim(100).is_some());
}

#[test]
fn concurrent_source_claims_never_exceed_the_aggregate_limit() {
    let budget = Budget::new(32);
    let barrier = Arc::new(std::sync::Barrier::new(8));
    let workers = (0..8)
        .map(|_| {
            let budget = budget.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let lease = budget.claim(16);
                barrier.wait();
                lease
            })
        })
        .collect::<Vec<_>>();
    let leases = workers
        .into_iter()
        .filter_map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(leases.len(), 2);
    drop(leases);
    assert!(budget.claim(32).is_some());
}
