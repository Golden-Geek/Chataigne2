use super::*;
use golden_core::{engine::Engine, node::Folder};

use std::time::Instant;

#[test]
fn native_timer_worker_receives_a_new_deadline_after_an_idle_wait() {
    let mut engine = Engine::new(Folder::new("root"));
    let waker = engine.node_waker(engine.root).unwrap();
    let wake = RuntimeWakeSender::new(waker.clone()).unwrap();
    assert!(wake.matches(&waker));
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
