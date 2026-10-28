use std::sync::mpsc;

use super::*;

#[test]
fn dropping_control_does_not_wait_and_resources_retire_only_on_their_worker_after_completion() {
    let mut h = Harness::new(
        config(),
        NullConfig {
            auto_complete: false,
            ..Default::default()
        },
    );
    let request = h.submit(plan(1, 1, 1));
    h.installed(request);
    h.wait("manual GPU use", |h| h.runtime().snapshot().frames_submitted == 1);
    let token = h.null.snapshot().last_submitted.unwrap();
    let runtime = h.runtime.take().unwrap();
    let test_thread = thread::current().id();
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let drop_thread = thread::spawn(move || {
        let id = thread::current().id();
        drop(runtime);
        done_tx.send(id).unwrap();
    });
    let control_drop_thread = done_rx
        .recv_timeout(Duration::from_millis(500))
        .expect("control Drop must not join GPU retirement");
    drop_thread.join().unwrap();
    assert!(!h.retirement.wait(Duration::from_millis(20)));
    assert_eq!(h.null.snapshot().live, 1);
    assert_eq!(h.null.snapshot().dropped, 0);
    assert!(h.null.complete_through(token));
    assert!(h.retirement.wait(DEADLINE));
    let snapshot = h.null.snapshot();
    assert_eq!(snapshot.live, 0);
    assert_eq!(snapshot.created, snapshot.dropped);
    assert_ne!(snapshot.last_drop_thread, Some(test_thread));
    assert_ne!(snapshot.last_drop_thread, Some(control_drop_thread));
}

#[test]
fn device_loss_rebuilds_the_same_plan_and_old_completion_cannot_retire_new_resources() {
    let mut h = Harness::new(
        config(),
        NullConfig {
            auto_complete: false,
            ..Default::default()
        },
    );
    let request = h.submit(plan(1, 1, 1));
    let (original, _) = h.installed(request);
    h.wait("first device use", |h| h.runtime().snapshot().frames_submitted == 1);
    let old_token = h.null.snapshot().last_submitted.unwrap();
    h.null.lose_device();
    h.advance(20_000_000);
    h.drive("device recreation and unchanged plan", |h| {
        h.runtime().snapshot().resources.is_some_and(|resources| {
            resources.active.is_some_and(|active| {
                active.device.generation() == original.device.generation() + 1 && active.identity == original.identity
            })
        })
    });
    h.wait("new-device submitted use", |h| {
        h.null
            .snapshot()
            .last_submitted
            .is_some_and(|token| token.device != old_token.device && token.serial > 0)
    });
    let rebuilt = h.runtime().snapshot().resources.unwrap().active.unwrap();
    let new_token = h.null.snapshot().last_submitted.unwrap();
    assert_ne!(rebuilt.generation, original.generation);
    assert_eq!(rebuilt.identity, original.identity);
    h.runtime().close();
    assert!(!h.null.complete_through(old_token));
    assert!(!h.retirement.wait(Duration::from_millis(20)));
    assert_eq!(h.null.snapshot().live, 1);
    assert!(h.null.complete_through(new_token));
    assert!(h.retirement.wait(DEADLINE));
    assert_eq!(h.null.snapshot().live, 0);
}
