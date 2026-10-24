use super::*;
use crate::app::module_modules_protocol_dmx::transport::{ReceivePoll, input::Receiver};
use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::mpsc,
    time::Duration,
};

mod priority;

fn config(protocol: DmxProtocol) -> DmxTransportConfig {
    DmxTransportConfig {
        protocol,
        source_id: [77; 16],
        bind_ip: Ipv4Addr::LOCALHOST.into(),
        listen_port: protocol.default_port(),
        receive_enabled: false,
        universe: 1,
        destination: Some((Ipv4Addr::LOCALHOST, 12345).into()),
    }
}

#[derive(Debug)]
struct Packet {
    bytes: Vec<u8>,
    destination: SocketAddr,
}
fn capture(packets: &mut Vec<Packet>) -> impl FnMut(&[u8], SocketAddr) -> Result<(), String> + '_ {
    |bytes, destination| {
        packets.push(Packet {
            bytes: bytes.to_vec(),
            destination,
        });
        Ok(())
    }
}
fn sequence(protocol: DmxProtocol, packet: &Packet) -> u8 {
    match protocol {
        DmxProtocol::ArtNet => {
            let artnet_protocol::ArtCommand::Output(frame) =
                artnet_protocol::ArtCommand::from_buffer(&packet.bytes).unwrap()
            else {
                panic!("ArtDMX")
            };
            frame.sequence
        }
        DmxProtocol::Sacn => {
            let sacn::packet::Payload::Data(frame) = sacn::Packet::parse(&packet.bytes).unwrap().payload else {
                panic!("sACN data")
            };
            assert!(!frame.stream_terminated);
            frame.sequence_number.get()
        }
    }
}

#[test]
fn reconnect_restores_confirmed_look_and_receiver_accepts_continuing_sequence() {
    for protocol in [DmxProtocol::ArtNet, DmxProtocol::Sacn] {
        let config = config(protocol);
        let mut owner = OutputContinuity::default();
        let first = owner.claim(&config);
        let now = Instant::now();
        let mut packets = Vec::new();
        first
            .send(
                &DmxFrame::new(1, vec![11; 512]).unwrap(),
                now,
                &mut capture(&mut packets),
            )
            .unwrap();
        assert!(
            first
                .send(
                    &DmxFrame::new(1, vec![77; 512]).unwrap(),
                    now + Duration::from_millis(23),
                    &mut |_, _| Err("failed submission".into())
                )
                .is_err()
        );
        first.suspend();
        let recovered = owner.claim(&config);
        assert!(
            !recovered.needs_initial_look(),
            "failed command must not become initialization"
        );
        recovered
            .poll(now + Duration::from_millis(30), &mut capture(&mut packets))
            .unwrap();
        if packets.len() == 1 {
            let next = recovered.deadline(now).expect("restore has a paced deadline");
            assert!(next <= now + Duration::from_millis(53));
            recovered.poll(next, &mut capture(&mut packets)).unwrap();
        }
        assert_eq!(packets.len(), 2);
        assert_ne!(sequence(protocol, &packets[0]), sequence(protocol, &packets[1]));
        let mut receiver = Receiver::new(&config);
        for (index, packet) in packets.iter().enumerate() {
            assert_eq!(packet.destination, config.destination.unwrap());
            let ReceivePoll::Frame(frame) = receiver.handle(
                Duration::from_millis(index as u64 * 30),
                (Ipv4Addr::LOCALHOST, 54321).into(),
                &packet.bytes,
            ) else {
                panic!("receiver rejected restored sequence")
            };
            assert_eq!(frame.slots, vec![11; 512]);
        }
        assert!(!first.terminate(), "retiring cleanup cannot terminate the replacement");
    }
}

#[test]
fn first_submission_failure_stays_silent_until_a_new_explicit_intent() {
    for protocol in [DmxProtocol::ArtNet, DmxProtocol::Sacn] {
        let config = config(protocol);
        let mut owner = OutputContinuity::default();
        let first = owner.claim(&config);
        let now = Instant::now();
        assert!(
            first
                .send(&DmxFrame::new(1, vec![7]).unwrap(), now, &mut |_, _| Err(
                    "failed".into()
                ))
                .is_err()
        );
        first.suspend();
        let recovered = owner.claim(&config);
        let mut packets = Vec::new();
        assert!(
            !recovered
                .poll(now + Duration::from_millis(30), &mut capture(&mut packets))
                .unwrap()
        );
        assert!(packets.is_empty());
        assert!(recovered.deadline(now + Duration::from_millis(30)).is_none());
        recovered
            .send(
                &DmxFrame::new(1, vec![44; 512]).unwrap(),
                now + Duration::from_millis(30),
                &mut capture(&mut packets),
            )
            .unwrap();
        assert_eq!(packets.len(), 1);
        let mut receiver = Receiver::new(&config);
        let ReceivePoll::Frame(frame) =
            receiver.handle(Duration::ZERO, (Ipv4Addr::LOCALHOST, 54321).into(), &packets[0].bytes)
        else {
            panic!("new intent")
        };
        assert_eq!(frame.slots, vec![44; 512]);
    }
}

#[test]
fn new_owner_cancels_partial_termination_without_bypassing_data_pacing() {
    for protocol in [DmxProtocol::ArtNet, DmxProtocol::Sacn] {
        let config = config(protocol);
        let mut owner = OutputContinuity::default();
        let first = owner.claim(&config);
        let now = Instant::now();
        let mut packets = Vec::new();
        first
            .send(&DmxFrame::new(1, vec![33]).unwrap(), now, &mut capture(&mut packets))
            .unwrap();
        assert!(first.terminate());
        let recovered = owner.claim(&config);
        assert!(
            !recovered
                .poll(now + Duration::from_millis(11), &mut capture(&mut packets))
                .unwrap()
        );
        assert!(
            first
                .poll(now + Duration::from_millis(23), &mut capture(&mut packets))
                .is_err()
        );
        assert!(
            recovered
                .poll(now + Duration::from_millis(23), &mut capture(&mut packets))
                .unwrap()
        );
        assert_eq!(packets.len(), 2);
        assert_eq!(sequence(protocol, &packets[1]), sequence(protocol, &packets[0]) + 1);
    }
}

#[test]
fn route_changes_and_returning_retiring_universes_keep_source_state_and_fence_old_senders() {
    let mut config = config(DmxProtocol::Sacn);
    let mut owner = OutputContinuity::default();
    let first = owner.claim(&config);
    let now = Instant::now();
    let mut packets = Vec::new();
    first
        .send(&DmxFrame::new(1, vec![9]).unwrap(), now, &mut capture(&mut packets))
        .unwrap();
    config.universe = 2;
    let second = owner.claim(&config);
    second
        .send(
            &DmxFrame::new(2, vec![22]).unwrap(),
            now + Duration::from_millis(23),
            &mut capture(&mut packets),
        )
        .unwrap();
    config.universe = 1;
    config.destination = Some((Ipv4Addr::LOCALHOST, 12346).into());
    let third = owner.claim(&config);
    third
        .poll(now + Duration::from_millis(46), &mut capture(&mut packets))
        .unwrap();
    assert_eq!(packets.len(), 3);
    assert_eq!(packets[2].destination, config.destination.unwrap());
    assert_eq!(
        sequence(DmxProtocol::Sacn, &packets[2]),
        sequence(DmxProtocol::Sacn, &packets[0]) + 1
    );
    assert!(!first.terminate());
    assert!(!second.terminate());
}

#[test]
fn authored_lifecycle_restart_uses_new_authored_state_without_restarting_sequence() {
    let config = config(DmxProtocol::Sacn);
    let mut owner = OutputContinuity::default();
    let first = owner.claim(&config);
    let now = Instant::now();
    let mut packets = Vec::new();
    first
        .send(&DmxFrame::new(1, vec![33]).unwrap(), now, &mut capture(&mut packets))
        .unwrap();
    owner.restart_from_authored();
    let next = owner.claim(&config);
    assert!(next.needs_initial_look());
    assert!(
        !next
            .poll(now + Duration::from_millis(30), &mut capture(&mut packets))
            .unwrap()
    );
    next.send(
        &DmxFrame::new(1, vec![9]).unwrap(),
        now + Duration::from_millis(30),
        &mut capture(&mut packets),
    )
    .unwrap();
    assert_eq!(packets.len(), 2);
    assert_eq!(
        sequence(DmxProtocol::Sacn, &packets[1]),
        sequence(DmxProtocol::Sacn, &packets[0]) + 1
    );
}

#[test]
fn actor_claim_and_lifecycle_restart_do_not_wait_for_native_protocol_locks() {
    let config = config(DmxProtocol::Sacn);
    let mut owner = OutputContinuity::default();
    let first = owner.claim(&config);
    let stream = first.stream.clone();
    let (entered, ready) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let _state = stream.state.lock().unwrap();
        entered.send(()).unwrap();
        let _ = wait.recv_timeout(Duration::from_secs(1));
    });
    ready.recv_timeout(Duration::from_secs(1)).unwrap();
    let started = Instant::now();
    owner.restart_from_authored();
    let _next = owner.claim(&config);
    let elapsed = started.elapsed();
    release.send(()).unwrap();
    worker.join().unwrap();
    assert!(elapsed < Duration::from_millis(100), "actor waited for native state");
}

#[test]
fn a_replacement_that_never_activates_does_not_suppress_predecessor_termination() {
    let config = config(DmxProtocol::Sacn);
    let mut owner = OutputContinuity::default();
    let first = owner.claim(&config);
    let now = Instant::now();
    let mut packets = Vec::new();
    first
        .send(&DmxFrame::new(1, vec![33]).unwrap(), now, &mut capture(&mut packets))
        .unwrap();
    let mut new_route = config.clone();
    new_route.destination = Some((Ipv4Addr::LOCALHOST, 12346).into());
    let _unopened = owner.claim(&new_route);
    assert!(first.terminate());
    while let Some(at) = first.deadline(now) {
        first.poll(at, &mut capture(&mut packets)).unwrap();
        assert!(packets.len() <= 4);
    }
    assert_eq!(packets.len(), 4);
    for packet in &packets[1..] {
        assert_eq!(packet.destination, config.destination.unwrap());
        let sacn::packet::Payload::Data(frame) = sacn::Packet::parse(&packet.bytes).unwrap().payload else {
            panic!("termination data")
        };
        assert!(frame.stream_terminated);
    }
}

#[test]
fn stale_attempts_cannot_consume_or_rearm_new_authored_initialization() {
    let config = config(DmxProtocol::Sacn);
    let mut owner = OutputContinuity::default();
    let first = owner.claim(&config);
    first.finish_initial_look();
    owner.restart_from_authored();
    let second = owner.claim(&config);
    first.finish_initial_look();
    assert!(second.needs_initial_look());
    second.finish_initial_look();
    first.finish_initial_look();
    assert!(!second.needs_initial_look());
    owner.restart_from_authored();
    assert!(owner.claim(&config).needs_initial_look());
}

#[test]
fn final_native_submission_order_spans_different_stream_locks_without_blocking_actor_claims() {
    let config = config(DmxProtocol::Sacn);
    let mut owner = OutputContinuity::default();
    let first = owner.claim(&config);
    let now = Instant::now();
    let order = Arc::new(Mutex::new(Vec::new()));
    let (entered, ready) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::sync_channel(1);
    let first_order = order.clone();
    let old_worker = std::thread::spawn(move || {
        first
            .send(&DmxFrame::new(1, vec![1]).unwrap(), now, &mut |_, _| {
                entered.send(()).unwrap();
                let _ = wait.recv_timeout(Duration::from_secs(1));
                first_order.lock().unwrap().push(1);
                Ok(())
            })
            .unwrap();
    });
    ready.recv_timeout(Duration::from_secs(1)).unwrap();
    let mut other = config.clone();
    other.universe = 2;
    let started = Instant::now();
    let second = owner.claim(&other);
    let elapsed = started.elapsed();
    let (entered, ready) = mpsc::sync_channel(1);
    let (finished, done) = mpsc::sync_channel(1);
    let second_order = order.clone();
    let new_worker = std::thread::spawn(move || {
        entered.send(()).unwrap();
        second
            .send(&DmxFrame::new(2, vec![2]).unwrap(), now, &mut |_, _| {
                second_order.lock().unwrap().push(2);
                Ok(())
            })
            .unwrap();
        finished.send(()).unwrap();
    });
    ready.recv_timeout(Duration::from_secs(1)).unwrap();
    let blocked = done.recv_timeout(Duration::from_millis(20)).is_err();
    release.send(()).unwrap();
    old_worker.join().unwrap();
    new_worker.join().unwrap();
    assert!(elapsed < Duration::from_millis(100), "actor waited for native I/O");
    assert!(
        blocked,
        "replacement submitted before its predecessor completed native submission"
    );
    assert_eq!(*order.lock().unwrap(), vec![1, 2]);
}
