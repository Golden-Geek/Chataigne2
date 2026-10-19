use super::*;
use std::sync::mpsc;

struct Payload(mpsc::Sender<String>);
impl Drop for Payload {
    fn drop(&mut self) {
        let _ = self.0.send(thread::current().name().unwrap_or("unnamed").to_string());
    }
}

#[test]
fn final_drop_stays_on_the_disposer_after_pool_owner_and_producer_exit() {
    let pool = PayloadPool::new("payload-disposal", 2, 8).unwrap();
    let (done_tx, done_rx) = mpsc::channel();
    let value = pool.try_retain(Payload(done_tx), 8).unwrap();
    let retained = value.clone();
    assert!(pool.try_retain((), 1).is_err());
    drop(pool);
    drop(value);
    assert!(done_rx.try_recv().is_err());
    drop(retained);
    assert_eq!(
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap(),
        "payload-disposal"
    );
}

#[test]
fn blocked_cleanup_holds_byte_and_item_capacity_without_blocking_consumers() {
    struct Blocked {
        entered: mpsc::Sender<()>,
        gate: Arc<(Mutex<bool>, Condvar)>,
    }
    impl Drop for Blocked {
        fn drop(&mut self) {
            let _ = self.entered.send(());
            let (state, changed) = &*self.gate;
            let lock = state.lock().unwrap();
            drop(changed.wait_while(lock, |open| !*open).unwrap());
        }
    }
    struct Release(Arc<(Mutex<bool>, Condvar)>);
    impl Drop for Release {
        fn drop(&mut self) {
            *self.0.0.lock().unwrap() = true;
            self.0.1.notify_all();
        }
    }
    let pool = PayloadPool::new("blocked-disposer", 1, 8).unwrap();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let release = Release(gate.clone());
    let (entered_tx, entered_rx) = mpsc::channel();
    let value = pool
        .try_retain(
            Blocked {
                entered: entered_tx,
                gate,
            },
            8,
        )
        .unwrap();
    let (dropped_tx, dropped_rx) = mpsc::channel();
    thread::spawn(move || {
        drop(value);
        dropped_tx.send(()).unwrap();
    });
    dropped_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("consumer drop must not run blocked cleanup");
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(pool.try_retain((), 1).is_err());
    drop(release);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    drop(pool.try_retain((), 8).unwrap());
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn a_payload_destructor_panic_does_not_strand_the_pool_or_later_cleanup() {
    struct Fails;
    impl Drop for Fails {
        fn drop(&mut self) {
            panic!("controlled payload destructor failure");
        }
    }
    let pool = PayloadPool::new("panic-disposer", 1, 8).unwrap();
    drop(pool.try_retain(Fails, 8).unwrap());
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let (done_tx, done_rx) = mpsc::channel();
    drop(pool.try_retain(Payload(done_tx), 8).unwrap());
    done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
