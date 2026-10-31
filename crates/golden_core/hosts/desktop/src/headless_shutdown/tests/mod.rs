use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn sentinel() -> PathBuf {
    std::env::temp_dir().join(format!(
        "golden-headless-stop-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ))
}

#[test]
fn sentinel_requests_stop_and_cancellation_releases_a_waiting_watcher() {
    let path = sentinel();
    let stop = UiServerShutdown::default();
    let watcher = ShutdownFileWatcher::start(Some(path.clone()), stop.clone())
        .unwrap()
        .unwrap();
    assert!(!stop.is_requested());
    std::fs::write(&path, b"stop").unwrap();
    assert!(stop.wait_timeout(Duration::from_secs(2)));
    watcher.finish().unwrap();
    std::fs::remove_file(path).unwrap();
    let watcher = ShutdownFileWatcher::start(Some(sentinel()), UiServerShutdown::default())
        .unwrap()
        .unwrap();
    watcher.finish().unwrap();
}

#[test]
fn invalid_sentinel_observation_reports_failure_without_leaving_the_host_running() {
    let stop = UiServerShutdown::default();
    let watcher = ShutdownFileWatcher::start(Some(PathBuf::from("invalid\0sentinel")), stop.clone())
        .unwrap()
        .unwrap();
    assert!(stop.wait_timeout(Duration::from_secs(2)));
    assert!(
        watcher
            .finish()
            .unwrap_err()
            .to_string()
            .contains("cannot observe shutdown sentinel")
    );
}
