use super::*;
use std::sync::{atomic::AtomicUsize, mpsc};

type Gate = (mpsc::SyncSender<()>, mpsc::Receiver<()>);

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
    frames: Arc<Mutex<std::collections::VecDeque<DmxFrame>>>,
}
struct Fake {
    control: Control,
    send_gate: Option<Gate>,
    cleanup: Option<Gate>,
}
impl worker::Endpoint for Fake {
    fn send(&mut self, _: &DmxFrame, _: Option<SocketAddr>) -> Result<(), String> {
        if let Some((entered, gate)) = self.send_gate.take() {
            let _ = entered.send(());
            let _ = gate.recv();
        }
        self.control.sent.fetch_add(1, Ordering::AcqRel);
        if self.control.fail.load(Ordering::Acquire) {
            return Err("injected DMX send failure".into());
        }
        Ok(())
    }
    fn receive(&mut self) -> Result<Option<DmxFrame>, String> {
        assert!(
            !self.control.panic.load(Ordering::Acquire),
            "injected DMX worker failure"
        );
        self.control.reads.fetch_add(1, Ordering::AcqRel);
        Ok(self.control.frames.lock().unwrap().pop_front())
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
        assert!(DmxTransportHandle::spawn_using(
            config(),
            WorkerNotifier::new(|| {}),
            &lifetimes,
            pool(8),
            pool(8),
            |_| -> Result<Fake, String> { panic!("slot released before setup returned") }
        )
        .is_err());
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
    assert!(receipts
        .iter()
        .all(|receipt| receipt.status() == SendStatus::FailedUnconfirmed));
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
    assert!(DmxTransportHandle::spawn_using(
        config(),
        WorkerNotifier::new(|| {}),
        &lifetimes,
        pool(8),
        pool(8),
        |_| -> Result<Fake, String> { panic!("cleanup released its slot") }
    )
    .is_err());
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
