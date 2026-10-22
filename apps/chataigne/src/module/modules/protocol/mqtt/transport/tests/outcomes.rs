use super::super::publications::Publications;
use super::*;

#[test]
fn protocol_retransmission_does_not_consume_a_second_accepted_command() {
    let source = pool(8);
    let mut ledger = Publications::default();
    let mut receipts = Vec::new();
    for _ in 0..2 {
        let value = request();
        let bytes = admission::publish_bytes(&value).unwrap();
        let (command, receipt) =
            AdmittedPublish::new(source.try_retain(value, bytes).unwrap(), notify(), NodeId(1), false);
        ledger.queued(command);
        receipts.push(receipt);
    }
    ledger.dispatched(1);
    ledger.dispatched(1);
    ledger.acknowledged(1);
    assert_eq!(receipts[0].status(), PublishStatus::Acknowledged);
    assert_eq!(receipts[1].status(), PublishStatus::Pending);
    ledger.dispatched(2);
    ledger.session_lost();
    assert_eq!(receipts[1].status(), PublishStatus::FailedUnconfirmed);
    assert!(source.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn qos_zero_is_explicitly_unconfirmed_and_never_claims_a_broker_acknowledgement() {
    let source = pool(2);
    let mut value = request();
    value.qos = MqttQos::AtMost;
    let bytes = admission::publish_bytes(&value).unwrap();
    let (command, receipt) = AdmittedPublish::new(source.try_retain(value, bytes).unwrap(), notify(), NodeId(1), false);
    let mut ledger = Publications::default();
    ledger.queued(command);
    ledger.dispatched(0);
    assert_eq!(receipt.status(), PublishStatus::DispatchedUnconfirmed);
}

#[test]
fn reconnect_resumes_existing_packet_identity_or_fails_a_lost_session_without_replay() {
    for present in [false, true] {
        let control = Control::default();
        let lifetimes = slots();
        let mut handle = fake(control.clone(), &lifetimes, pool(16), pool(16));
        let receipt = handle.send(request(), false).unwrap();
        until(|| control.published.load(Ordering::Acquire) == 1);
        control.fail();
        control.event(ack(present));
        if present {
            control.event(Event::Outgoing(rumqttc::Outgoing::Publish(1)));
            control.event(Event::Incoming(Packet::PubAck(rumqttc::PubAck { pkid: 1 })));
        }
        until(|| receipt.status() != PublishStatus::Pending);
        assert_eq!(
            receipt.status(),
            if present {
                PublishStatus::Acknowledged
            } else {
                PublishStatus::FailedUnconfirmed
            }
        );
        assert_eq!(control.published.load(Ordering::Acquire), 1);
        handle.stop();
        drop(handle);
        assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    }
}

#[test]
fn qos_two_waits_for_pubcomp_and_failed_flush_still_accounts_buffered_acknowledgements() {
    let control = Control::default();
    let lifetimes = slots();
    let mut handle = fake(control.clone(), &lifetimes, pool(16), pool(16));
    let mut value = request();
    value.qos = MqttQos::Exactly;
    let receipt = handle.send(value, false).unwrap();
    until(|| control.published.load(Ordering::Acquire) == 1);
    control.event(Event::Incoming(Packet::PubRec(rumqttc::PubRec { pkid: 1 })));
    control.event(Event::Outgoing(rumqttc::Outgoing::PubRel(1)));
    assert_eq!(receipt.status(), PublishStatus::Pending);
    control
        .buffered
        .lock()
        .unwrap()
        .push_back(Event::Incoming(Packet::PubComp(rumqttc::PubComp { pkid: 1 })));
    control.fail();
    until(|| receipt.status() == PublishStatus::Acknowledged);
    handle.stop();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn new_broker_sessions_clear_collisions_and_stale_buffered_events_but_resumed_sessions_keep_them() {
    let mut state = rumqttc::MqttState::new(16, false);
    state.collision = Some(rumqttc::Publish::new("old", rumqttc::QoS::AtLeastOnce, vec![1; 1024]));
    state.events.push_back(Event::Outgoing(rumqttc::Outgoing::Publish(1)));
    worker::restore_session(&mut state, true);
    assert!(state.collision.is_some());
    assert_eq!(state.events.len(), 1);
    worker::restore_session(&mut state, false);
    assert!(state.collision.is_none());
    assert!(state.events.is_empty());
    assert!(admission::diagnostic("x".repeat(1024 * 1024)).capacity() <= 1024);
}
