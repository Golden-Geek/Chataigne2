use super::*;
use std::sync::{atomic::AtomicUsize, mpsc};

type Gate = (mpsc::SyncSender<()>, mpsc::Receiver<()>);
mod engine;

#[test]
fn queued_input_projection_retains_its_source_until_retirement_rejects_the_entire_prefix() {
    use crate::app::{AppEngine, AppNode, ArtNetModule};
    use golden_core::{
        engine::EngineEditError,
        node::{Folder, Node},
        parameter::ParamValue,
        process_ctx::{ExecutionPhase, ProcessCtx},
    };
    let mut engine = AppEngine::new(Folder::new("root").into());
    engine.add_node(ArtNetModule::create().into(), None);
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let lifetimes = slots();
    let control = Control::default();
    let input = pool(1);
    let mut handle = fake(control.clone(), &lifetimes, pool(8), input.clone());
    control
        .frames
        .lock()
        .unwrap()
        .push_back(DmxFrame::new(1, vec![42]).unwrap());
    until(|| handle.has_frame());
    let frame = handle.take_frame().unwrap();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    ctx.set_tree_snapshot(engine.process_tree_snapshot());
    let AppNode::ArtNetModule(module) = engine.nodes.get_mut(id).unwrap() else {
        unreachable!()
    };
    let target = module.base.received_frame.id();
    module.base.apply_received_frame(&mut ctx, frame, 1);
    assert!(!ctx.edits.pending.is_empty());
    engine.absorb_edits(&mut ctx).unwrap();
    handle.stop();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(
        input.try_reserve(1).is_err(),
        "queued edits retain the original input quota"
    );
    assert!(matches!(
        engine.apply_edits(),
        Err(EngineEditError::EditPreconditionFailed { .. })
    ));
    assert_eq!(
        engine.nodes.get(target).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Str("[]".into())
    );
    assert!(input.wait_for_idle(Duration::from_secs(1)));
}
#[derive(Clone, Default)]
struct Control {
    sent: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    fail: Arc<AtomicBool>,
    panic: Arc<AtomicBool>,
    continuous: Arc<AtomicBool>,
    ignored: Arc<AtomicUsize>,
    invalid: Arc<AtomicBool>,
    paced: Arc<AtomicBool>,
    command_at: Arc<Mutex<Option<Instant>>>,
    sent_values: Arc<Mutex<Vec<u8>>>,
    frames: Arc<Mutex<std::collections::VecDeque<DmxFrame>>>,
}
struct Fake {
    control: Control,
    send_gate: Option<Gate>,
    cleanup: Option<Gate>,
}
impl worker::Endpoint for Fake {
    fn send(&mut self, frame: &DmxFrame) -> Result<(), String> {
        assert!(!self.control.panic.load(Ordering::Acquire), "injected DMX send panic");
        if let Some((entered, gate)) = self.send_gate.take() {
            let _ = entered.send(());
            let _ = gate.recv();
        }
        self.control.sent.fetch_add(1, Ordering::AcqRel);
        self.control.sent_values.lock().unwrap().push(frame.slots[0]);
        if self.control.paced.load(Ordering::Acquire) {
            *self.control.command_at.lock().unwrap() = Some(Instant::now() + Duration::from_millis(30));
        }
        if self.control.fail.load(Ordering::Acquire) {
            return Err("injected DMX send failure".into());
        }
        Ok(())
    }
    fn receive(&mut self) -> Result<ReceivePoll, String> {
        assert!(
            !self.control.panic.load(Ordering::Acquire),
            "injected DMX worker failure"
        );
        let sequence = self.control.reads.fetch_add(1, Ordering::AcqRel);
        if self
            .control
            .ignored
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| count.checked_sub(1))
            .is_ok()
        {
            return Ok(if self.control.invalid.load(Ordering::Acquire) {
                ReceivePoll::Invalid("injected malformed DMX packet")
            } else {
                ReceivePoll::Ignored
            });
        }
        let frame = self.control.frames.lock().unwrap().pop_front();
        if frame.is_none() && self.control.continuous.load(Ordering::Acquire) {
            Ok(ReceivePoll::Frame(DmxFrame::new(1, vec![(sequence % 256) as u8; 512])?))
        } else {
            Ok(frame.map_or(ReceivePoll::Empty, ReceivePoll::Frame))
        }
    }
    fn command_at(&self) -> Option<Instant> {
        *self.control.command_at.lock().unwrap()
    }
    fn shutdown(&mut self) {
        if let Some((entered, gate)) = self.cleanup.take() {
            let _ = entered.send(());
            let _ = gate.recv();
        }
    }
}
fn config() -> DmxTransportConfig {
    DmxTransportConfig {
        protocol: DmxProtocol::ArtNet,
        source_id: [1; 16],
        bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        listen_port: 1,
        receive_enabled: true,
        universe: 1,
        destination: Some("127.0.0.1:1".parse().unwrap()),
    }
}
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}

#[test]
fn paced_output_keeps_fifo_receipts_while_continuous_input_makes_progress() {
    let lifetimes = slots();
    let control = Control::default();
    control.paced.store(true, Ordering::Release);
    control.continuous.store(true, Ordering::Release);
    let mut handle = fake(control.clone(), &lifetimes, pool(32), pool(32));
    let started = Instant::now();
    let receipts: Vec<_> = (0..10)
        .map(|value| handle.send(DmxFrame::new(1, vec![value]).unwrap()).unwrap())
        .collect();
    until(|| receipts.iter().all(|receipt| receipt.status() == SendStatus::Sent));
    assert!(started.elapsed() >= Duration::from_millis(270));
    assert_eq!(*control.sent_values.lock().unwrap(), (0..10).collect::<Vec<_>>());
    assert!(control.reads.load(Ordering::Acquire) > 10);
    assert!(handle.has_frame());
    handle.stop();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn a_clock_wait_does_not_acknowledge_or_lose_retained_commands_and_is_cancellable() {
    let lifetimes = slots();
    let control = Control::default();
    *control.command_at.lock().unwrap() = Some(Instant::now() + Duration::from_secs(60));
    control.continuous.store(true, Ordering::Release);
    let output = pool(8);
    let mut handle = fake(control.clone(), &lifetimes, output.clone(), pool(32));
    let receipts: Vec<_> = (0..8)
        .map(|value| handle.send(DmxFrame::new(1, vec![value]).unwrap()).unwrap())
        .collect();
    until(|| control.reads.load(Ordering::Acquire) >= 64);
    assert!(receipts.iter().all(|receipt| receipt.status() == SendStatus::Pending));
    assert_eq!(control.sent.load(Ordering::Acquire), 0);
    assert!(
        output.try_reserve(1).is_err(),
        "clock waiting must keep original payload admission"
    );
    let stopped = Instant::now();
    handle.stop();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(stopped.elapsed() < Duration::from_secs(1));
    assert!(
        receipts
            .iter()
            .all(|receipt| receipt.status() == SendStatus::FailedUnconfirmed)
    );
    assert!(output.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn ignored_packet_bursts_drain_without_a_wait_per_packet_and_preserve_output() {
    let lifetimes = slots();
    let control = Control::default();
    control.ignored.store(8192, Ordering::Release);
    control
        .frames
        .lock()
        .unwrap()
        .push_back(DmxFrame::new(1, vec![42]).unwrap());
    let started = Instant::now();
    let mut handle = fake(control.clone(), &lifetimes, pool(8), pool(8));
    let receipt = handle.send(DmxFrame::new(1, vec![19]).unwrap()).unwrap();
    until(|| handle.has_frame() && receipt.status() == SendStatus::Sent);
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "discarded packets must not each incur a five-millisecond wait"
    );
    assert!(control.reads.load(Ordering::Acquire) >= 8193);
    assert_eq!(handle.take_frame().unwrap().slots, [42]);
    assert_eq!(control.sent.load(Ordering::Acquire), 1);
    handle.stop();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn malformed_packet_flood_warns_once_without_losing_valid_input_or_output() {
    let lifetimes = slots();
    let control = Control::default();
    control.invalid.store(true, Ordering::Release);
    control.ignored.store(8192, Ordering::Release);
    control
        .frames
        .lock()
        .unwrap()
        .push_back(DmxFrame::new(1, vec![42]).unwrap());
    let mut handle = fake(control.clone(), &lifetimes, pool(8), pool(8));
    let receipt = handle.send(DmxFrame::new(1, vec![19]).unwrap()).unwrap();
    until(|| handle.has_frame() && receipt.status() == SendStatus::Sent);
    let mut warnings = 0;
    while let Some(status) = handle.take_status() {
        match status {
            DmxWorkerEvent::Warning(message) => {
                assert_eq!(message, "injected malformed DMX packet");
                warnings += 1;
            }
            DmxWorkerEvent::Ready => {}
            other => panic!("unexpected DMX status: {other:?}"),
        }
    }
    assert_eq!(warnings, 1);
    assert!(control.reads.load(Ordering::Acquire) >= 8193);
    assert_eq!(handle.take_frame().unwrap().slots, [42]);
    assert_eq!(control.sent.load(Ordering::Acquire), 1);
    assert!(handle.is_running());
    handle.stop();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}
fn pool(items: usize) -> PayloadPool {
    PayloadPool::new("dmx-test-payloads", items, 8 * 1024 * 1024).unwrap()
}
fn fake(control: Control, lifetimes: &RetirementPool, output: PayloadPool, input: PayloadPool) -> DmxTransportHandle {
    let handle = DmxTransportHandle::spawn_using(
        config(),
        WorkerNotifier::new(|| {}),
        lifetimes,
        output,
        input,
        move |_| {
            Ok(Fake {
                control,
                send_gate: None,
                cleanup: None,
            })
        },
    )
    .unwrap();
    until(|| handle.is_ready());
    handle.take_status();
    handle
}

#[test]
fn blocked_setup_is_asynchronous_holds_its_slot_and_cancellation_prevents_late_io() {
    let lifetimes = slots();
    let control = Control::default();
    let observed = control.clone();
    let (entered, opening) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let mut handle = DmxTransportHandle::spawn_using(
        config(),
        WorkerNotifier::new(|| {}),
        &lifetimes,
        pool(8),
        pool(8),
        move |_| {
            entered.send(()).unwrap();
            gate.recv().unwrap();
            Ok(Fake {
                control,
                send_gate: None,
                cleanup: None,
            })
        },
    )
    .unwrap();
    opening.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(!handle.is_ready());
    assert!(handle.send(DmxFrame::new(1, vec![1]).unwrap()).is_err());
    handle.stop();
    for _ in 0..3 {
        assert!(
            DmxTransportHandle::spawn_using(
                config(),
                WorkerNotifier::new(|| {}),
                &lifetimes,
                pool(8),
                pool(8),
                |_| -> Result<Fake, String> { panic!("slot released before setup returned") }
            )
            .is_err()
        );
    }
    release.send(()).unwrap();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert_eq!(observed.sent.load(Ordering::Acquire), 0);
    assert_eq!(observed.reads.load(Ordering::Acquire), 0);
}

#[test]
fn blocked_send_fills_a_bounded_queue_and_stop_cancels_every_admitted_suffix() {
    let lifetimes = slots();
    let control = Control::default();
    let (entered, sending) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let output = pool(128);
    let mut handle = DmxTransportHandle::spawn_using(
        config(),
        WorkerNotifier::new(|| {}),
        &lifetimes,
        output.clone(),
        pool(8),
        move |_| {
            Ok(Fake {
                control,
                send_gate: Some((entered, gate)),
                cleanup: None,
            })
        },
    )
    .unwrap();
    until(|| handle.is_ready());
    let first = handle.send(DmxFrame::new(1, vec![1]).unwrap()).unwrap();
    sending.recv_timeout(Duration::from_secs(1)).unwrap();
    let mut receipts = Vec::new();
    for _ in 0..WORKER_COMMAND_CAPACITY {
        receipts.push(handle.send(DmxFrame::new(1, vec![2]).unwrap()).unwrap());
    }
    assert!(handle.send(DmxFrame::new(1, vec![3]).unwrap()).is_err());
    handle.stop();
    release.send(()).unwrap();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert_eq!(first.status(), SendStatus::Sent);
    assert!(
        receipts
            .iter()
            .all(|receipt| receipt.status() == SendStatus::FailedUnconfirmed)
    );
    assert!(output.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn native_cleanup_retains_the_slot_and_stop_does_not_join() {
    let lifetimes = slots();
    let (entered, cleaning) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let mut handle = DmxTransportHandle::spawn_using(
        config(),
        WorkerNotifier::new(|| {}),
        &lifetimes,
        pool(8),
        pool(8),
        move |_| {
            Ok(Fake {
                control: Control::default(),
                send_gate: None,
                cleanup: Some((entered, gate)),
            })
        },
    )
    .unwrap();
    until(|| handle.is_ready());
    handle.stop();
    cleaning.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(!handle.is_running());
    assert_eq!(lifetimes.metrics().active, 1);
    assert!(
        DmxTransportHandle::spawn_using(
            config(),
            WorkerNotifier::new(|| {}),
            &lifetimes,
            pool(8),
            pool(8),
            |_| -> Result<Fake, String> { panic!("cleanup released its slot") }
        )
        .is_err()
    );
    release.send(()).unwrap();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    let mut recovered = fake(Control::default(), &lifetimes, pool(8), pool(8));
    recovered.stop();
    drop(recovered);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn retained_input_keeps_its_quota_and_source_retirement_is_independent_of_consumption() {
    let lifetimes = slots();
    let control = Control::default();
    let input = pool(1);
    let mut handle = fake(control.clone(), &lifetimes, pool(8), input.clone());
    control
        .frames
        .lock()
        .unwrap()
        .push_back(DmxFrame::new(1, vec![7]).unwrap());
    until(|| handle.has_frame());
    let frame = handle.take_frame().unwrap();
    assert!(input.try_reserve(1).is_err());
    assert!(frame.source.is_current());
    handle.stop();
    assert!(!frame.source.is_current());
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(!input.wait_for_idle(Duration::from_millis(5)));
    drop(frame);
    assert!(input.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn errors_survive_latest_frame_pressure_and_failed_sends_have_explicit_receipts() {
    let lifetimes = slots();
    let control = Control::default();
    let mut handle = fake(control.clone(), &lifetimes, pool(8), pool(8));
    control
        .frames
        .lock()
        .unwrap()
        .extend([DmxFrame::new(1, vec![1]).unwrap(), DmxFrame::new(1, vec![2]).unwrap()]);
    until(|| handle.take_replaced_frames() > 0);
    control.fail.store(true, Ordering::Release);
    let receipt = handle.send(DmxFrame::new(1, vec![9]).unwrap()).unwrap();
    until(|| receipt.status() == SendStatus::FailedUnconfirmed);
    assert!(receipt.failure().unwrap().contains("injected"));
    until(|| !handle.is_running());
    assert!(matches!(handle.take_status(), Some(DmxWorkerEvent::Error(_))));
    assert!(handle.take_frame().is_some());
    handle.stop();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn payload_capacity_and_other_universes_are_rejected_before_native_dispatch() {
    let lifetimes = slots();
    let output = pool(1);
    let control = Control::default();
    let handle = fake(control.clone(), &lifetimes, output.clone(), pool(8));
    let held = output.try_reserve(1).unwrap();
    assert!(handle.send(DmxFrame::new(1, vec![1]).unwrap()).is_err());
    drop(held);
    assert!(handle.send(DmxFrame::new(2, vec![1]).unwrap()).is_err());
    let frame = DmxFrame {
        universe: 1,
        sequence: 0,
        priority: 100,
        slots: Vec::with_capacity(513),
    };
    assert!(handle.send(frame).is_err());
    assert_eq!(control.sent.load(Ordering::Acquire), 0);
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}
