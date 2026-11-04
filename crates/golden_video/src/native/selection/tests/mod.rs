use super::*;
use crate::native::AdapterOutputs;

fn adapter(low: u32, software: bool, dedicated_video_memory: u64) -> AdapterInfo {
    AdapterInfo {
        id: AdapterId { low, high: 0 },
        description: format!("GPU {low}"),
        software,
        vendor_id: 0,
        device_id: 0,
        dedicated_video_memory,
        shared_system_memory: 0,
        outputs: AdapterOutputs::Observed(Vec::new()),
    }
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
    let selected = AdapterId { low: 2, high: 0 };
    assert_eq!(
        select_from(&available, AdapterPreference::Explicit(selected))
            .unwrap()
            .id,
        selected
    );
    for id in [AdapterId { low: 9, high: 0 }, AdapterId { low: 1, high: 0 }] {
        assert!(matches!(
            select_from(&available, AdapterPreference::Explicit(id)),
            Err(AdapterSelectionError::Unavailable(actual)) if actual == id
        ));
    }
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
