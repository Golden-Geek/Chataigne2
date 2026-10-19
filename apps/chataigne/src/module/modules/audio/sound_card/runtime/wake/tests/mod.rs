use super::*;
use golden_core::{engine::Engine, node::Folder};

#[test]
fn repeated_delays_coalesce_and_due_wakes_resume_the_passive_node_once() {
    let mut engine = Engine::new(Folder::new("root"));
    let timer = WakeTimer::new();
    let waker = engine.node_waker(engine.root).unwrap();
    let wake = timer.register(waker.clone()).unwrap();
    assert!(wake.matches(&waker));
    wake.wake_after(Duration::from_secs(2));
    for _ in 0..4096 {
        wake.wake_after(Duration::from_secs(3));
    }
    let mut state = timer.state.lock().unwrap();
    assert_eq!(state.registrations.len(), 1);
    let (due, next) = WakeTimer::collect_due(&mut state, Instant::now());
    assert!(due.is_empty());
    assert!(next.is_some());
    let (due, next) = WakeTimer::collect_due(&mut state, Instant::now() + Duration::from_secs(4));
    assert_eq!(due.len(), 1);
    assert!(next.is_none());
    let (again, _) = WakeTimer::collect_due(&mut state, Instant::now() + Duration::from_secs(5));
    assert!(again.is_empty());
    drop(state);
    due[0].wake().unwrap();
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 1);
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
}

#[test]
fn timer_registration_rejects_before_work_and_recovers_after_ownership_is_released() {
    let engine = Engine::new(Folder::new("root"));
    let waker = engine.node_waker(engine.root).unwrap();
    let timer = WakeTimer::new();
    let mut sources = (0..DELAYED_WAKE_CAPACITY)
        .map(|_| timer.register(waker.clone()).unwrap())
        .collect::<Vec<_>>();
    assert!(timer.register(waker.clone()).is_err());
    sources[0].wake_after(Duration::from_secs(10));
    drop(sources.remove(0));
    assert!(timer.register(waker).is_ok());
    let mut state = timer.state.lock().unwrap();
    let (due, next) = WakeTimer::collect_due(&mut state, Instant::now() + Duration::from_secs(20));
    assert!(due.is_empty(), "dropping a source cancels its delayed wake");
    assert!(next.is_none());
}

#[test]
fn native_timer_worker_receives_a_new_deadline_after_an_idle_wait() {
    let mut engine = Engine::new(Folder::new("root"));
    let wake = RuntimeWakeSender::new(engine.node_waker(engine.root).unwrap()).unwrap();
    wake.wake_after(Duration::from_millis(1));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        if engine.tick_stats().wake_callbacks_fired == 1 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "native timer must publish without actor polling the node"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
}
