use super::super::midi_message::MidiMessage;
use super::input::{InputConnection, InputSink};
use super::output::OutputConnection;
use super::*;
mod outgoing;
mod passive;
use std::sync::{mpsc, Condvar, Mutex};

#[derive(Clone, Default)]
struct Gate(Arc<(Mutex<bool>, Condvar)>);
impl Gate {
    fn wait(&self) {
        let (lock, changed) = &*self.0;
        drop(changed.wait_while(lock.lock().unwrap(), |released| !*released).unwrap());
    }
    fn release(&self) {
        let (lock, changed) = &*self.0;
        *lock.lock().unwrap() = true;
        changed.notify_all();
    }
}
struct Release(Gate);
impl Drop for Release {
    fn drop(&mut self) {
        self.0.release();
    }
}
fn pool() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn notify() -> WorkerNotifier {
    WorkerNotifier::new(|| {})
}
fn output_config() -> MidiOutputConfig {
    MidiOutputConfig {
        port_variant: "0|fake".into(),
    }
}
fn input_config() -> MidiInputConfig {
    MidiInputConfig {
        port_variant: "0|fake".into(),
    }
}
fn until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !condition() {
        assert!(Instant::now() < deadline, "fake runtime did not settle");
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[derive(Default)]
struct FakeOutput {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    send_gate: Option<(mpsc::Sender<()>, Gate)>,
    drop_gate: Option<(mpsc::Sender<()>, Gate)>,
    fail: bool,
}
impl OutputConnection for FakeOutput {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        if let Some((started, gate)) = self.send_gate.take() {
            started.send(()).unwrap();
            gate.wait();
        }
        self.sent.lock().unwrap().push(bytes.to_vec());
        if self.fail {
            Err("fake native failure".into())
        } else {
            Ok(())
        }
    }
    fn is_present(&mut self) -> Result<bool, String> {
        Ok(true)
    }
}
impl Drop for FakeOutput {
    fn drop(&mut self) {
        assert_eq!(std::thread::current().name(), Some("midi-output"));
        if let Some((started, gate)) = self.drop_gate.take() {
            let _ = started.send(());
            gate.wait();
        }
    }
}
struct FakeInput {
    _sink: InputSink,
    present: Arc<AtomicBool>,
}
impl InputConnection for FakeInput {
    fn is_present(&mut self) -> Result<bool, String> {
        Ok(self.present.load(Ordering::Acquire))
    }
}
impl Drop for FakeInput {
    fn drop(&mut self) {
        assert_eq!(std::thread::current().name(), Some("midi-input"));
    }
}

#[test]
fn blocked_output_setup_is_nonblocking_and_holds_lifetime_until_native_cleanup() {
    let pool = pool();
    let gate = Gate::default();
    let _release = Release(gate.clone());
    let (started, seen) = mpsc::channel();
    let blocked = gate.clone();
    let handle = MidiOutputHandle::spawn_using(output_config(), &pool, notify(), move |_| {
        started.send(()).unwrap();
        blocked.wait();
        Ok(FakeOutput::default())
    })
    .unwrap();
    seen.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(!handle.is_ready());
    let start = Instant::now();
    drop(handle);
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(pool.metrics().active, 1);
    assert!(MidiOutputHandle::spawn_using(output_config(), &pool, notify(), |_| Ok(FakeOutput::default())).is_err());
    gate.release();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let recovered =
        MidiOutputHandle::spawn_using(output_config(), &pool, notify(), |_| Ok(FakeOutput::default())).unwrap();
    until(|| recovered.is_ready());
    drop(recovered);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn blocked_output_send_bounds_items_and_preserves_order_through_graceful_retirement() {
    let pool = pool();
    let gate = Gate::default();
    let _release = Release(gate.clone());
    let (started, seen) = mpsc::channel();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let copy = sent.clone();
    let blocked = gate.clone();
    let mut handle = MidiOutputHandle::spawn_using(output_config(), &pool, notify(), move |_| {
        Ok(FakeOutput {
            sent: copy,
            send_gate: Some((started, blocked)),
            drop_gate: None,
            fail: false,
        })
    })
    .unwrap();
    until(|| handle.is_ready());
    handle.send(&[0x90, 0, 1], "first", true).unwrap();
    seen.recv_timeout(Duration::from_secs(2)).unwrap();
    for n in 0..256 {
        handle.send(&[0x90, (n % 128) as u8, 2], "queued", false).unwrap();
    }
    assert!(handle
        .send(&[0xF8], "overflow", false)
        .unwrap_err()
        .contains("capacity"));
    handle.finish_admission();
    assert!(handle.send(&[0xF8], "closed", false).is_err());
    gate.release();
    let mut events = Vec::new();
    until(|| handle.drain(&mut events).state == PendingDrainState::Disconnected);
    assert_eq!(
        events.iter().filter(|e| matches!(e, MidiOutputEvent::Sent(_))).count(),
        257
    );
    let sent = sent.lock().unwrap();
    assert_eq!(sent.len(), 257);
    assert_eq!(sent[0], [0x90, 0, 1]);
    for (n, bytes) in sent[1..].iter().enumerate() {
        assert_eq!(bytes, &[0x90, (n % 128) as u8, 2]);
    }
    assert!(events
        .iter()
        .any(|e| matches!(e,MidiOutputEvent::Sent(Some(log)) if log.contains("first"))));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn output_retained_byte_admission_is_independent_of_item_limit() {
    let pool = pool();
    let gate = Gate::default();
    let _release = Release(gate.clone());
    let (started, seen) = mpsc::channel();
    let blocked = gate.clone();
    let handle = MidiOutputHandle::spawn_using(output_config(), &pool, notify(), move |_| {
        Ok(FakeOutput {
            sent: Arc::new(Mutex::new(Vec::new())),
            send_gate: Some((started, blocked)),
            drop_gate: None,
            fail: false,
        })
    })
    .unwrap();
    until(|| handle.is_ready());
    handle.send(&[0xF8], "block", false).unwrap();
    seen.recv_timeout(Duration::from_secs(2)).unwrap();
    let bytes = vec![0; MESSAGE_BYTES];
    let mut admitted = 0;
    while handle.send(&bytes, "large", false).is_ok() {
        admitted += 1;
        assert!(admitted < 256);
    }
    assert!((1..32).contains(&admitted));
    assert!(handle.send(&vec![0; MESSAGE_BYTES + 1], "oversized", false).is_err());
    let start = Instant::now();
    drop(handle);
    assert!(start.elapsed() < Duration::from_millis(100));
    gate.release();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn failed_send_has_one_unconfirmed_effect_and_never_replays_accepted_suffix() {
    let pool = pool();
    let gate = Gate::default();
    let _release = Release(gate.clone());
    let (started, seen) = mpsc::channel();
    let blocked = gate.clone();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let copy = sent.clone();
    let handle = MidiOutputHandle::spawn_using(output_config(), &pool, notify(), move |_| {
        Ok(FakeOutput {
            sent: copy,
            send_gate: Some((started, blocked)),
            fail: true,
            drop_gate: None,
        })
    })
    .unwrap();
    until(|| handle.is_ready());
    handle.send(&[1], "first", false).unwrap();
    seen.recv_timeout(Duration::from_secs(2)).unwrap();
    for n in 2..=5 {
        handle.send(&[n], "suffix", false).unwrap();
    }
    gate.release();
    let mut events = Vec::new();
    until(|| handle.drain(&mut events).state == PendingDrainState::Disconnected);
    assert!(!handle.is_ready());
    assert_eq!(&*sent.lock().unwrap(), &[vec![1]]);
    assert_eq!(
        events.iter().filter(|e| matches!(e, MidiOutputEvent::Error(_))).count(),
        5
    );
    assert!(events
        .iter()
        .any(|e| matches!(e,MidiOutputEvent::Error(error) if error.contains("unconfirmed"))));
    assert!(handle.send(&[6], "later", false).is_err());
}

#[test]
fn blocked_native_output_destructor_retains_capacity_without_actor_join() {
    let pool = pool();
    let gate = Gate::default();
    let _release = Release(gate.clone());
    let (started, seen) = mpsc::channel();
    let blocked = gate.clone();
    let handle = MidiOutputHandle::spawn_using(output_config(), &pool, notify(), move |_| {
        Ok(FakeOutput {
            sent: Arc::new(Mutex::new(Vec::new())),
            send_gate: None,
            drop_gate: Some((started, blocked)),
            fail: false,
        })
    })
    .unwrap();
    until(|| handle.is_ready());
    let start = Instant::now();
    drop(handle);
    assert!(start.elapsed() < Duration::from_millis(100));
    seen.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(pool.metrics().active, 1);
    assert!(pool.try_reserve().is_err());
    gate.release();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn callback_admission_counts_full_and_oversize_rejections_and_retains_ordered_prefix() {
    let pool = pool();
    let (sender, receiver) = mpsc::channel();
    let handle = MidiInputHandle::spawn_using(input_config(), &pool, notify(), move |_, sink| {
        // Fill before setup completes, so the runtime cannot race the capacity assertion.
        for n in 0..256 {
            assert!(sink.push(&[0x90, (n % 128) as u8, 7]));
        }
        assert!(!sink.push(&[0xF8]));
        assert!(!sink.push(&vec![0; MESSAGE_BYTES + 1]));
        sender.send(()).unwrap();
        Ok(FakeInput {
            _sink: sink,
            present: Arc::new(AtomicBool::new(true)),
        })
    })
    .unwrap();
    receiver.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut notes = Vec::new();
    let mut rejected = 0;
    until(|| {
        while let Ok(event) = handle.try_recv() {
            match event {
                MidiInputEvent::Message(input) => {
                    if let MidiMessage::NoteOn { note, .. } = input.message {
                        notes.push(note);
                    }
                }
                MidiInputEvent::Rejected(n) => rejected += n,
                _ => {}
            }
        }
        notes.len() == 256 && rejected == 2
    });
    assert_eq!(notes, (0..256).map(|n| (n % 128) as u8).collect::<Vec<_>>());
    drop(handle);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn blocked_input_setup_cancels_without_waiting_and_releases_lifetime_after_return() {
    let pool = pool();
    let gate = Gate::default();
    let _release = Release(gate.clone());
    let (started, seen) = mpsc::channel();
    let blocked = gate.clone();
    let handle = MidiInputHandle::spawn_using(input_config(), &pool, notify(), move |_, sink| {
        started.send(()).unwrap();
        blocked.wait();
        Ok(FakeInput {
            _sink: sink,
            present: Arc::new(AtomicBool::new(true)),
        })
    })
    .unwrap();
    seen.recv_timeout(Duration::from_secs(2)).unwrap();
    let start = Instant::now();
    drop(handle);
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(pool.metrics().active, 1);
    gate.release();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn input_disappearance_clears_readiness_and_publishes_accepted_prefix_before_exit() {
    let pool = pool();
    let present = Arc::new(AtomicBool::new(true));
    let copy = present.clone();
    let handle = MidiInputHandle::spawn_using(input_config(), &pool, notify(), move |_, sink| {
        assert!(sink.push(&[0x90, 60, 100]));
        Ok(FakeInput {
            _sink: sink,
            present: copy,
        })
    })
    .unwrap();
    until(|| handle.is_ready());
    present.store(false, Ordering::Release);
    let mut events = Vec::new();
    until(|| match handle.try_recv() {
        Ok(e) => {
            events.push(e);
            false
        }
        Err(std::sync::mpsc::TryRecvError::Disconnected) => true,
        _ => false,
    });
    assert!(!handle.is_ready());
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, MidiInputEvent::Message(_)))
            .count(),
        1
    );
    assert!(events
        .iter()
        .any(|e| matches!(e,MidiInputEvent::Error(error) if error.contains("disappeared"))));
}

#[test]
fn clock_analysis_preserves_24_tick_beat_occurrences_and_timestamp_bpm() {
    let mut analyzer = clock::MidiClockAnalyzer::default();
    let mut beats = 0;
    let mut bpm = None;
    for n in 1..=120 {
        let timing = analyzer.observe(&[0xF8], Duration::from_micros(n * 20_833)).unwrap();
        beats += usize::from(timing.beat_triggered);
        bpm = timing.bpm.or(bpm);
    }
    assert_eq!(beats, 5);
    assert!((bpm.unwrap() - 120.0).abs() < 0.01);
    assert!(analyzer.observe(&[0x90, 60, 100], Duration::ZERO).is_none());
}

#[test]
fn discovery_recovers_from_failure_without_an_actor_native_call() {
    let pool = pool();
    let mut attempts = 0;
    let discovery = MidiDiscovery::spawn_using(&pool, notify(), "none", "none", move || {
        attempts += 1;
        if attempts == 1 {
            Err("fake query failure".into())
        } else {
            Ok(MidiPortOptions {
                inputs: Vec::new(),
                outputs: Vec::new(),
            })
        }
    })
    .unwrap();
    let mut failure = false;
    let mut recovered = false;
    until(|| {
        if let Some(event) = discovery.take().0 {
            match event.result {
                Err(_) => failure = true,
                Ok(_) => recovered = true,
            }
        }
        failure && recovered
    });
    drop(discovery);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn blocked_discovery_is_bounded_and_cancellation_never_joins_native_query() {
    let pool = pool();
    let gate = Gate::default();
    let _release = Release(gate.clone());
    let (started, seen) = mpsc::channel();
    let blocked = gate.clone();
    let discovery = MidiDiscovery::spawn_using(&pool, notify(), "none", "none", move || {
        started.send(()).unwrap();
        blocked.wait();
        Ok(MidiPortOptions {
            inputs: Vec::new(),
            outputs: Vec::new(),
        })
    })
    .unwrap();
    seen.recv_timeout(Duration::from_secs(2)).unwrap();
    let start = Instant::now();
    drop(discovery);
    assert!(start.elapsed() < Duration::from_millis(100));
    assert!(pool.try_reserve().is_err());
    gate.release();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
