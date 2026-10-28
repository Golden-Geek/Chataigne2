use super::{Ledger, Owner, Work, bgra_bytes};

#[test]
fn clones_alias_one_allocation_and_old_fences_cannot_retire_new_work() {
    let ledger = Ledger::default();
    let texture = ledger
        .allocate(Owner::Application, bgra_bytes(1920, 1080, 1).unwrap(), 1)
        .unwrap();
    let clone = texture.clone();
    ledger.work(Work::Clear);
    let old_fence = ledger.submitted();
    ledger.work(Work::Draw);
    drop(texture);
    assert_eq!(ledger.snapshot().live_textures, 1);
    drop(clone);
    assert_eq!(ledger.snapshot().retiring_textures, 1);
    ledger.complete(old_fence);
    assert_eq!(ledger.snapshot().retiring_textures, 1);
    ledger.complete(ledger.submitted());
    assert_eq!(ledger.snapshot().retiring_textures, 0);
    assert_eq!(ledger.snapshot().live_bytes_by_owner, [0; 4]);
    assert_eq!(ledger.snapshot().retiring_bytes_by_owner, [0; 4]);
}

#[test]
fn resize_counts_old_buffers_until_completion_and_rejects_overflow_atomically() {
    let ledger = Ledger::default();
    let old = ledger
        .allocate(Owner::Swapchain, bgra_bytes(1920, 1080, 2).unwrap(), 2)
        .unwrap();
    ledger.work(Work::Draw);
    drop(old);
    let new = ledger
        .allocate(Owner::Swapchain, bgra_bytes(3840, 2160, 2).unwrap(), 2)
        .unwrap();
    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.live_textures, 2);
    assert_eq!(snapshot.retiring_textures, 2);
    assert_eq!(snapshot.peak_live_plus_retiring_textures, 4);
    assert!(ledger.allocate(Owner::Application, u64::MAX, 1).is_err());
    assert_eq!(ledger.snapshot(), snapshot);
    assert!(bgra_bytes(u32::MAX, u32::MAX, 2).is_err());
    ledger.complete(ledger.submitted());
    drop(new);
    assert_eq!(ledger.snapshot().retiring_textures, 0);
    assert_eq!(ledger.snapshot().live_textures, 0);
}
