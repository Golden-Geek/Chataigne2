use super::*;

#[test]
fn filetime_words_preserve_the_full_unsigned_os_counter() {
    assert_eq!(filetime(FILETIME::default()), 0);
    assert_eq!(
        filetime(FILETIME {
            dwLowDateTime: 7,
            dwHighDateTime: 2,
        }),
        (2_u64 << 32) | 7
    );
    assert_eq!(
        filetime(FILETIME {
            dwLowDateTime: u32::MAX,
            dwHighDateTime: u32::MAX,
        }),
        u64::MAX
    );
}

#[test]
fn unavailable_process_counters_serialize_as_unknown_instead_of_zero() {
    let snapshot = ProcessResourceSnapshot {
        process_id: 42,
        observed_qpc: 123,
        qpc_frequency: 10_000_000,
        memory: None,
        memory_hresult: Some(-1),
        handle_count: Some(0),
        handle_count_hresult: None,
        times: None,
        times_hresult: Some(-2),
    };
    let value = serde_json::to_value(snapshot).unwrap();
    assert!(value["memory"].is_null());
    assert_eq!(value["memory_hresult"], -1);
    assert_eq!(value["handle_count"], 0);
    assert!(value["handle_count_hresult"].is_null());
    assert!(value["times"].is_null());
    assert_eq!(value["times_hresult"], -2);
}
