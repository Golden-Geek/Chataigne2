use super::super::prepared_event::{AudioEventDraft, EventPreparer};
use super::*;
use golden_audio::{AudioEvent, PlaybackId, PlaybackInfo, VoiceId};
use std::sync::{atomic::AtomicBool, mpsc, Condvar};

fn playback() -> PreparedAudioEvent {
    let mut draft = AudioEventDraft::new(AudioEvent::PlaybackStarted(PlaybackInfo {
        playback_id: PlaybackId::from_static("log-fixture"),
        path: "clip.wav".into(),
        voice: VoiceId::new(1, 2),
    }));
    EventPreparer::new().unwrap().try_prepare(&mut draft).unwrap()
}

fn selection() -> LogSelection {
    LogSelection {
        outgoing: true,
        connection: true,
    }
}

fn wait(completion: &Completion) -> u8 {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let status = completion.status.load(Ordering::Acquire);
        if status != PENDING {
            return status;
        }
        assert!(Instant::now() < deadline, "native logging completion must settle");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[derive(Default)]
struct Gate {
    open: Mutex<bool>,
    changed: Condvar,
}
impl Gate {
    fn wait(&self) {
        let guard = self.open.lock().unwrap();
        drop(self.changed.wait_while(guard, |open| !*open).unwrap());
    }
    fn release(&self) {
        *self.open.lock().unwrap() = true;
        self.changed.notify_all();
    }
}

type Seen = Arc<Mutex<Vec<(LogLevel, NodeId, String, String)>>>;
struct BlockedSink {
    service: Service,
    gate: Arc<Gate>,
    entered: mpsc::Receiver<()>,
    seen: Seen,
}
impl Drop for BlockedSink {
    fn drop(&mut self) {
        self.gate.release();
    }
}
fn blocked() -> BlockedSink {
    let gate = Arc::new(Gate::default());
    let seen = Seen::default();
    let (entered, receiver) = mpsc::channel();
    let gate_worker = gate.clone();
    let seen_worker = seen.clone();
    let service = Service::new(Arc::new(move |level, origin, message| {
        entered.send(()).unwrap();
        gate_worker.wait();
        seen_worker.lock().unwrap().push((
            level,
            origin,
            message.into(),
            std::thread::current().name().unwrap().into(),
        ));
    }));
    BlockedSink {
        service,
        gate,
        entered: receiver,
        seen,
    }
}

#[test]
fn slow_logging_is_native_and_completion_wakes_without_actor_polling() {
    let mut engine = golden_core::engine::Engine::new(golden_core::node::Folder::new("root"));
    let wake = RuntimeWakeSender::new(engine.node_waker(engine.root).unwrap()).unwrap();
    let fixture = blocked();
    let event = playback();
    let mut delivery = LogDelivery::default();
    assert_eq!(
        delivery.service_using(&fixture.service, &event, selection(), NodeId(42), Some(&wake)),
        LogProgress::Waiting
    );
    fixture.entered.recv_timeout(Duration::from_secs(3)).unwrap();
    for _ in 0..64 {
        assert_eq!(
            delivery.service_using(
                &fixture.service,
                &event,
                LogSelection {
                    outgoing: false,
                    connection: false
                },
                NodeId(99),
                Some(&wake)
            ),
            LogProgress::Waiting
        );
        engine.run_tick(Duration::from_millis(1)).unwrap();
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    }
    assert_eq!(fixture.service.admission.metrics().active, 1);
    fixture.gate.release();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        if engine.tick_stats().wake_callbacks_fired > 0 {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        delivery.service_using(&fixture.service, &event, selection(), NodeId(99), Some(&wake)),
        LogProgress::Applied
    );
    let seen = fixture.seen.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, LogLevel::Info);
    assert_eq!(seen[0].1, NodeId(42), "admission freezes origin and policy");
    assert_eq!(seen[0].2, event.outgoing_log.as_ref().unwrap().as_str());
    assert_eq!(seen[0].3, "sound-card-native-feedback");
}

#[test]
fn full_ticket_admission_keeps_payloads_and_defers_without_repeated_attempts() {
    let fixture = blocked();
    let event = playback();
    let mut tickets = Vec::new();
    for index in 0..TICKETS {
        tickets.push(
            fixture
                .service
                .submit(event.clone(), selection(), NodeId(index as u64), None)
                .unwrap(),
        );
    }
    fixture.entered.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(fixture.service.admission.metrics().active, TICKETS);
    let mut deferred = LogDelivery::default();
    assert_eq!(
        deferred.service_using(&fixture.service, &event, selection(), NodeId(100), None),
        LogProgress::Waiting
    );
    let rejected = fixture.service.admission.metrics().rejected;
    assert!(deferred.retry_at.is_some());
    for _ in 0..128 {
        assert_eq!(
            deferred.service_using(&fixture.service, &event, selection(), NodeId(100), None),
            LogProgress::Waiting
        );
    }
    assert_eq!(fixture.service.admission.metrics().rejected, rejected);
    fixture.gate.release();
    for ticket in &tickets {
        assert_eq!(wait(ticket), APPLIED);
    }
    assert_eq!(fixture.seen.lock().unwrap().len(), TICKETS);
    tickets.clear();
    deferred.retry_at = Some(Instant::now());
    assert_eq!(
        deferred.service_using(&fixture.service, &event, selection(), NodeId(100), None),
        LogProgress::Waiting
    );
    let DeliveryState::Pending(completion) = &deferred.state else {
        panic!("retry must be admitted");
    };
    assert_eq!(wait(completion), APPLIED);
    assert_eq!(fixture.seen.lock().unwrap().last().unwrap().1, NodeId(100));
}

#[test]
fn writer_failure_has_an_explicit_terminal_receipt_and_does_not_replay() {
    let failed = AtomicBool::new(false);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let written = calls.clone();
    let service = Service::new(Arc::new(move |_, origin, _| {
        written.lock().unwrap().push(origin);
        if !failed.swap(true, Ordering::AcqRel) {
            panic!("controlled log writer failure");
        }
    }));
    let event = playback();
    let mut delivery = LogDelivery::default();
    delivery.service_using(&service, &event, selection(), NodeId(1), None);
    let DeliveryState::Pending(completion) = &delivery.state else {
        panic!("accepted log");
    };
    assert_eq!(wait(completion), FAILED);
    for _ in 0..16 {
        assert_eq!(
            delivery.service_using(&service, &event, selection(), NodeId(1), None),
            LogProgress::Failed
        );
    }
    let healthy = service.submit(event, selection(), NodeId(2), None).unwrap();
    assert_eq!(wait(&healthy), APPLIED);
    assert_eq!(calls.lock().unwrap().as_slice(), &[NodeId(1), NodeId(2)]);
    assert_eq!(service.lifetime.metrics().peak, 1);
}

#[test]
fn cancellation_settles_queued_logs_and_cannot_grow_workers_behind_a_blocked_sink() {
    let fixture = blocked();
    let event = playback();
    let first = fixture
        .service
        .submit(event.clone(), selection(), NodeId(1), None)
        .unwrap();
    fixture.entered.recv_timeout(Duration::from_secs(3)).unwrap();
    let queued = fixture
        .service
        .submit(event.clone(), selection(), NodeId(2), None)
        .unwrap();
    fixture.service.worker.lock().unwrap().take();
    for _ in 0..16 {
        assert!(fixture
            .service
            .submit(event.clone(), selection(), NodeId(3), None)
            .is_err());
    }
    assert_eq!(fixture.service.lifetime.metrics().peak, 1);
    fixture.gate.release();
    assert_eq!(wait(&first), APPLIED);
    assert_eq!(wait(&queued), FAILED);
    assert!(fixture.service.lifetime.wait_for_idle(Duration::from_secs(3)));
    let recovered = fixture.service.submit(event, selection(), NodeId(3), None).unwrap();
    assert_eq!(wait(&recovered), APPLIED);
    assert_eq!(
        fixture
            .seen
            .lock()
            .unwrap()
            .iter()
            .map(|entry| entry.1)
            .collect::<Vec<_>>(),
        vec![NodeId(1), NodeId(3)]
    );
}

impl LogDelivery {
    pub(in super::super) fn pending_for_test(&mut self, wake: RuntimeWakeSender) -> Box<dyn FnOnce(bool) + Send> {
        let admission = RetirementPool::new(NonZeroUsize::MIN).try_reserve().unwrap();
        let completion = Arc::new(Completion {
            status: AtomicU8::new(PENDING),
            wake: Some(wake),
            _admission: admission,
        });
        self.state = DeliveryState::Pending(completion.clone());
        Box::new(move |applied| {
            completion
                .status
                .store(if applied { APPLIED } else { FAILED }, Ordering::Release);
            completion.wake.as_ref().unwrap().wake();
        })
    }
}

#[test]
fn earlier_coalesced_wake_rearms_the_unelapsed_log_admission_deadline() {
    let service = Service::new(Arc::new(|_, _, _| {}));
    let permits = (0..TICKETS)
        .map(|_| service.admission.try_reserve().unwrap())
        .collect::<Vec<_>>();
    let mut engine = golden_core::engine::Engine::new(golden_core::node::Folder::new("root"));
    let wake = RuntimeWakeSender::new(engine.node_waker(engine.root).unwrap()).unwrap();
    let mut delivery = LogDelivery::default();
    let event = playback();
    assert_eq!(
        delivery.service_using(&service, &event, selection(), NodeId(1), Some(&wake)),
        LogProgress::Waiting
    );
    let retry_at = delivery.retry_at.unwrap();
    wake.wake_after(Duration::from_millis(1));
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        if engine.tick_stats().wake_callbacks_fired > 0 {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        Instant::now() < retry_at,
        "fixture must consume the earlier deadline first"
    );
    assert_eq!(
        delivery.service_using(&service, &event, selection(), NodeId(1), Some(&wake)),
        LogProgress::Waiting
    );
    assert_eq!(service.admission.metrics().rejected, 1);
    drop(permits);
    loop {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        if engine.tick_stats().wake_callbacks_fired > 0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "remaining logging retry must wake without manual polling"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(Instant::now() >= retry_at);
    delivery.service_using(&service, &event, selection(), NodeId(1), Some(&wake));
    let DeliveryState::Pending(completion) = &delivery.state else {
        panic!("deferred log must be admitted");
    };
    assert_eq!(wait(completion), APPLIED);
}

#[test]
fn disabled_policy_before_admission_does_not_wait_for_a_capacity_retry() {
    let service = Service::new(Arc::new(|_, _, _| panic!("disabled message must not reach the logger")));
    let _permits = (0..TICKETS)
        .map(|_| service.admission.try_reserve().unwrap())
        .collect::<Vec<_>>();
    let event = playback();
    let mut delivery = LogDelivery::default();
    assert_eq!(
        delivery.service_using(&service, &event, selection(), NodeId(1), None),
        LogProgress::Waiting
    );
    assert_eq!(
        delivery.service_using(
            &service,
            &event,
            LogSelection {
                outgoing: false,
                connection: false
            },
            NodeId(1),
            None
        ),
        LogProgress::Applied
    );
    assert_eq!(service.admission.metrics().rejected, 1);
    assert!(service.worker.lock().unwrap().is_none());
}

#[test]
fn disabled_log_policy_needs_neither_worker_nor_admission() {
    let service = Service::new(Arc::new(|_, _, _| panic!("disabled log must not reach sink")));
    let mut delivery = LogDelivery::default();
    assert_eq!(
        delivery.service_using(
            &service,
            &playback(),
            LogSelection {
                outgoing: false,
                connection: false
            },
            NodeId(1),
            None
        ),
        LogProgress::Applied
    );
    assert!(service.worker.lock().unwrap().is_none());
    assert_eq!(service.admission.metrics().active, 0);
}

#[test]
fn native_display_message_bound_is_utf8_safe_and_marked() {
    assert_eq!(bounded_message("unchanged"), "unchanged");
    let source = "€".repeat(MESSAGE_BYTES);
    let bounded = bounded_message(&source);
    assert!(bounded.len() <= MESSAGE_BYTES);
    assert!(bounded.ends_with(" [truncated]"));
    assert!(source.starts_with(bounded.strip_suffix(" [truncated]").unwrap()));
}

#[test]
fn admitted_log_keeps_event_storage_charged_after_its_actor_owner_disappears() {
    let events = golden_io::PayloadPool::new("native-log-event-test", 1, 1024 * 1024).unwrap();
    let callbacks = golden_io::PayloadPool::new("native-log-callback-test", 1, 1024 * 1024).unwrap();
    let preparer = EventPreparer::using(events.clone(), callbacks.clone());
    let mut draft = AudioEventDraft::new(AudioEvent::PlaybackStarted(PlaybackInfo {
        playback_id: PlaybackId::from_static("retained-log"),
        path: "clip.wav".into(),
        voice: VoiceId::new(1, 2),
    }));
    let event = preparer.try_prepare(&mut draft).unwrap();
    drop(draft);
    let fixture = blocked();
    let ticket = fixture
        .service
        .submit(event.clone(), selection(), NodeId(7), None)
        .unwrap();
    fixture.entered.recv_timeout(Duration::from_secs(3)).unwrap();
    drop(event);
    assert!(events.try_reserve(1).is_err());
    assert!(callbacks.try_reserve(1).is_err());
    fixture.gate.release();
    assert_eq!(wait(&ticket), APPLIED);
    assert!(events.wait_for_idle(Duration::from_secs(3)));
    assert!(callbacks.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn connection_messages_keep_severity_origin_and_native_publication_order() {
    let records = Arc::new(Mutex::new(Vec::new()));
    let sink_records = records.clone();
    let service = Service::new(Arc::new(move |level, origin, _| {
        sink_records.lock().unwrap().push((level, origin));
    }));
    for (readiness, level) in [
        (golden_audio::AudioDeviceReadiness::Recovering, LogLevel::Info),
        (golden_audio::AudioDeviceReadiness::Ready, LogLevel::Success),
        (golden_audio::AudioDeviceReadiness::Busy, LogLevel::Error),
    ] {
        let mut status = golden_audio::AudioStreamStatus::disabled(golden_audio::AudioDirection::Output);
        status.enabled = true;
        status.readiness = readiness;
        status.selected_label = Some("Fixture".into());
        let event = EventPreparer::new()
            .unwrap()
            .try_prepare(&mut AudioEventDraft::new(AudioEvent::DeviceStatusChanged(status)))
            .unwrap();
        let completion = service
            .submit(
                event,
                LogSelection {
                    outgoing: false,
                    connection: true,
                },
                NodeId(17),
                None,
            )
            .unwrap();
        assert_eq!(wait(&completion), APPLIED);
        assert_eq!(records.lock().unwrap().last(), Some(&(level, NodeId(17))));
    }
    assert_eq!(
        records.lock().unwrap().as_slice(),
        &[
            (LogLevel::Info, NodeId(17)),
            (LogLevel::Success, NodeId(17)),
            (LogLevel::Error, NodeId(17))
        ]
    );
}
