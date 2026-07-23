use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use allocation_counter::measure;

use crate::{PlanPublishError, PlanSwapResult, RealtimeScope, acknowledged_plan_exchange, is_realtime_thread};

#[derive(Debug)]
struct DropProbe {
    value: usize,
    drops: Arc<AtomicUsize>,
    realtime_drops: Arc<AtomicUsize>,
}

impl DropProbe {
    fn new(value: usize, drops: &Arc<AtomicUsize>, realtime_drops: &Arc<AtomicUsize>) -> Self {
        Self {
            value,
            drops: Arc::clone(drops),
            realtime_drops: Arc::clone(realtime_drops),
        }
    }
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::Relaxed);
        if is_realtime_thread() {
            self.realtime_drops.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[test]
fn swap_acknowledgement_reclaims_old_plan_only_off_callback() {
    let drops = Arc::new(AtomicUsize::new(0));
    let realtime_drops = Arc::new(AtomicUsize::new(0));
    let (mut publisher, mut realtime) =
        acknowledged_plan_exchange(Box::new(DropProbe::new(0, &drops, &realtime_drops)));
    publisher
        .publish(Box::new(DropProbe::new(1, &drops, &realtime_drops)))
        .unwrap();

    let allocation = measure(|| {
        let _scope = RealtimeScope::enter();
        assert_eq!(realtime.begin_block(), PlanSwapResult::Swapped);
    });

    assert_eq!(allocation.count_total, 0);
    assert_eq!(allocation.count_current, 0);
    assert_eq!(realtime.active().value, 1);
    assert_eq!(drops.load(Ordering::Relaxed), 0);
    assert_eq!(publisher.reclaim_acknowledged(), 1);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
    realtime.retire().reclaim();
    assert_eq!(drops.load(Ordering::Relaxed), 2);
    assert_eq!(realtime_drops.load(Ordering::Relaxed), 0);
}

#[test]
fn one_pending_plan_is_enforced_without_losing_the_rejected_plan() {
    let (mut publisher, _realtime) = acknowledged_plan_exchange(Box::new(0_u64));
    publisher.publish(Box::new(1)).unwrap();
    let error = publisher.publish(Box::new(2)).unwrap_err();
    assert!(matches!(error, PlanPublishError::Pending(_)));
    assert_eq!(*error.into_plan(), 2);
}

#[test]
fn full_return_queue_retains_old_plan_until_a_later_block() {
    let drops = Arc::new(AtomicUsize::new(0));
    let realtime_drops = Arc::new(AtomicUsize::new(0));
    let (mut publisher, mut realtime) =
        acknowledged_plan_exchange(Box::new(DropProbe::new(0, &drops, &realtime_drops)));
    publisher
        .publish(Box::new(DropProbe::new(1, &drops, &realtime_drops)))
        .unwrap();
    realtime.fill_return_queue_for_test(Box::new(DropProbe::new(99, &drops, &realtime_drops)));

    {
        let _scope = RealtimeScope::enter();
        assert_eq!(realtime.begin_block(), PlanSwapResult::SwappedAndRetainedOldPlan);
    }
    assert_eq!(drops.load(Ordering::Relaxed), 0);
    assert!(publisher.has_pending_plan());
    assert_eq!(publisher.reclaim_acknowledged(), 1);
    assert!(publisher.has_pending_plan());

    {
        let _scope = RealtimeScope::enter();
        assert_eq!(realtime.begin_block(), PlanSwapResult::ReturnedRetainedPlan);
    }
    assert_eq!(publisher.reclaim_acknowledged(), 1);
    assert!(!publisher.has_pending_plan());
    assert_eq!(realtime.pressure().plan_return_full, 1);
    realtime.retire().reclaim();
    assert_eq!(realtime_drops.load(Ordering::Relaxed), 0);
}

#[test]
fn million_plan_swaps_have_balanced_control_thread_destruction() {
    const SWAPS: usize = 1_000_000;
    let drops = Arc::new(AtomicUsize::new(0));
    let realtime_drops = Arc::new(AtomicUsize::new(0));
    let (mut publisher, mut realtime) =
        acknowledged_plan_exchange(Box::new(DropProbe::new(0, &drops, &realtime_drops)));

    for value in 1..=SWAPS {
        publisher
            .publish(Box::new(DropProbe::new(value, &drops, &realtime_drops)))
            .unwrap();
        {
            let _scope = RealtimeScope::enter();
            assert_eq!(realtime.begin_block(), PlanSwapResult::Swapped);
        }
        assert_eq!(publisher.reclaim_acknowledged(), 1);
    }

    assert_eq!(realtime.active().value, SWAPS);
    assert_eq!(realtime.metrics().swaps, SWAPS as u64);
    realtime.retire().reclaim();
    drop(publisher);
    assert_eq!(drops.load(Ordering::Relaxed), SWAPS + 1);
    assert_eq!(realtime_drops.load(Ordering::Relaxed), 0);
}

#[test]
fn shutdown_with_a_pending_swap_destroys_every_plan_off_callback() {
    let drops = Arc::new(AtomicUsize::new(0));
    let realtime_drops = Arc::new(AtomicUsize::new(0));
    let (mut publisher, realtime) = acknowledged_plan_exchange(Box::new(DropProbe::new(0, &drops, &realtime_drops)));
    publisher
        .publish(Box::new(DropProbe::new(1, &drops, &realtime_drops)))
        .unwrap();

    realtime.retire().reclaim();
    drop(publisher);

    assert_eq!(drops.load(Ordering::Relaxed), 2);
    assert_eq!(realtime_drops.load(Ordering::Relaxed), 0);
}
