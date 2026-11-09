use super::*;
use crate::native::AdapterOutputs;

fn adapter(low: u32, software: bool, dedicated_video_memory: u64) -> AdapterInfo {
    AdapterInfo {
        id: AdapterId { low, high: 0 },
        physical_id: Ok(physical(low)),
        description: format!("GPU {low}"),
        software,
        vendor_id: 0,
        device_id: 0,
        dedicated_video_memory,
        shared_system_memory: 0,
        outputs: AdapterOutputs::Observed(Vec::new()),
    }
}

fn physical(low: u32) -> PhysicalAdapterId {
    PhysicalAdapterId::from_key(&format!("pnp:PCI\\MODEL\\INSTANCE-{low}")).unwrap()
}

#[test]
fn automatic_obeys_os_preference_instead_of_memory_size_or_display_ownership() {
    let available = [adapter(1, true, u64::MAX), adapter(2, false, 1), adapter(3, false, 100)];
    assert_eq!(select_from(&available, AdapterPreference::Automatic).unwrap().id.low, 2);
    assert!(matches!(
        select_from(&available[..1], AdapterPreference::Automatic),
        Err(AdapterSelectionError::NoHardware)
    ));
}

#[test]
fn explicit_selection_survives_reordering_and_never_substitutes_a_missing_or_software_gpu() {
    let available = [adapter(3, false, 100), adapter(2, false, 1), adapter(1, true, 0)];
    let selected = physical(2);
    assert_eq!(
        select_from(&available, AdapterPreference::Explicit(selected))
            .unwrap()
            .id
            .low,
        2
    );
    for id in [physical(9), physical(1)] {
        assert!(matches!(
            select_from(&available, AdapterPreference::Explicit(id.clone())),
            Err(AdapterSelectionError::Unavailable(actual)) if actual == id
        ));
    }
}

#[test]
fn physical_selection_follows_only_the_same_device_after_luid_replacement_and_rejects_ambiguity() {
    let mut selected = adapter(2, false, 100);
    selected.id = AdapterId { low: 42, high: 5 };
    let other = adapter(3, false, 100);
    let available = [other, selected.clone()];
    assert_eq!(
        select_from(&available, AdapterPreference::Explicit(physical(2)))
            .unwrap()
            .id,
        selected.id
    );
    let ambiguous = [selected.clone(), selected];
    assert!(matches!(
        select_from(&ambiguous, AdapterPreference::Explicit(physical(2))),
        Err(AdapterSelectionError::Ambiguous(_))
    ));
    let mut unknown = adapter(9, false, 100);
    unknown.physical_id = Err(-1);
    assert!(matches!(
        select_from(&[unknown, adapter(3, false, 1)], AdapterPreference::Automatic),
        Err(AdapterSelectionError::IdentityUnavailable(_, -1))
    ));
}

#[test]
fn identity_tokens_preserve_both_words_including_negative_high_bits() {
    for id in [
        AdapterId { low: 0, high: 0 },
        AdapterId {
            low: u32::MAX,
            high: i32::MIN,
        },
    ] {
        assert_eq!(AdapterId::from_key(&id.key()), Some(id));
    }
    for invalid in [
        "0",
        "NVIDIA",
        "dxgi:1:2",
        "dxgi:00000000:00000001:0",
        "dxgi:zz000000:00000001",
    ] {
        assert_eq!(AdapterId::from_key(invalid), None, "{invalid}");
    }
}
