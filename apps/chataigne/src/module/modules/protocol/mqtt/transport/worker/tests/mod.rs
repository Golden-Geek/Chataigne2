use super::*;
use std::{
    sync::{atomic::AtomicUsize, Mutex},
    task::{Poll, Waker},
};

type Gate = Arc<(AtomicBool, Mutex<Option<Waker>>)>;

fn pending_broker() -> (NativeBroker, Gate, Arc<AtomicUsize>, Arc<AtomicBool>) {
    let config = MqttTransportConfig {
        remote_host: "unused.test".into(),
        remote_port: 1883,
        client_id: "poll-fixture".into(),
        credentials: None,
        clean_session: true,
        keep_alive_secs: 30,
        subscriptions: vec![],
    };
    let mut broker = NativeBroker::with_resolver(&config, |_, _| panic!("an in-progress poll must not resolve again"));
    let eventloop = broker.eventloop.take().unwrap();
    let gate = Arc::new((AtomicBool::new(false), Mutex::new(None)));
    let starts = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicBool::new(false));
    let (released, started, destroyed) = (gate.clone(), starts.clone(), dropped.clone());
    broker.polling = Some(Box::pin(async move {
        struct Witness(Arc<AtomicBool>);
        impl Drop for Witness {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let _witness = Witness(destroyed);
        started.fetch_add(1, Ordering::AcqRel);
        std::future::poll_fn(|ctx| {
            if released.0.load(Ordering::Acquire) {
                Poll::Ready(())
            } else {
                *released.1.lock().unwrap() = Some(ctx.waker().clone());
                Poll::Pending
            }
        })
        .await;
        (
            eventloop,
            Ok(Event::Incoming(Packet::ConnAck(rumqttc::ConnAck {
                session_present: false,
                code: rumqttc::ConnectReturnCode::Success,
            }))),
        )
    }));
    (broker, gate, starts, dropped)
}

#[test]
fn short_service_turns_preserve_one_protocol_poll_and_its_completion() {
    let (mut broker, gate, starts, _) = pending_broker();
    for _ in 0..3 {
        broker.prepare().unwrap();
        assert!(matches!(broker.poll().unwrap(), BrokerPoll::Idle));
        assert_eq!(starts.load(Ordering::Acquire), 1);
    }
    gate.0.store(true, Ordering::Release);
    if let Some(waker) = gate.1.lock().unwrap().take() {
        waker.wake();
    }
    assert!(matches!(
        broker.poll().unwrap(),
        BrokerPoll::Event(Event::Incoming(Packet::ConnAck(_)))
    ));
    assert!(broker.polling.is_none() && broker.eventloop.is_some());
}

#[test]
fn teardown_drops_the_pending_protocol_operation_instead_of_detaching_it() {
    let (mut broker, _gate, starts, dropped) = pending_broker();
    assert!(matches!(broker.poll().unwrap(), BrokerPoll::Idle));
    assert_eq!(starts.load(Ordering::Acquire), 1);
    assert!(!dropped.load(Ordering::Acquire));
    drop(broker);
    assert!(dropped.load(Ordering::Acquire));
}
