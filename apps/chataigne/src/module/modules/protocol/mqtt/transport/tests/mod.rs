use super::*;
use crate::app::module::common::mqtt::MqttQos;
use rumqttc::{Event, Packet};
use std::{
    collections::VecDeque,
    sync::{atomic::AtomicUsize, mpsc, Mutex},
    time::{Duration, Instant},
};
use worker::{Broker, BrokerPoll};

mod lifecycle;
mod outcomes;
mod outgoing;
mod wire;

fn config() -> MqttTransportConfig {
    MqttTransportConfig {
        remote_host: "127.0.0.1".into(),
        remote_port: 1883,
        client_id: "bounded-test".into(),
        credentials: None,
        clean_session: true,
        keep_alive_secs: 30,
        subscriptions: vec![],
    }
}
fn request() -> MqttPublishRequest {
    MqttPublishRequest {
        topic: "test/value".into(),
        payload: b"one".to_vec(),
        qos: MqttQos::AtLeast,
        retain: false,
        description: "publish".into(),
    }
}
fn pool(items: usize) -> PayloadPool {
    PayloadPool::new("mqtt-fixture-payloads", items, 4 * 1024 * 1024).unwrap()
}
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn notify() -> WorkerNotifier {
    WorkerNotifier::new(|| {})
}
fn ack(present: bool) -> Event {
    Event::Incoming(Packet::ConnAck(rumqttc::ConnAck {
        session_present: present,
        code: rumqttc::ConnectReturnCode::Success,
    }))
}
fn until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "bounded worker did not make expected progress"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[derive(Default, Clone)]
struct Control {
    events: Arc<Mutex<VecDeque<Result<BrokerPoll, String>>>>,
    published: Arc<AtomicUsize>,
    attempts: Arc<AtomicUsize>,
    last_request: Arc<Mutex<Option<MqttPublishRequest>>>,
    busy: Arc<AtomicBool>,
    panic: Arc<AtomicBool>,
    buffered: Arc<Mutex<VecDeque<Event>>>,
}
impl Control {
    fn event(&self, event: Event) {
        self.events.lock().unwrap().push_back(Ok(BrokerPoll::Event(event)));
    }
    fn fail(&self) {
        self.events
            .lock()
            .unwrap()
            .push_back(Err("injected broker failure".into()));
    }
}
struct FakeBroker {
    control: Control,
    held: Vec<Vec<u8>>,
    cleanup: Option<(mpsc::SyncSender<()>, mpsc::Receiver<()>)>,
}
impl FakeBroker {
    fn new(control: Control) -> Self {
        control.event(ack(false));
        Self {
            control,
            held: Vec::new(),
            cleanup: None,
        }
    }
}
impl Broker for FakeBroker {
    fn poll(&mut self) -> Result<BrokerPoll, String> {
        assert!(
            !self.control.panic.load(Ordering::Acquire),
            "injected native worker failure"
        );
        if let Some(event) = self.control.events.lock().unwrap().pop_front() {
            return event;
        }
        std::thread::sleep(Duration::from_millis(1));
        Ok(BrokerPoll::Idle)
    }
    fn publish(&mut self, request: &MqttPublishRequest) -> bool {
        self.control.attempts.fetch_add(1, Ordering::AcqRel);
        if self.control.busy.load(Ordering::Acquire) {
            return false;
        }
        let id = self.control.published.load(Ordering::Acquire) + 1;
        self.held.push(request.payload.clone());
        *self.control.last_request.lock().unwrap() = Some(request.clone());
        self.control.published.store(id, Ordering::Release);
        self.control
            .event(Event::Outgoing(rumqttc::Outgoing::Publish(id as u16)));
        true
    }
    fn subscribe(&mut self, _: &MqttSubscriptionConfig) -> bool {
        true
    }
    fn buffered(&mut self) -> Option<Event> {
        self.control.buffered.lock().unwrap().pop_front()
    }
}
impl Drop for FakeBroker {
    fn drop(&mut self) {
        if let Some((entered, release)) = self.cleanup.take() {
            let _ = entered.send(());
            let _ = release.recv();
        }
    }
}
fn fake(control: Control, lifetimes: &RetirementPool, output: PayloadPool, input: PayloadPool) -> MqttTransportHandle {
    let mut cfg = config();
    cfg.clean_session = false;
    let handle = MqttTransportHandle::spawn_using(cfg, NodeId(1), notify(), lifetimes, output, input, move |_| {
        FakeBroker::new(control)
    })
    .unwrap();
    until(|| handle.connected.load(Ordering::Acquire));
    handle
}

#[test]
fn native_admission_counts_capacity_and_wire_headers_before_publication() {
    let mut value = request();
    value.payload = Vec::with_capacity(MAX_PACKET + 1);
    assert!(admission::publish_bytes(&value).is_err());
    value.payload = vec![0; MAX_PACKET];
    assert!(admission::publish_bytes(&value).is_err());
    let mut cfg = config();
    cfg.subscriptions = Vec::with_capacity(129);
    assert!(admission::config_bytes(&cfg).is_err());
    let lifetimes = slots();
    let result = MqttTransportHandle::spawn_using(
        cfg,
        NodeId(1),
        notify(),
        &lifetimes,
        pool(8),
        pool(8),
        |_| -> FakeBroker { panic!("invalid configuration reached native setup") },
    );
    assert!(result.is_err());
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}
