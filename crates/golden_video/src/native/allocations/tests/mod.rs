use std::rc::Rc;

use super::*;

#[test]
fn reused_owners_and_weak_indexes_do_not_duplicate_or_retain_allocation_charges() {
    let owner = TextureAllocations::default();
    let allocation = Rc::new(owner.register(4096));
    let weak = Rc::downgrade(&allocation);
    let reused = allocation.clone();
    assert_eq!(
        (
            owner.snapshot().created,
            owner.snapshot().live,
            owner.snapshot().texel_bytes
        ),
        (1, 1, 4096)
    );
    drop(allocation);
    assert_eq!(owner.snapshot().released, 0);
    drop(reused);
    assert!(weak.upgrade().is_none());
    assert_eq!(
        (
            owner.snapshot().live,
            owner.snapshot().released,
            owner.snapshot().texel_bytes
        ),
        (0, 1, 0)
    );
}

#[test]
fn current_and_peak_ownership_remain_distinct_across_retirement_and_replacement() {
    let owner = TextureAllocations::default();
    let first = owner.register(1024);
    let second = owner.register(2048);
    assert_eq!(
        (owner.snapshot().peak_live, owner.snapshot().peak_texel_bytes),
        (2, 3072)
    );
    drop(first);
    assert_eq!((owner.snapshot().live, owner.snapshot().texel_bytes), (1, 2048));
    drop(second);
    let replacement = owner.register(512);
    assert_eq!((owner.snapshot().created, owner.snapshot().released), (3, 2));
    drop(replacement);
    assert_eq!(
        (
            owner.snapshot().live,
            owner.snapshot().texel_bytes,
            owner.snapshot().released
        ),
        (0, 0, 3)
    );
    assert_eq!(owner.snapshot().peak_texel_bytes, 3072);
}

#[test]
fn partial_sdk_initialization_remains_unknown_instead_of_zero_storage() {
    assert_eq!(OutputTextureAllocation::UnknownSdk.texel_bytes(), None);
}

#[test]
fn aggregate_inventory_retains_unknown_sdk_storage_until_its_endpoint_retires() {
    use crate::native::observation::OutputStatus;
    use crate::native::{D3d11Control, OutputState};
    let control = D3d11Control::default();
    let device = DeviceGeneration::new(uuid::Uuid::new_v4(), 1);
    let output = OutputId::new(1).unwrap();
    control.register_outputs(device, [output].into_iter());
    control.output_state(
        output,
        device,
        OutputStatus {
            state: OutputState::Recovering,
            generation: 1,
            attempts: 1,
            hresult: Some(-1),
            allocation: Some(OutputTextureAllocation::UnknownSdk),
        },
    );
    assert_eq!(control.allocations_snapshot().owned_texel_bytes, None);
    control.output_state(
        output,
        device,
        OutputStatus {
            state: OutputState::Stopped,
            generation: 1,
            attempts: 1,
            hresult: None,
            allocation: None,
        },
    );
    assert_eq!(control.allocations_snapshot().owned_texel_bytes, Some(0));
    assert_eq!(control.allocations_snapshot().driver_physical_bytes, None);
}
