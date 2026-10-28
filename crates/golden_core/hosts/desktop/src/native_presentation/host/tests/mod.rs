use super::*;

#[test]
fn repeated_exit_requests_wait_for_retirement_and_preserve_the_first_code() {
    let mut retirement = Retirement::default();
    assert_eq!(retirement.request_exit(42), ExitAction::Wait);
    assert_eq!(retirement.request_exit(0), ExitAction::Wait);
    assert_eq!(retirement.complete(true), Some(42));
    assert_eq!(retirement.request_exit(42), ExitAction::DestroyWindow);
    assert_eq!(retirement.request_exit(42), ExitAction::Wait);
    retirement.window_released = true;
    assert_eq!(retirement.request_exit(0), ExitAction::Redirect(42));
    assert_eq!(retirement.request_exit(42), ExitAction::Allow);
}

#[test]
fn spontaneous_completion_keeps_the_editor_alive_and_future_exit_does_not_wait() {
    let mut finished = Retirement::default();
    assert_eq!(finished.complete(true), None);
    assert_eq!(finished.request_exit(0), ExitAction::DestroyWindow);

    let mut failed = Retirement::default();
    assert_eq!(failed.complete(false), None);
    assert_eq!(failed.request_exit(0), ExitAction::DestroyWindow);
}

#[test]
fn worker_failure_overrides_a_pending_successful_exit() {
    let mut failed = Retirement::default();
    assert_eq!(failed.request_exit(0), ExitAction::Wait);
    assert_eq!(failed.complete(false), Some(1));
    assert_eq!(failed.request_exit(0), ExitAction::DestroyWindow);
    failed.window_released = true;
    assert_eq!(failed.request_exit(0), ExitAction::Redirect(1));
    assert_eq!(failed.request_exit(1), ExitAction::Allow);
}

#[test]
fn preparation_failure_is_returned_without_waiting_for_a_worker() {
    let host = PresentationHost::default();
    host.fail_before_start(Error::other("attachment failed"));
    assert_eq!(host.exit_action(1), ExitAction::DestroyWindow);
    assert_eq!(host.window_destroyed(), Some(1));
    assert_eq!(host.exit_action(1), ExitAction::Allow);
    assert_eq!(host.finish().unwrap_err().to_string(), "attachment failed");
}

#[test]
fn disabled_and_headless_hosts_remain_available_on_every_platform() {
    validate_platform(false, false).unwrap();
    validate_platform(true, true).unwrap();
}
