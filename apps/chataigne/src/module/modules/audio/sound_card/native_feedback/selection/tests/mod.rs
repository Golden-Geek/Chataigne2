use super::super::super::prepared_event::{AudioEventDraft, EventPreparer};
use super::*;
use golden_audio::{AudioDirection, AudioEvent, AudioStreamStatus, BackendId, PlaybackId, PlaybackInfo, VoiceId};
use std::sync::{mpsc, Condvar};

fn event() -> PreparedAudioEvent {
    let mut status = AudioStreamStatus::disabled(AudioDirection::Output);
    status.selected_target = Some(golden_audio::AudioDeviceTargetId::SystemDefault {
        backend: BackendId::from_static("fixture"),
    });
    EventPreparer::new()
        .unwrap()
        .try_prepare(&mut AudioEventDraft::new(AudioEvent::DeviceStatusChanged(status)))
        .unwrap()
}

fn poll(
    work: &mut SelectionDelivery,
    service: &Service,
    event: &PreparedAudioEvent,
    parameter: NodeId,
    driver: &str,
    value: &str,
) -> SelectionProgress {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let outcome = work.service_using(service, event, parameter, driver, value, None);
        if outcome != SelectionProgress::Waiting {
            return outcome;
        }
        assert!(Instant::now() < deadline, "selection inspection must settle");
        std::thread::sleep(Duration::from_millis(1));
    }
}

struct Blocked {
    service: Service,
    gate: Arc<(Mutex<bool>, Condvar)>,
}
impl Blocked {
    fn release(&self) {
        *self.gate.0.lock().unwrap() = true;
        self.gate.1.notify_all();
    }
}
impl Drop for Blocked {
    fn drop(&mut self) {
        self.release();
    }
}

fn blocked() -> Blocked {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_gate = gate.clone();
    let (entered, ready) = mpsc::channel();
    let service = Service::new(Arc::new(move |_, _, _| {
        entered.send(()).unwrap();
        drop(
            worker_gate
                .1
                .wait_while(worker_gate.0.lock().unwrap(), |open| !*open)
                .unwrap(),
        );
    }));
    let playback = EventPreparer::new()
        .unwrap()
        .try_prepare(&mut AudioEventDraft::new(AudioEvent::PlaybackStarted(PlaybackInfo {
            playback_id: PlaybackId::from_static("selection-blocker"),
            path: "clip.wav".into(),
            voice: VoiceId::new(1, 1),
        })))
        .unwrap();
    service
        .submit(
            playback,
            LogSelection {
                outgoing: true,
                connection: false,
            },
            NodeId(1),
            None,
        )
        .unwrap();
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    Blocked { service, gate }
}

#[test]
fn idle_selection_owns_no_native_request_or_input() {
    let selection = SelectionDelivery::default();
    assert!(selection.current.is_none());
    assert!(selection.retry_at.is_none());
}

#[test]
fn slow_native_inspection_does_not_poll_and_cached_results_do_not_reparse() {
    let fixture = blocked();
    let mut engine = golden_core::engine::Engine::new(golden_core::node::Folder::new("root"));
    let wake = RuntimeWakeSender::new(engine.node_waker(engine.root).unwrap()).unwrap();
    let event = event();
    let mut work = SelectionDelivery::default();
    for _ in 0..64 {
        assert_eq!(
            work.service_using(
                &fixture.service,
                &event,
                NodeId(2),
                "fixture",
                "system_default",
                Some(&wake)
            ),
            SelectionProgress::Waiting
        );
        engine.run_tick(Duration::from_millis(1)).unwrap();
        assert_eq!(engine.tick_stats().wake_callbacks_fired, 0);
    }
    assert_eq!(work.checks, 1);
    fixture.release();
    assert_eq!(
        poll(
            &mut work,
            &fixture.service,
            &event,
            NodeId(2),
            "fixture",
            "system_default"
        ),
        SelectionProgress::Ready(true)
    );
    engine.run_tick(Duration::from_millis(1)).unwrap();
    assert_eq!(engine.tick_stats().wake_callbacks_fired, 1);
    for _ in 0..128 {
        assert_eq!(
            work.service_using(&fixture.service, &event, NodeId(2), "fixture", "system_default", None),
            SelectionProgress::Ready(true)
        );
    }
    assert_eq!(work.checks, 1);
    assert_eq!(fixture.service.lifetime.metrics().peak, 1);
}

#[test]
fn changed_or_removed_controls_cannot_receive_an_older_native_match() {
    let fixture = blocked();
    let event = event();
    let mut work = SelectionDelivery::default();
    assert_eq!(
        work.service_using(&fixture.service, &event, NodeId(2), "fixture", "system_default", None),
        SelectionProgress::Waiting
    );
    assert_eq!(
        work.service_using(&fixture.service, &event, NodeId(2), "fixture", "none", None),
        SelectionProgress::Ready(false)
    );
    assert!(work.current.is_none());
    fixture.release();
    assert_eq!(
        poll(
            &mut work,
            &fixture.service,
            &event,
            NodeId(3),
            "fixture",
            "system_default"
        ),
        SelectionProgress::Ready(true)
    );
    assert_eq!(work.checks, 2, "a replacement parameter needs a new inspection");
    assert_eq!(
        poll(
            &mut work,
            &fixture.service,
            &event,
            NodeId(3),
            "other",
            "system_default"
        ),
        SelectionProgress::Ready(false)
    );
}

#[test]
fn byte_rejection_precedes_source_pool_initialization_and_queue_admission() {
    let service = Service::new(Arc::new(|_, _, _| {}));
    let event = event();
    let mut work = SelectionDelivery::default();
    for (driver, value) in [
        (
            "x".repeat(super::super::super::DRIVER_SELECTION_BYTES + 1),
            "system_default".into(),
        ),
        ("fixture".into(), "x".repeat(SELECTOR_BYTES + 1)),
    ] {
        assert_eq!(
            work.service_using(&service, &event, NodeId(2), &driver, &value, None),
            SelectionProgress::Failed
        );
        assert!(work.current.is_none());
        assert!(service.inputs.lock().unwrap().is_none());
        assert_eq!(service.admission.metrics().active, 0);
    }
}

#[test]
fn cached_controls_keep_the_global_source_quota_until_the_last_owner_releases_them() {
    let service = Service::new(Arc::new(|_, _, _| {}));
    let event = event();
    let mut cache = Vec::new();
    for index in 0..TICKETS {
        let mut work = SelectionDelivery::default();
        assert_eq!(
            poll(
                &mut work,
                &service,
                &event,
                NodeId(index as u64),
                "fixture",
                "system_default"
            ),
            SelectionProgress::Ready(true)
        );
        cache.push(work);
    }
    let mut full = SelectionDelivery::default();
    assert_eq!(
        full.service_using(&service, &event, NodeId(99), "fixture", "system_default", None),
        SelectionProgress::Waiting
    );
    assert!(full.current.is_none() && full.retry_at.is_some());
    drop(cache);
    assert!(service
        .inputs
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .wait_for_idle(Duration::from_secs(3)));
    full.retry_at = Some(Instant::now() - Duration::from_millis(1));
    assert_eq!(
        poll(&mut full, &service, &event, NodeId(99), "fixture", "system_default"),
        SelectionProgress::Ready(true)
    );
}

#[test]
fn cancelled_queue_settles_failed_inspection_and_a_later_event_recovers() {
    let fixture = blocked();
    let event = event();
    let mut work = SelectionDelivery::default();
    work.service_using(&fixture.service, &event, NodeId(2), "fixture", "system_default", None);
    fixture.service.worker.lock().unwrap().take();
    fixture.release();
    assert_eq!(
        poll(
            &mut work,
            &fixture.service,
            &event,
            NodeId(2),
            "fixture",
            "system_default"
        ),
        SelectionProgress::Failed
    );
    assert!(fixture.service.lifetime.wait_for_idle(Duration::from_secs(3)));
    let mut recovered = SelectionDelivery::default();
    assert_eq!(
        poll(
            &mut recovered,
            &fixture.service,
            &event,
            NodeId(2),
            "fixture",
            "system_default"
        ),
        SelectionProgress::Ready(true)
    );
    assert_eq!(fixture.service.lifetime.metrics().peak, 1);
}

#[test]
fn full_job_admission_rearms_its_coarse_deadline_after_an_earlier_wake() {
    let service = Service::new(Arc::new(|_, _, _| {}));
    let permits = (0..TICKETS)
        .map(|_| service.admission.try_reserve().unwrap())
        .collect::<Vec<_>>();
    let mut engine = golden_core::engine::Engine::new(golden_core::node::Folder::new("root"));
    let wake = RuntimeWakeSender::new(engine.node_waker(engine.root).unwrap()).unwrap();
    let event = event();
    let mut work = SelectionDelivery::default();
    assert_eq!(
        work.service_using(&service, &event, NodeId(2), "fixture", "system_default", Some(&wake)),
        SelectionProgress::Waiting
    );
    let retry_at = work.retry_at.unwrap();
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
    assert!(Instant::now() < retry_at);
    work.service_using(&service, &event, NodeId(2), "fixture", "system_default", Some(&wake));
    assert_eq!(
        service.admission.metrics().rejected,
        1,
        "an early wake must not re-admit immediately"
    );
    drop(permits);
    loop {
        engine.run_tick(Duration::from_millis(1)).unwrap();
        if engine.tick_stats().wake_callbacks_fired > 0 {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(Instant::now() >= retry_at);
    assert_eq!(
        poll(&mut work, &service, &event, NodeId(2), "fixture", "system_default"),
        SelectionProgress::Ready(true)
    );
}

#[test]
fn encoded_and_maximum_malformed_selectors_are_inspected_on_the_native_owner() {
    let service = Service::new(Arc::new(|_, _, _| {}));
    let event = event();
    let selected = golden_audio::AudioDeviceSelection::follow_system_default(
        BackendId::from_static("fixture"),
        AudioDirection::Output,
    );
    let mut value = serde_json::to_string(&selected).unwrap();
    value.reserve(SELECTOR_BYTES * 2);
    let mut work = SelectionDelivery::default();
    assert_eq!(
        poll(&mut work, &service, &event, NodeId(2), "fixture", &value),
        SelectionProgress::Ready(true)
    );
    assert!(
        work.current.as_ref().unwrap().inputs.value.capacity() < value.capacity(),
        "only borrowed admitted bytes are copied, not caller spare capacity"
    );
    let malformed = " ".repeat(SELECTOR_BYTES);
    assert_eq!(
        poll(&mut work, &service, &event, NodeId(2), "fixture", &malformed),
        SelectionProgress::Ready(false)
    );
}
