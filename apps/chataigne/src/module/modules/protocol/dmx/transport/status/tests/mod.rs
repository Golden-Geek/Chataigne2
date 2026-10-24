use super::*;

#[test]
fn unconsumed_health_is_bounded_latest_state_with_utf8_safe_diagnostics() {
    let mut status = Status::default();
    status.publish(DmxWorkerEvent::Ready);
    for _ in 0..10_000 {
        status.publish(DmxWorkerEvent::Warning("superseded".into()));
        status.publish(DmxWorkerEvent::InputLost);
        status.publish(DmxWorkerEvent::InputRecovered);
    }
    status.publish(DmxWorkerEvent::Warning("é".repeat(2048)));
    assert!(status.has_pending());
    assert!(matches!(status.take(), Some(DmxWorkerEvent::Ready)));
    let Some(DmxWorkerEvent::Warning(warning)) = status.take() else {
        panic!("latest warning")
    };
    assert_eq!(warning, "é".repeat(512));
    assert!(warning.capacity() <= 1024);
    assert!(matches!(status.take(), Some(DmxWorkerEvent::InputRecovered)));
    assert!(!status.has_pending());
    assert!(status.take().is_none());
}

#[test]
fn terminal_error_supersedes_and_survives_other_unconsumed_health() {
    let mut status = Status::default();
    status.publish(DmxWorkerEvent::Ready);
    status.publish(DmxWorkerEvent::Warning("pressure".into()));
    status.publish(DmxWorkerEvent::InputLost);
    status.publish(DmxWorkerEvent::Error("endpoint failed".into()));
    for _ in 0..10_000 {
        status.publish(DmxWorkerEvent::Ready);
        status.publish(DmxWorkerEvent::Warning("new pressure".into()));
        status.publish(DmxWorkerEvent::InputRecovered);
    }
    assert!(matches!(status.take(), Some(DmxWorkerEvent::Error(error)) if error == "endpoint failed"));
    assert!(status.take().is_none());
}
