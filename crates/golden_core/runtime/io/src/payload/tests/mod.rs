use super::*;
mod reservations;

#[test]
fn atomic_disposal_claim_rejects_a_new_strong_owner_and_invalidates_weak_revival() {
    let (tx, rx) = mpsc::channel();
    let value = Arc::new(Payload(tx));
    let weak = Arc::downgrade(&value);
    let other = weak.upgrade().unwrap();
    let returned = claim::<Payload>(value).expect_err("a second strong owner prevents disposal");
    assert!(rx.try_recv().is_err());
    drop(other);
    let claimed = claim::<Payload>(returned).unwrap();
    assert!(
        rx.try_recv().is_err(),
        "claiming must not run a destructor under the registry lock"
    );
    assert!(
        weak.upgrade().is_none(),
        "a successful claim prevents weak resurrection"
    );
    drop(claimed);
    rx.recv_timeout(Duration::from_secs(2)).unwrap();
}

#[test]
fn external_arc_leases_share_one_reservation_and_poll_final_raw_release_off_consumer() {
    struct Probe(std::sync::mpsc::Sender<std::thread::ThreadId>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.send(std::thread::current().id()).unwrap();
        }
    }
    let (dropped_tx, dropped_rx) = std::sync::mpsc::channel();
    let raw = Arc::new(Probe(dropped_tx));
    let pool = PayloadPool::new("external-arc-test", 1, 64).unwrap();
    let first = pool.try_retain_arc(raw.clone(), 64).unwrap();
    let second = pool.try_retain_arc(raw.clone(), 64).unwrap();
    let rejected = pool.try_retain_arc(raw.clone(), 65).unwrap_err().into_value();
    assert!(Arc::ptr_eq(&raw, &rejected));
    drop(rejected);
    drop(first);
    drop(second);
    assert!(
        pool.try_retain(1u32, 1).is_err(),
        "ordinary engine owners still retain capacity"
    );
    assert!(!pool.wait_for_idle(Duration::from_millis(150)));
    let consumer = std::thread::current().id();
    drop(raw); // No RetainedPayload notification accompanies this final external release.
    assert_ne!(dropped_rx.recv_timeout(Duration::from_secs(2)).unwrap(), consumer);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(pool.try_retain(7u32, 64).is_ok());
}

#[test]
fn an_external_arc_can_outlive_the_pool_handle_without_losing_its_background_final_owner() {
    struct Probe(std::sync::mpsc::Sender<std::thread::ThreadId>);
    impl Drop for Probe {
        fn drop(&mut self) {
            self.0.send(std::thread::current().id()).unwrap();
        }
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let raw = Arc::new(Probe(tx));
    let pool = PayloadPool::new("external-arc-shutdown", 1, 64).unwrap();
    drop(pool.try_retain_arc(raw.clone(), 64).unwrap());
    drop(pool);
    assert!(rx.recv_timeout(Duration::from_millis(150)).is_err());
    let consumer = std::thread::current().id();
    drop(raw);
    assert_ne!(rx.recv_timeout(Duration::from_secs(2)).unwrap(), consumer);
}
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
