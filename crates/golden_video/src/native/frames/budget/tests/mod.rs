use super::*;

fn total(snapshot: GpuFrameBudgetSnapshot) -> u64 {
    [
        snapshot.decoder_pools,
        snapshot.hardware_producers,
        snapshot.hap_producers,
        snapshot.software_producers,
    ]
    .iter()
    .map(|class| class.bytes)
    .sum()
}

#[test]
fn categories_share_admission_and_observers_retain_each_allocation_once() {
    let budget = GpuFrameBudget::new(100);
    let mut owners = Vec::new();
    for (bytes, kind) in [
        (40, AllocationKind::DecoderPool),
        (30, AllocationKind::HardwareProducer),
        (20, AllocationKind::HapProducer),
        (10, AllocationKind::SoftwareProducer),
    ] {
        owners.push(budget.claim(bytes, kind).unwrap());
    }
    let before = budget.snapshot();
    assert_eq!(before.retained_bytes, 100);
    assert_eq!(before.peak_bytes, 100);
    assert_eq!(total(before), 100);
    assert_eq!(
        before.decoder_pools,
        GpuFrameAllocation {
            allocations: 1,
            bytes: 40
        }
    );
    assert_eq!(before.hardware_producers.bytes, 30);
    assert_eq!(before.hap_producers.bytes, 20);
    assert_eq!(before.software_producers.bytes, 10);
    assert!(budget.claim(1, AllocationKind::SoftwareProducer).is_none());
    assert_eq!(budget.snapshot(), before, "rejected allocations change no counter");
    let observers = owners.clone();
    drop(owners);
    assert_eq!(budget.snapshot(), before, "shared observers do not duplicate storage");
    drop(observers);
    let retired = budget.snapshot();
    assert_eq!(retired.retained_bytes, 0);
    assert_eq!(total(retired), 0);
    assert_eq!(retired.peak_bytes, 100);
    assert_eq!(retired.decoder_pools.allocations, 0);
    assert!(budget.claim(100, AllocationKind::DecoderPool).is_some());
}

#[test]
fn zero_and_overflow_rejections_do_not_change_the_inventory() {
    let zero = GpuFrameBudget::new(0);
    assert!(zero.claim(1, AllocationKind::DecoderPool).is_none());
    assert!(zero.claim(0, AllocationKind::DecoderPool).is_none());
    assert_eq!(zero.snapshot().peak_bytes, 0);
    let budget = GpuFrameBudget::new(u64::MAX);
    let held = budget.claim(u64::MAX, AllocationKind::HapProducer).unwrap();
    assert!(budget.claim(1, AllocationKind::HardwareProducer).is_none());
    assert_eq!(budget.snapshot().retained_bytes, u64::MAX);
    drop(held);
    assert_eq!(budget.snapshot().retained_bytes, 0);
}

#[test]
fn concurrent_claims_and_retirement_have_coherent_bounded_snapshots() {
    let budget = GpuFrameBudget::new(64);
    std::thread::scope(|scope| {
        for kind in [
            AllocationKind::DecoderPool,
            AllocationKind::HardwareProducer,
            AllocationKind::HapProducer,
            AllocationKind::SoftwareProducer,
        ] {
            let budget = budget.clone();
            scope.spawn(move || {
                for _ in 0..1000 {
                    let lease = budget.claim(16, kind);
                    let snapshot = budget.snapshot();
                    assert_eq!(snapshot.retained_bytes, total(snapshot));
                    assert!(snapshot.retained_bytes <= snapshot.limit_bytes);
                    assert!(snapshot.peak_bytes <= snapshot.limit_bytes);
                    drop(lease);
                }
            });
        }
    });
    assert_eq!(budget.snapshot().retained_bytes, 0);
    assert_eq!(total(budget.snapshot()), 0);
}
