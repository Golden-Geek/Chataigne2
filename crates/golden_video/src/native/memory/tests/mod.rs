use super::*;

fn segment(budget: u64) -> MemorySegmentSnapshot {
    MemorySegmentSnapshot {
        current_usage_bytes: 7,
        budget_bytes: budget,
        available_for_reservation_bytes: budget,
        current_reservation_bytes: 0,
    }
}

fn observation(local: Option<u64>) -> AdapterMemorySnapshot {
    AdapterMemorySnapshot {
        adapter: AdapterId { low: 1, high: -2 },
        process_id: 42,
        observed_qpc: 123,
        qpc_frequency: 10_000_000,
        dedicated_video_bytes: 128 * 1024 * 1024,
        shared_system_capacity_bytes: u64::MAX,
        local: local.map(segment),
        local_hresult: local.is_none().then_some(-1),
        nonlocal: Some(segment(u64::MAX)),
        nonlocal_hresult: None,
    }
}

#[test]
fn admission_uses_the_smaller_workload_or_half_local_budget_without_overflow() {
    for (budget, ceiling, expected) in [
        (100, 100, 50),
        (100, 20, 20),
        (9, 100, 4),
        (1, 100, 0),
        (100, 0, 0),
        (u64::MAX, u64::MAX, u64::MAX / 2),
    ] {
        assert_eq!(observation(Some(budget)).admission_limit(ceiling), Ok(expected));
    }
    let mib = 1024 * 1024;
    assert_eq!(observation(Some(4096 * mib)).admission_limit(512 * mib), Ok(512 * mib));
    assert_eq!(
        observation(Some(4096 * mib)).admission_limit(1024 * mib),
        Ok(1024 * mib)
    );
}

#[test]
fn missing_or_zero_local_budget_cannot_fall_back_to_nonlocal_or_capacity() {
    assert_eq!(
        observation(None).admission_limit(1024),
        Err(MemoryAdmissionError::MissingLocalBudget)
    );
    assert_eq!(
        observation(Some(0)).admission_limit(1024),
        Err(MemoryAdmissionError::ZeroLocalBudget)
    );
}

#[test]
fn serialized_unknown_and_zero_observations_remain_distinct() {
    let missing = serde_json::to_value(observation(None)).unwrap();
    assert!(missing["local"].is_null());
    assert_eq!(missing["local_hresult"], -1);
    assert_eq!(missing["adapter"]["high"], -2);
    assert_eq!(missing["shared_system_capacity_bytes"], u64::MAX);
    let zero = serde_json::to_value(observation(Some(0))).unwrap();
    assert_eq!(zero["local"]["budget_bytes"], 0);
    assert_eq!(zero["local"]["current_usage_bytes"], 7);
    assert!(zero["local_hresult"].is_null());
}
