use super::super::coordination::Coordinator;
use super::*;

type Trace = Arc<Mutex<Vec<(KeyboardKey, bool)>>>;
struct Effects {
    trace: Trace,
    pause: Option<mpsc::Receiver<()>>,
    cleanup: Option<mpsc::Receiver<()>>,
    panic: bool,
}
impl Provider for Effects {
    fn execute(&mut self, request: Request) -> Result<String, String> {
        self.trace.lock().unwrap().push((request.key, true));
        if let Some(pause) = self.pause.take() {
            pause.recv().unwrap();
        }
        assert!(!self.panic, "controlled compound key effect failure");
        self.trace.lock().unwrap().push((request.key, false));
        Ok("compound key effect complete".into())
    }
}
impl Drop for Effects {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            let _ = cleanup.recv();
        }
    }
}
fn spawn_shared(
    coordinator: &Arc<Coordinator>,
    native: &RetirementPool,
    effects: Effects,
) -> (Runtime, RetirementPool) {
    let owner = slots(1);
    let runtime = Runtime::spawn_shared_using(
        &owner,
        native,
        pool(),
        WorkerNotifier::new(|| {}),
        coordinator.clone(),
        move || Ok(effects),
    )
    .unwrap();
    (runtime, owner)
}
fn effects(trace: &Trace) -> Effects {
    Effects {
        trace: trace.clone(),
        pause: None,
        cleanup: None,
        panic: false,
    }
}

#[test]
fn two_modules_cannot_interleave_the_halves_of_a_tap_and_retained_receipts_do_not_block_peers() {
    let coordinator = Coordinator::new();
    let native = slots(2);
    let trace = Trace::default();
    let (release, pause) = mpsc::channel();
    let mut first_effect = effects(&trace);
    first_effect.pause = Some(pause);
    let (first, _) = spawn_shared(&coordinator, &native, first_effect);
    let (second, _) = spawn_shared(&coordinator, &native, effects(&trace));
    let admission = slots(4);
    let ingress = slots(4);
    let first_receipt = submit(&first, source(&first, &admission, &ingress, KeyboardKey::A));
    until(|| trace.lock().unwrap().len() == 1);
    let second_receipt = submit(&second, source(&second, &admission, &ingress, KeyboardKey::B));
    thread::sleep(Duration::from_millis(25));
    assert_eq!(*trace.lock().unwrap(), vec![(KeyboardKey::A, true)]);
    release.send(()).unwrap();
    until(|| first_receipt.complete() && second_receipt.complete());
    assert_eq!(
        *trace.lock().unwrap(),
        vec![
            (KeyboardKey::A, true),
            (KeyboardKey::A, false),
            (KeyboardKey::B, true),
            (KeyboardKey::B, false)
        ]
    );
    assert_eq!(admission.metrics().active, 2);
    assert!(first_receipt.attempted() && second_receipt.attempted());
    drop((first, second));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn older_captured_local_source_orders_before_a_peer_already_in_its_native_mailbox() {
    let coordinator = Coordinator::new();
    let native = slots(2);
    let trace = Trace::default();
    let (first, _) = spawn_shared(&coordinator, &native, effects(&trace));
    let (second, _) = spawn_shared(&coordinator, &native, effects(&trace));
    let admission = slots(4);
    let ingress = slots(4);
    let older = source(&first, &admission, &ingress, KeyboardKey::A);
    let later = submit(&second, source(&second, &admission, &ingress, KeyboardKey::B));
    thread::sleep(Duration::from_millis(25));
    assert!(trace.lock().unwrap().is_empty());
    let earlier = submit(&first, older);
    until(|| earlier.complete() && later.complete());
    assert_eq!(
        *trace.lock().unwrap(),
        vec![
            (KeyboardKey::A, true),
            (KeyboardKey::A, false),
            (KeyboardKey::B, true),
            (KeyboardKey::B, false)
        ]
    );
    drop((first, second));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn cancelling_a_waiting_module_reports_not_sent_before_the_active_peer_returns() {
    let coordinator = Coordinator::new();
    let native = slots(2);
    let trace = Trace::default();
    let (release, pause) = mpsc::channel();
    let mut blocked = effects(&trace);
    blocked.pause = Some(pause);
    let (first, _) = spawn_shared(&coordinator, &native, blocked);
    let (second, owner) = spawn_shared(&coordinator, &native, effects(&trace));
    let admission = slots(4);
    let ingress = slots(4);
    let active = submit(&first, source(&first, &admission, &ingress, KeyboardKey::A));
    until(|| trace.lock().unwrap().len() == 1);
    let waiting = submit(&second, source(&second, &admission, &ingress, KeyboardKey::B));
    drop(second);
    until(|| waiting.complete());
    assert_eq!(waiting.take(), Some(Outcome::NotSent));
    assert!(!waiting.attempted());
    assert!(!active.complete());
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
    release.send(()).unwrap();
    until(|| active.complete());
    assert_eq!(
        *trace.lock().unwrap(),
        vec![(KeyboardKey::A, true), (KeyboardKey::A, false)]
    );
    drop(first);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn panic_retires_its_suffix_and_unblocks_a_peer_before_blocked_provider_cleanup_finishes() {
    let coordinator = Coordinator::new();
    let native = slots(2);
    let trace = Trace::default();
    let (release, pause) = mpsc::channel();
    let (retire, cleanup) = mpsc::channel();
    let mut broken = effects(&trace);
    broken.pause = Some(pause);
    broken.cleanup = Some(cleanup);
    broken.panic = true;
    let (first, owner) = spawn_shared(&coordinator, &native, broken);
    let (second, _) = spawn_shared(&coordinator, &native, effects(&trace));
    let admission = slots(4);
    let ingress = slots(4);
    let active = submit(&first, source(&first, &admission, &ingress, KeyboardKey::A));
    until(|| trace.lock().unwrap().len() == 1);
    let suffix = submit(&first, source(&first, &admission, &ingress, KeyboardKey::B));
    let peer = submit(&second, source(&second, &admission, &ingress, KeyboardKey::C));
    release.send(()).unwrap();
    until(|| active.complete() && suffix.complete() && peer.complete() && !first.is_running());
    assert_eq!(active.take(), Some(Outcome::Unconfirmed));
    assert_eq!(suffix.take(), Some(Outcome::NotSent));
    assert!(matches!(peer.take(), Some(Outcome::Completed(Ok(_)))));
    assert_eq!(owner.metrics().active, 1);
    assert_eq!(
        *trace.lock().unwrap(),
        vec![(KeyboardKey::A, true), (KeyboardKey::C, true), (KeyboardKey::C, false)]
    );
    drop((first, second));
    retire.send(()).unwrap();
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
