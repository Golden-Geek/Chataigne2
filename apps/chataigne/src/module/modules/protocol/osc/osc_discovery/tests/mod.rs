use super::*;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Instant,
};

fn next(handle: &Discovery) -> DiscoveryEvent {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let (event, stopped) = handle.take();
        if let Some(event) = event {
            return event;
        }
        assert!(!stopped, "discovery stopped before its result");
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn option(value: &str) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: value.into(),
        value: golden_core::parameter::ParamValue::Enum(value.into()),
        label: value.into(),
        tags: Vec::new(),
        ordering: Some(0),
    }
}

#[test]
fn catalog_keeps_a_missing_selection_and_reuses_unchanged_domain_storage() {
    let pool = PayloadPool::new("osc-catalog-test", 4, 256 * 1024).unwrap();
    let selection = Selection::parse("10.0.0.8").unwrap();
    let first = Catalog::prepare(vec![option("any")], selection, None, &pool).unwrap();
    assert!(!first.available);
    assert!(first.options.contains_variant("10.0.0.8"));
    let again = Catalog::prepare(vec![option("any")], selection, Some(&first), &pool).unwrap();
    assert!(again.options.same_storage(&first.options));
    let recovered = Catalog::prepare(vec![option("any"), option("10.0.0.8")], selection, Some(&first), &pool).unwrap();
    assert!(recovered.available);
}

#[test]
fn catalog_rejects_item_byte_and_selection_bounds_before_publication() {
    let pool = PayloadPool::new("osc-catalog-bounds", 4, 256 * 1024).unwrap();
    assert!(Catalog::prepare((0..65).map(|_| option("any")).collect(), Selection::Any, None, &pool).is_err());
    let mut large = option("any");
    large.label = "x".repeat(1025);
    assert!(Catalog::prepare(vec![large], Selection::Any, None, &pool).is_err());
    assert!(Selection::parse(&"x".repeat(65)).is_err());
    assert!(Selection::parse("host.example").is_err());
    let mut full = vec![option("any")];
    full.extend((1..=62).map(|index| option(&format!("10.0.0.{index}"))));
    full.shrink_to_fit();
    let catalog = Catalog::prepare(full, Selection::parse("10.0.1.8").unwrap(), None, &pool).unwrap();
    assert_eq!(catalog.options.len(), 64);
    assert_eq!(catalog.options.capacity(), 64);
}

#[test]
fn coalesced_selection_updates_fence_a_blocked_old_discovery_result() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered, waiting) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let mut gate = Some(gate);
    let handle = Discovery::spawn_using(Selection::Any, WorkerNotifier::new(|| {}), &pool, move || {
        if let Some(gate) = gate.take() {
            entered.send(()).unwrap();
            gate.recv().unwrap();
        }
        Ok(vec![option("any")])
    })
    .unwrap();
    waiting.recv_timeout(Duration::from_secs(3)).unwrap();
    handle.select(Selection::parse("10.0.0.1").unwrap()).unwrap();
    handle.select(Selection::parse("10.0.0.2").unwrap()).unwrap();
    release.send(()).unwrap();
    let old = next(&handle);
    assert!(old.generation < handle.generation());
    let current = next(&handle);
    assert_eq!(current.generation, handle.generation());
    let catalog = current.result.unwrap();
    assert_eq!(catalog.selection, Selection::parse("10.0.0.2").unwrap());
    assert!(catalog.options.contains_variant("10.0.0.2"));
    drop(handle);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn discovery_recovers_provider_failure_without_an_actor_poll_or_selection_edit() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let failing = Arc::new(AtomicBool::new(true));
    let provider = failing.clone();
    let handle = Discovery::spawn_using(Selection::Any, WorkerNotifier::new(|| {}), &pool, move || {
        if provider.load(Ordering::Acquire) {
            Err("injected discovery failure".into())
        } else {
            Ok(vec![option("any")])
        }
    })
    .unwrap();
    assert!(next(&handle).result.is_err());
    failing.store(false, Ordering::Release);
    assert!(next(&handle).result.is_ok());
    drop(handle);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn blocked_discovery_cancels_without_joining_and_retains_its_lifetime_slot() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered, waiting) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let handle = Discovery::spawn_using(Selection::Any, WorkerNotifier::new(|| {}), &pool, move || {
        entered.send(()).unwrap();
        gate.recv().unwrap();
        Ok(vec![option("any")])
    })
    .unwrap();
    waiting.recv_timeout(Duration::from_secs(3)).unwrap();
    let (done, dropped) = mpsc::channel();
    let owner = std::thread::spawn(move || {
        drop(handle);
        done.send(()).unwrap();
    });
    let nonblocking = dropped.recv_timeout(Duration::from_secs(3)).is_ok();
    let rejected = Discovery::spawn_using(Selection::Any, WorkerNotifier::new(|| {}), &pool, || {
        Ok(vec![option("any")])
    });
    release.send(()).unwrap();
    owner.join().unwrap();
    assert!(nonblocking);
    assert!(rejected.is_err());
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    let recovered = Discovery::spawn_using(Selection::Any, WorkerNotifier::new(|| {}), &pool, || {
        Ok(vec![option("any")])
    })
    .unwrap();
    assert!(next(&recovered).result.is_ok());
    drop(recovered);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn blocked_provider_cleanup_does_not_release_capacity_or_join_the_actor() {
    struct Cleanup {
        entered: mpsc::Sender<()>,
        gate: mpsc::Receiver<()>,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            self.entered.send(()).unwrap();
            self.gate.recv().unwrap();
        }
    }
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered, waiting) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let cleanup = Cleanup { entered, gate };
    let handle = Discovery::spawn_using(Selection::Any, WorkerNotifier::new(|| {}), &pool, move || {
        let _keep = &cleanup;
        Ok(vec![option("any")])
    })
    .unwrap();
    assert!(next(&handle).result.is_ok());
    drop(handle);
    waiting.recv_timeout(Duration::from_secs(3)).unwrap();
    let rejected = Discovery::spawn_using(Selection::Any, WorkerNotifier::new(|| {}), &pool, || {
        Ok(vec![option("any")])
    });
    release.send(()).unwrap();
    assert!(rejected.is_err());
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}
