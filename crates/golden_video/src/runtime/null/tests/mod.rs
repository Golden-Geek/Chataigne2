use super::*;
use std::sync::mpsc;
use uuid::Uuid;

#[test]
fn unbound_and_foreign_completion_controls_cannot_claim_a_device() {
    let control = NullControl::new();
    let device = DeviceGeneration::new(Uuid::new_v4(), 0);
    let token = CompletionToken { device, serial: 1 };
    assert!(!control.complete_through(token));
    let _backend = NullBackend::new(NullConfig::default(), device, control.clone()).unwrap();
    assert!(NullBackend::new(NullConfig::default(), device, control.clone()).is_err());
    assert!(control.complete_through(token));
    assert!(!control.complete_through(CompletionToken { serial: 0, ..token }));
}

#[test]
fn explicit_device_loss_wakes_a_worker_only_completion_wait() {
    let control = NullControl::new();
    let worker_control = control.clone();
    let device = DeviceGeneration::new(Uuid::new_v4(), 0);
    let (started, ready) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut backend = NullBackend::new(NullConfig::default(), device, worker_control.clone()).unwrap();
        backend.initialize().unwrap();
        {
            let mut state = worker_control.0.state.lock().unwrap();
            state.snapshot.last_submitted = Some(CompletionToken { device, serial: 1 });
        }
        started.send(()).unwrap();
        assert_eq!(
            backend.wait_for_completion(Duration::from_secs(60)),
            Err(BackendError::DeviceLost)
        );
    });
    ready.recv_timeout(Duration::from_secs(2)).unwrap();
    control.lose_device();
    worker.join().unwrap();
}

#[test]
fn initial_allocation_failures_do_not_initialize_or_replace_the_bound_device() {
    let control = NullControl::new();
    let device = DeviceGeneration::new(Uuid::new_v4(), 0);
    let mut backend = NullBackend::new(
        NullConfig {
            initialization_failures: 2,
            ..Default::default()
        },
        device,
        control.clone(),
    )
    .unwrap();
    assert_eq!(control.snapshot().device, Some(device));
    assert_eq!(control.snapshot().initialization_attempts, 0);
    assert!(!control.snapshot().initialized);
    assert_eq!(backend.completion(), Err(BackendError::AllocationFailed));
    for attempts in 1..=2 {
        assert_eq!(backend.initialize(), Err(BackendError::AllocationFailed));
        let snapshot = control.snapshot();
        assert_eq!(snapshot.initialization_attempts, attempts);
        assert_eq!(snapshot.device, Some(device));
        assert!(!snapshot.initialized);
        assert!(!snapshot.device_lost);
        assert_eq!(snapshot.created, 0);
        assert_eq!(snapshot.reset_attempts, 0);
    }
    backend.initialize().unwrap();
    assert!(control.snapshot().initialized);
    assert_eq!(control.snapshot().initialization_attempts, 3);
    backend.initialize().unwrap();
    assert_eq!(control.snapshot().initialization_attempts, 3);
    drop(backend);
    assert!(!control.snapshot().initialized);
}

#[test]
fn initialization_failure_budget_can_be_released_without_injecting_device_loss() {
    let control = NullControl::new();
    let device = DeviceGeneration::new(Uuid::new_v4(), 0);
    let mut backend = NullBackend::new(
        NullConfig {
            initialization_failures: usize::MAX,
            ..Default::default()
        },
        device,
        control.clone(),
    )
    .unwrap();
    assert_eq!(backend.initialize(), Err(BackendError::AllocationFailed));
    control.fail_next_initializations(0);
    backend.initialize().unwrap();
    let snapshot = control.snapshot();
    assert!(snapshot.initialized);
    assert!(!snapshot.device_lost);
    assert_eq!(snapshot.device, Some(device));
    assert_eq!(snapshot.initialization_attempts, 2);
    assert_eq!(snapshot.reset_attempts, 0);
}
