use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn refreshes_coalesce_and_reserve_one_monotonic_completion_without_sdk_work_in_control() {
    let wakes = Arc::new(AtomicUsize::new(0));
    let count = wakes.clone();
    let discovery = Discovery::new(WorkerNotifier::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
    }));
    assert_eq!(discovery.request(false).unwrap(), SenderDiscoverySnapshot::default());
    let first = discovery.request(true).unwrap();
    assert!(first.refreshing);
    assert_eq!(first.revision, 1);
    assert_eq!(discovery.request(true).unwrap(), first);
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    let revision = discovery.take().unwrap();
    assert_eq!(discovery.take(), None);
    assert_eq!(discovery.complete(0, Ok((vec!["obsolete".into()], false))), None);
    let done = discovery
        .complete(revision, Ok((vec!["Exact sender".into()], true)))
        .unwrap();
    assert_eq!(done.revision, 2);
    assert!(!done.refreshing);
    assert!(done.truncated);
    assert_eq!(done.names, ["Exact sender"]);
    assert_eq!(discovery.complete(revision, Err("late".into())), None);
    assert_eq!(discovery.request(false).unwrap(), done);
}

#[test]
fn failed_scan_keeps_previous_names_and_another_explicit_refresh_can_recover() {
    let discovery = Discovery::new(WorkerNotifier::new(|| {}));
    discovery.request(true).unwrap();
    discovery
        .complete(discovery.take().unwrap(), Ok((vec!["Before".into()], false)))
        .unwrap();
    discovery.request(true).unwrap();
    let failed = discovery
        .complete(discovery.take().unwrap(), Err("registry unavailable".into()))
        .unwrap();
    assert!(!failed.refreshing);
    assert_eq!(failed.names, ["Before"]);
    assert_eq!(failed.error.as_deref(), Some("registry unavailable"));
    discovery.request(true).unwrap();
    let recovered = discovery
        .complete(discovery.take().unwrap(), Ok((vec!["Renamed".into()], false)))
        .unwrap();
    assert_eq!(recovered.names, ["Renamed"]);
    assert_eq!(recovered.error, None);
    assert_eq!(recovered.revision, 6);
}

#[test]
fn revision_exhaustion_rejects_before_a_refresh_or_partial_state_change() {
    let discovery = Discovery::new(WorkerNotifier::new(|| {}));
    discovery.state.lock().unwrap().snapshot.revision = u32::MAX - 1;
    assert!(discovery.request(true).is_err());
    assert_eq!(discovery.take(), None);
    assert!(!discovery.request(false).unwrap().refreshing);
}

#[test]
fn escaped_names_fit_the_existing_control_reply_bound_without_losing_normal_capacity() {
    let discovery = Discovery::new(WorkerNotifier::new(|| {}));
    discovery.request(true).unwrap();
    let escaped = (0..MAX_NAMES)
        .map(|index| format!("{index:02}{}", "\u{1}".repeat(253)))
        .collect();
    let snapshot = discovery
        .complete(discovery.take().unwrap(), Ok((escaped, false)))
        .unwrap();
    assert!(snapshot.truncated);
    assert!(snapshot.names.len() < MAX_NAMES);
    assert!(serde_json::to_vec(&snapshot).unwrap().len() < 64 * 1024);
    discovery.request(true).unwrap();
    let normal = (0..MAX_NAMES)
        .map(|index| format!("{index:02}{}", "x".repeat(253)))
        .collect::<Vec<_>>();
    let snapshot = discovery
        .complete(discovery.take().unwrap(), Ok((normal.clone(), false)))
        .unwrap();
    assert_eq!(snapshot.names, normal);
    assert!(!snapshot.truncated);
    assert!(serde_json::to_vec(&snapshot).unwrap().len() < 64 * 1024);
}
