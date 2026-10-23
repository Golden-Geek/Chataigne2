use super::*;
use std::{
    sync::{atomic::AtomicUsize, mpsc},
    thread,
};

struct Endpoint {
    drops: Arc<AtomicUsize>,
    cleanup: Option<(mpsc::Sender<()>, Mutex<mpsc::Receiver<()>>)>,
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        if let Some((entered, release)) = self.cleanup.take() {
            let _ = entered.send(());
            let _ = release.into_inner().unwrap().recv();
        }
        self.drops.fetch_add(1, Ordering::AcqRel);
    }
}
fn catalog(label: &str) -> MouseRuntimeEvent {
    MouseRuntimeEvent::DevicesChanged(vec![DiscoveredMouseDevice {
        index: 0,
        variant_id: "fake:mouse".into(),
        label: label.into(),
        details: "fixture".into(),
    }])
}
fn edge(index: usize) -> MouseRuntimeEvent {
    MouseRuntimeEvent::Input {
        device: format!("fake:{}", index % 2),
        event: MouseInputEvent::ButtonChanged {
            button: MouseButtonKind::Left,
            pressed: index.is_multiple_of(2),
        },
    }
}
fn endpoint(drops: &Arc<AtomicUsize>) -> Endpoint {
    Endpoint {
        drops: drops.clone(),
        cleanup: None,
    }
}
fn open(registry: &Registry<Endpoint>, drops: &Arc<AtomicUsize>) -> (Subscription<Endpoint>, Broadcast) {
    let mut bus = None;
    let subscription = registry
        .subscribe(MouseInputConfig::default(), |sender| {
            sender.send(catalog("initial"))?;
            bus = Some(sender);
            Ok(endpoint(drops))
        })
        .unwrap();
    (subscription, bus.unwrap())
}
fn existing(registry: &Registry<Endpoint>) -> Subscription<Endpoint> {
    registry
        .subscribe(MouseInputConfig::default(), |_| panic!("must reuse process endpoint"))
        .unwrap()
}

#[test]
fn subscribers_share_one_endpoint_preserve_device_edges_and_retire_independently() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let (mut first, bus) = open(&registry, &drops);
    let mut second = existing(&registry);
    assert_eq!(first.poll().unwrap(), vec![catalog("initial")]);
    assert_eq!(second.poll().unwrap(), vec![catalog("initial")]);
    for index in 0..32 {
        bus.send(edge(index)).unwrap();
    }
    for offset in [0, 16] {
        let expected = (offset..offset + 16).map(edge).collect::<Vec<_>>();
        assert_eq!(first.poll().unwrap(), expected);
        assert_eq!(second.poll().unwrap(), expected);
    }
    drop(first);
    assert_eq!(drops.load(Ordering::Acquire), 0);
    bus.send(edge(33)).unwrap();
    assert_eq!(second.poll().unwrap(), vec![edge(33)]);
    drop(second);
    assert_eq!(drops.load(Ordering::Acquire), 1);
    let (replacement, _) = open(&registry, &drops);
    drop(replacement);
    assert_eq!(drops.load(Ordering::Acquire), 2);
}

#[test]
fn slow_subscriber_rejects_explicitly_without_stalling_or_retiring_its_peer() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let (mut slow, bus) = open(&registry, &drops);
    let mut fast = existing(&registry);
    slow.poll().unwrap();
    fast.poll().unwrap();
    for index in 0..65 {
        bus.send(edge(index)).unwrap();
        assert_eq!(fast.poll().unwrap(), vec![edge(index)]);
    }
    assert!(slow.failure.stopped.load(Ordering::Acquire));
    let third = existing(&registry);
    let fourth = existing(&registry);
    assert!(registry
        .subscribe(MouseInputConfig::default(), |_| panic!(
            "failed subscriber still owns its slot"
        ))
        .is_err());
    drop((third, fourth));
    let mut accepted = Vec::new();
    for _ in 0..4 {
        accepted.extend(slow.poll().unwrap());
    }
    assert_eq!(accepted, (0..64).map(edge).collect::<Vec<_>>());
    assert!(slow.poll().unwrap_err().contains("overload"));
    drop(slow);
    assert_eq!(drops.load(Ordering::Acquire), 0);
    bus.send(edge(65)).unwrap();
    assert_eq!(fast.poll().unwrap(), vec![edge(65)]);
}

#[test]
fn four_subscriber_bound_and_catalog_seed_survive_subscription_churn() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let (first, bus) = open(&registry, &drops);
    let mut subscriptions = vec![first, existing(&registry), existing(&registry), existing(&registry)];
    assert!(registry
        .subscribe(MouseInputConfig::default(), |_| panic!(
            "capacity must reject before open"
        ))
        .err()
        .unwrap()
        .contains("four"));
    bus.send(catalog("latest")).unwrap();
    subscriptions.pop();
    let mut replacement = existing(&registry);
    bus.send(edge(1)).unwrap();
    assert_eq!(replacement.poll().unwrap(), vec![catalog("latest"), edge(1)]);
    assert_eq!(drops.load(Ordering::Acquire), 0);
}

#[test]
fn catalog_byte_pressure_is_local_and_preserves_the_accepted_prefix() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let (mut slow, bus) = open(&registry, &drops);
    let mut fast = existing(&registry);
    slow.poll().unwrap();
    fast.poll().unwrap();
    for index in 0..5 {
        let record = catalog(&format!("catalog {index}"));
        bus.send(record.clone()).unwrap();
        assert_eq!(fast.poll().unwrap(), vec![record]);
    }
    assert_eq!(slow.receiver.metrics().retained_weight, 2 * 1024 * 1024);
    assert_eq!(
        slow.poll().unwrap(),
        (0..4).map(|i| catalog(&format!("catalog {i}"))).collect::<Vec<_>>()
    );
    assert!(slow.poll().unwrap_err().contains("overload"));
    assert_eq!(fast.poll().unwrap(), vec![]);
}

#[test]
fn invalid_native_catalog_notifies_all_subscribers_before_endpoint_disposal() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let (mut first, bus) = open(&registry, &drops);
    let mut second = existing(&registry);
    first.poll().unwrap();
    second.poll().unwrap();
    assert!(bus.send(catalog(&"x".repeat(MAX_DEVICE_TEXT + 1))).is_err());
    for subscriber in [&mut first, &mut second] {
        assert!(subscriber.poll().unwrap_err().contains("1024"));
    }
    assert!(registry
        .subscribe(MouseInputConfig::default(), |_| panic!(
            "failed endpoint is still owned"
        ))
        .is_err());
    assert_eq!(drops.load(Ordering::Acquire), 0);
    drop(first);
    drop(second);
    assert_eq!(drops.load(Ordering::Acquire), 1);
}

#[test]
fn blocked_cleanup_keeps_the_process_registration_slot_until_join_finishes() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let subscription = registry
        .subscribe(MouseInputConfig::default(), |_| {
            Ok(Endpoint {
                drops: drops.clone(),
                cleanup: Some((entered_tx, Mutex::new(release_rx))),
            })
        })
        .unwrap();
    let cleanup = thread::spawn(move || drop(subscription));
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(registry
        .subscribe(MouseInputConfig::default(), |_| panic!(
            "must not register during cleanup"
        ))
        .err()
        .unwrap()
        .contains("retiring"));
    assert_eq!(registry.lifetime.metrics().active, 1);
    assert_eq!(drops.load(Ordering::Acquire), 0);
    release_tx.send(()).unwrap();
    cleanup.join().unwrap();
    let (replacement, _) = open(&registry, &drops);
    drop(replacement);
    assert_eq!(drops.load(Ordering::Acquire), 2);
}

#[test]
fn overlapping_opens_share_the_starting_endpoint_and_failed_open_can_recover() {
    let registry = Arc::new(Registry::new());
    let drops = Arc::new(AtomicUsize::new(0));
    assert!(registry
        .subscribe(
            MouseInputConfig::default(),
            |_| Err("controlled startup failure".into())
        )
        .is_err());
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let first_registry = registry.clone();
    let first_drops = drops.clone();
    let first = thread::spawn(move || {
        first_registry
            .subscribe(MouseInputConfig::default(), |bus| {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                bus.send(catalog("shared startup"))?;
                Ok(endpoint(&first_drops))
            })
            .unwrap()
    });
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let second_registry = registry.clone();
    let second = thread::spawn(move || existing(&second_registry));
    release_tx.send(()).unwrap();
    let mut first = first.join().unwrap();
    let mut second = second.join().unwrap();
    assert_eq!(first.poll().unwrap(), vec![catalog("shared startup")]);
    assert_eq!(second.poll().unwrap(), vec![catalog("shared startup")]);
    drop(first);
    assert_eq!(drops.load(Ordering::Acquire), 0);
    drop(second);
    assert_eq!(drops.load(Ordering::Acquire), 1);
}

#[test]
fn native_panic_notifies_subscribers_before_endpoint_cleanup_and_reconfiguration_recovers() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    for iteration in 0..8 {
        let (mut subscription, bus) = open(&registry, &drops);
        subscription.poll().unwrap();
        let worker = thread::spawn(move || {
            let _stopped = Stopped(bus);
            panic!("controlled native pump failure");
        });
        assert!(worker.join().is_err());
        assert!(subscription.poll().unwrap_err().contains("stopped unexpectedly"));
        assert_eq!(drops.load(Ordering::Acquire), iteration);
        drop(subscription);
        assert_eq!(drops.load(Ordering::Acquire), iteration + 1);
    }
}

#[test]
fn capture_policy_is_owned_by_each_live_subscriber_and_selection() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let (plain, bus) = open(&registry, &drops);
    let devices = vec![
        DiscoveredMouseDevice {
            index: 0,
            variant_id: "fake:a".into(),
            label: "A".into(),
            details: "fixture".into(),
        },
        DiscoveredMouseDevice {
            index: 1,
            variant_id: "fake:b".into(),
            label: "B".into(),
            details: "fixture".into(),
        },
    ];
    assert!(!bus.capture_enabled());
    let capture = registry
        .subscribe(
            MouseInputConfig {
                capture_os_input: true,
                selection: "fake:b".into(),
            },
            |_| panic!("shared endpoint"),
        )
        .unwrap();
    assert!(bus.capture_enabled());
    assert!(!bus.capture_target(&devices, "fake:a"));
    assert!(bus.capture_target(&devices, "fake:b"));
    assert!(bus.has_capture_selection(&devices));
    assert!(!bus.has_capture_selection(&[]));
    drop(capture);
    assert!(!bus.capture_enabled());
    assert!(!bus.capture_target(&devices, "fake:b"));
    drop(plain);
}
#[test]
fn failed_capture_subscriber_stops_capturing_without_affecting_peer_input() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let (mut plain, bus) = open(&registry, &drops);
    let capture = registry
        .subscribe(
            MouseInputConfig {
                capture_os_input: true,
                selection: "auto".into(),
            },
            |_| panic!("shared endpoint"),
        )
        .unwrap();
    plain.poll().unwrap();
    for index in 0..65 {
        bus.send(edge(index)).unwrap();
        plain.poll().unwrap();
    }
    assert!(capture.failure.stopped.load(Ordering::Acquire));
    assert!(!bus.capture_enabled());
    assert!(plain.poll().unwrap().is_empty());
}

#[test]
fn capture_configuration_wakes_the_native_owner_and_rejects_oversized_settings_before_open() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let (plain, bus) = open(&registry, &drops);
    let changes = Arc::new(AtomicUsize::new(0));
    let observed = changes.clone();
    bus.notify_changes(WorkerNotifier::new(move || {
        observed.fetch_add(1, Ordering::AcqRel);
    }));
    let capture = registry
        .subscribe(
            MouseInputConfig {
                capture_os_input: true,
                selection: "auto".into(),
            },
            |_| panic!("shared endpoint"),
        )
        .unwrap();
    assert_eq!(changes.load(Ordering::Acquire), 1);
    drop(capture);
    assert_eq!(changes.load(Ordering::Acquire), 2);
    assert!(registry
        .subscribe(
            MouseInputConfig {
                capture_os_input: true,
                selection: "a".repeat(1025)
            },
            |_| panic!("oversized selection must not open")
        )
        .is_err());
    assert_eq!(changes.load(Ordering::Acquire), 2);
    drop(plain);
}

#[test]
fn actor_retirement_revokes_capture_before_the_subscription_is_dropped() {
    let registry = Registry::new();
    let drops = Arc::new(AtomicUsize::new(0));
    let (mut peer, bus) = open(&registry, &drops);
    peer.poll().unwrap();
    let actor = RevisionSource::default();
    let capture = registry
        .subscribe_guarded(
            MouseInputConfig {
                capture_os_input: true,
                selection: "auto".into(),
            },
            actor.reader().guard_at(0),
            |_| panic!("shared endpoint"),
        )
        .unwrap();
    assert!(bus.capture_enabled());
    drop(actor);
    assert!(!bus.capture_enabled());
    bus.send(edge(0)).unwrap();
    assert_eq!(peer.poll().unwrap(), vec![edge(0)]);
    assert!(capture.failure.stopped.load(Ordering::Acquire));
    assert_eq!(drops.load(Ordering::Acquire), 0);
    drop(capture);
    drop(peer);
    assert_eq!(drops.load(Ordering::Acquire), 1);
}
