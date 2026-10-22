use super::super::*;
use std::{
    sync::{atomic::AtomicUsize, mpsc},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Default)]
struct Control {
    actions: Arc<AtomicUsize>,
    polls: Arc<AtomicUsize>,
    connects: Arc<AtomicUsize>,
    fail_connects: Arc<AtomicUsize>,
    fail_poll: Arc<AtomicBool>,
    panic_action: Arc<AtomicBool>,
    event_count: usize,
}
struct Fake {
    control: Control,
    action: Option<mpsc::Receiver<()>>,
    cleanup: Option<mpsc::Receiver<()>>,
}
impl worker::Provider for Fake {
    fn connect(&mut self, _: &ButtplugTransportConfig) -> Result<Vec<ButtplugWorkerEvent>, String> {
        let n = self.control.connects.fetch_add(1, Ordering::AcqRel);
        if n < self.control.fail_connects.load(Ordering::Acquire) {
            return Err("controlled connection failure".into());
        }
        Ok(vec![ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Connected {
            server_name: "fixture".into(),
        })])
    }
    fn poll(&mut self) -> Result<Vec<ButtplugWorkerEvent>, String> {
        thread::sleep(Duration::from_millis(1));
        let n = self.control.polls.fetch_add(1, Ordering::AcqRel);
        if self.control.fail_poll.swap(false, Ordering::AcqRel) {
            return Err("controlled lost connection".into());
        }
        if n < self.control.event_count {
            Ok(vec![ButtplugWorkerEvent::DeviceAdded(ButtplugDeviceInfo {
                index: n as u32,
                variant_id: format!("device:{n}"),
                name: "fixture".into(),
                display_name: None,
                outputs: Vec::new(),
            })])
        } else {
            Ok(Vec::new())
        }
    }
    fn execute(&mut self, _: &ButtplugWorkerCommand) -> Result<String, String> {
        self.control.actions.fetch_add(1, Ordering::AcqRel);
        if let Some(gate) = self.action.take() {
            gate.recv().unwrap();
        }
        assert!(
            !self.control.panic_action.load(Ordering::Acquire),
            "controlled native panic"
        );
        Ok("completed fixture action".into())
    }
    fn disconnect(&mut self) {}
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some(gate) = self.cleanup.take() {
            let _ = gate.recv();
        }
    }
}
fn config() -> ButtplugTransportConfig {
    ButtplugTransportConfig {
        remote_host: "fixture".into(),
        remote_port: 1,
        path: "/".into(),
        secure: false,
        bypass_certificate_verification: false,
        client_name: "fixture".into(),
        auto_scan: false,
    }
}
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn pool(items: usize) -> PayloadPool {
    PayloadPool::new("buttplug-fixture-disposer", items, 64 * 1024 * 1024).unwrap()
}
fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !predicate() {
        assert!(Instant::now() < end, "fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn spawn(
    control: Control,
    slots: &RetirementPool,
    action: Option<mpsc::Receiver<()>>,
    cleanup: Option<mpsc::Receiver<()>>,
) -> ButtplugTransportHandle {
    ButtplugTransportHandle::spawn_using(
        config(),
        WorkerNotifier::new(|| {}),
        slots,
        pool(256),
        pool(128),
        move || {
            Ok(Fake {
                control,
                action,
                cleanup,
            })
        },
    )
    .unwrap()
}
fn connected(handle: &ButtplugTransportHandle) {
    until(|| handle.shared.connected.load(Ordering::Acquire));
}

#[test]
fn blocked_open_cancellation_retains_one_slot_without_joining_or_starting_replacements() {
    let slots = slots();
    let opened = Arc::new(AtomicBool::new(false));
    let entered = opened.clone();
    let (release, gate) = mpsc::channel();
    let handle = ButtplugTransportHandle::spawn_using(
        config(),
        WorkerNotifier::new(|| {}),
        &slots,
        pool(8),
        pool(8),
        move || {
            entered.store(true, Ordering::Release);
            gate.recv().unwrap();
            Ok(Fake {
                control: Control::default(),
                action: None,
                cleanup: None,
            })
        },
    )
    .unwrap();
    until(|| opened.load(Ordering::Acquire));
    let started = Instant::now();
    drop(handle);
    assert!(started.elapsed() < Duration::from_millis(100));
    for _ in 0..8 {
        assert!(ButtplugTransportHandle::spawn_using(
            config(),
            WorkerNotifier::new(|| {}),
            &slots,
            pool(4),
            pool(4),
            || -> Result<Fake, String> { panic!("capacity must be reserved before native open") }
        )
        .is_err());
    }
    assert_eq!(slots.metrics().active, 1);
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    let handle = spawn(Control::default(), &slots, None, None);
    connected(&handle);
    drop(handle);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn cancelled_suffix_completes_before_blocked_cleanup_and_started_action_is_not_replayed() {
    let slots = slots();
    let control = Control::default();
    let (release, gate) = mpsc::channel();
    let (cleanup, cleanup_gate) = mpsc::channel();
    let mut handle = spawn(control.clone(), &slots, Some(gate), Some(cleanup_gate));
    connected(&handle);
    let first = handle.send(ButtplugWorkerCommand::StopAllDevices).unwrap();
    until(|| control.actions.load(Ordering::Acquire) == 1);
    let mut accepted = Vec::new();
    while let Ok(receipt) = handle.send(ButtplugWorkerCommand::StopAllDevices) {
        accepted.push(receipt);
    }
    assert_eq!(
        accepted.len(),
        64,
        "native mailbox refuses source 65 while one action is executing"
    );
    handle.stop();
    release.send(()).unwrap();
    until(|| !handle.is_running() && first.complete() && accepted.iter().all(CommandReceipt::complete));
    assert!(matches!(first.take(), Some(CommandStatus::Completed(Ok(_)))));
    assert!(accepted
        .iter()
        .all(|receipt| receipt.take() == Some(CommandStatus::NotSent)));
    assert_eq!(control.actions.load(Ordering::Acquire), 1);
    assert_eq!(slots.metrics().active, 1);
    cleanup.send(()).unwrap();
    drop(handle);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn panic_marks_started_action_unconfirmed_and_queued_actions_not_sent_before_cleanup() {
    let slots = slots();
    let control = Control::default();
    control.panic_action.store(true, Ordering::Release);
    let (release, gate) = mpsc::channel();
    let (cleanup, cleanup_gate) = mpsc::channel();
    let handle = spawn(control.clone(), &slots, Some(gate), Some(cleanup_gate));
    connected(&handle);
    let first = handle.send(ButtplugWorkerCommand::StopAllDevices).unwrap();
    until(|| control.actions.load(Ordering::Acquire) == 1);
    let suffix = handle.send(ButtplugWorkerCommand::StopAllDevices).unwrap();
    release.send(()).unwrap();
    until(|| !handle.is_running() && first.complete() && suffix.complete());
    assert_eq!(first.take(), Some(CommandStatus::Unconfirmed));
    assert_eq!(suffix.take(), Some(CommandStatus::NotSent));
    assert_eq!(slots.metrics().active, 1);
    cleanup.send(()).unwrap();
    drop(handle);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn full_event_storage_preserves_ordered_occurrences_and_cancellation_wakes_capacity_wait() {
    let slots = slots();
    let control = Control {
        event_count: 128,
        ..Control::default()
    };
    let handle = spawn(control.clone(), &slots, None, None);
    connected(&handle);
    until(|| handle.worker.event_metrics().depth == 16);
    let polls = control.polls.load(Ordering::Acquire);
    thread::sleep(Duration::from_millis(30));
    assert!(control.polls.load(Ordering::Acquire) <= polls + 1);
    let mut indices = Vec::new();
    until(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        for event in events {
            if let ButtplugWorkerEvent::DeviceAdded(device) = &*event {
                indices.push(device.index);
            }
        }
        indices.len() == 128
    });
    assert_eq!(indices, (0..128).collect::<Vec<_>>());
    drop(handle);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    let handle = spawn(
        Control {
            event_count: 128,
            ..Control::default()
        },
        &slots,
        None,
        None,
    );
    connected(&handle);
    until(|| handle.worker.event_metrics().depth == 16);
    drop(handle);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn retained_source_quota_survives_native_completion_until_receipt_disposal() {
    let slots = slots();
    let commands = pool(2);
    let handle = ButtplugTransportHandle::spawn_using(
        config(),
        WorkerNotifier::new(|| {}),
        &slots,
        commands.clone(),
        pool(8),
        || {
            Ok(Fake {
                control: Control::default(),
                action: None,
                cleanup: None,
            })
        },
    )
    .unwrap();
    connected(&handle);
    let receipt = handle.send(ButtplugWorkerCommand::StopAllDevices).unwrap();
    until(|| receipt.complete());
    assert!(handle.send(ButtplugWorkerCommand::StopAllDevices).is_err());
    drop(receipt);
    until(|| handle.send(ButtplugWorkerCommand::StopAllDevices).is_ok());
    drop(handle);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(commands.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn connection_failures_use_backoff_and_a_lost_session_recovers_without_replaying_commands() {
    let slots = slots();
    let control = Control::default();
    control.fail_connects.store(2, Ordering::Release);
    let handle = spawn(control.clone(), &slots, None, None);
    until(|| control.connects.load(Ordering::Acquire) == 1);
    thread::sleep(Duration::from_millis(100));
    assert_eq!(control.connects.load(Ordering::Acquire), 1);
    connected(&handle);
    assert_eq!(control.connects.load(Ordering::Acquire), 3);
    let receipt = handle.send(ButtplugWorkerCommand::StopAllDevices).unwrap();
    until(|| receipt.complete());
    control.fail_poll.store(true, Ordering::Release);
    until(|| !handle.shared.connected.load(Ordering::Acquire));
    connected(&handle);
    assert_eq!(control.connects.load(Ordering::Acquire), 4);
    assert_eq!(control.actions.load(Ordering::Acquire), 1);
    drop(handle);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn admission_refuses_large_retained_fields_and_catalogs_before_publication() {
    let mut config = config();
    config.path = "x".repeat(4097);
    assert!(admission::config(&config).is_err());
    assert!(admission::command(&ButtplugWorkerCommand::StopDevice {
        target: "x".repeat(1025)
    })
    .is_err());
    let device = ButtplugDeviceInfo {
        index: 0,
        variant_id: "device:0".into(),
        name: "fixture".into(),
        display_name: None,
        outputs: Vec::new(),
    };
    assert!(admission::event(&ButtplugWorkerEvent::Devices(vec![device.clone(); 65])).is_err());
    assert!(admission::event(&ButtplugWorkerEvent::Devices(vec![device; 64])).is_ok());
}

#[test]
fn engine_construction_is_inert_and_connected_idle_uses_no_actor_polling() {
    use super::super::super::ButtplugModule;
    use crate::app::{AppEngine, AppNode};
    use golden_core::node::{Folder, Node};
    let mut engine = AppEngine::new(Folder::new("root").into());
    engine.add_node(ButtplugModule::create().into(), None);
    for _ in 0..8 {
        engine.apply_edits().unwrap();
    }
    engine.resolve().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let AppNode::ButtplugModule(module) = engine.nodes.get_mut(id).unwrap() else {
        panic!()
    };
    assert!(module.transport.is_none());
    assert!(!module.needs_update());
    let slots = slots();
    let wake = module.service.wake.as_ref().unwrap().notifier.clone();
    let handle = ButtplugTransportHandle::spawn_using(config(), wake, &slots, pool(128), pool(128), || {
        Ok(Fake {
            control: Control::default(),
            action: None,
            cleanup: None,
        })
    })
    .unwrap();
    connected(&handle);
    module.last_transport_config = Some(Box::new(module.transport_config().unwrap()));
    module.transport = Some(Box::new(handle));
    module.transport_dirty = false;
    module.request_service();
    for _ in 0..16 {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        engine.apply_edits().unwrap();
        engine
            .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EngineTick)
            .unwrap();
        engine.apply_edits().unwrap();
        thread::sleep(Duration::from_millis(1));
    }
    for _ in 0..100 {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        let stats = engine.tick_stats();
        assert_eq!(stats.callbacks_fired, 0);
        assert_eq!(stats.wake_callbacks_fired, 0);
        assert_eq!(stats.snapshot_rebuilds, 0);
    }
    drop(engine);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn replacement_connection_waits_for_previous_stop_all_cleanup_to_release_its_module_lease() {
    let slots = RetirementPool::new(NonZeroUsize::new(2).unwrap());
    let owner = RetirementPool::new(NonZeroUsize::MIN);
    let permit = owner.try_reserve().unwrap();
    let (release, gate) = mpsc::channel();
    let handle = ButtplugTransportHandle::spawn_using(
        config(),
        WorkerNotifier::new(|| {}),
        &slots,
        pool(16),
        pool(16),
        move || {
            Ok(worker::Exclusive::new(
                Fake {
                    control: Control::default(),
                    action: None,
                    cleanup: Some(gate),
                },
                permit,
            ))
        },
    )
    .unwrap();
    connected(&handle);
    drop(handle);
    thread::sleep(Duration::from_millis(20));
    assert!(owner.try_reserve().is_err());
    assert_eq!(owner.metrics().active, 1);
    assert_eq!(slots.metrics().active, 1);
    release.send(()).unwrap();
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    let permit = owner.try_reserve().unwrap();
    let handle = ButtplugTransportHandle::spawn_using(
        config(),
        WorkerNotifier::new(|| {}),
        &slots,
        pool(16),
        pool(16),
        move || {
            Ok(worker::Exclusive::new(
                Fake {
                    control: Control::default(),
                    action: None,
                    cleanup: None,
                },
                permit,
            ))
        },
    )
    .unwrap();
    connected(&handle);
    drop(handle);
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}
