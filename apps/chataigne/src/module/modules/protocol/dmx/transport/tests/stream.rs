use super::*;
use sacn::{Cid, Packet, packet::Payload};

fn data(socket: &UdpSocket, protocol: DmxProtocol) -> (u8, Vec<u8>, bool) {
    let mut bytes = [0; 2048];
    let (length, _) = socket.recv_from(&mut bytes).unwrap();
    decode(&bytes[..length], protocol)
}

fn decode(bytes: &[u8], protocol: DmxProtocol) -> (u8, Vec<u8>, bool) {
    match protocol {
        DmxProtocol::ArtNet => {
            let ArtCommand::Output(output) = ArtCommand::from_buffer(bytes).unwrap() else {
                panic!("expected ArtDMX");
            };
            (output.sequence, output.data.as_ref().to_vec(), false)
        }
        DmxProtocol::Sacn => {
            let packet = Packet::parse(bytes).unwrap();
            assert_eq!(packet.cid, Cid::from_bytes([77; 16]));
            let Payload::Data(data) = packet.payload else {
                panic!("unicast endpoint emitted non-data traffic");
            };
            (data.sequence_number.get(), data.values.to_vec(), data.stream_terminated)
        }
    }
}

#[test]
fn native_worker_clocks_fifo_blackout_and_refresh_then_terminates_on_the_unicast_route() {
    for protocol in [DmxProtocol::ArtNet, DmxProtocol::Sacn] {
        let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        receiver.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut transport = spawn(DmxTransportConfig {
            protocol,
            source_id: [77; 16],
            bind_ip: Ipv4Addr::LOCALHOST.into(),
            listen_port: protocol.default_port(),
            receive_enabled: false,
            universe: 1,
            destination: Some(receiver.local_addr().unwrap()),
        });
        let receipts: Vec<_> = [11, 22, 0]
            .into_iter()
            .map(|value| transport.send(DmxFrame::new(1, vec![value; 512]).unwrap()).unwrap())
            .collect();
        let mut last_sequence = None;
        for expected in [11, 22, 0] {
            let (sequence, levels, terminated) = data(&receiver, protocol);
            assert_eq!(levels, vec![expected; 512], "explicit commands must retain wire order");
            assert!(!terminated);
            if let Some(last) = last_sequence {
                assert_eq!(sequence, last + 1);
            }
            last_sequence = Some(sequence);
        }
        until(|| receipts.iter().all(|receipt| receipt.status() == SendStatus::Sent));
        // No further command is sent: only the worker's own stream deadline can emit this.
        let (sequence, levels, terminated) = data(&receiver, protocol);
        assert_eq!(sequence, last_sequence.unwrap() + 1);
        assert_eq!(levels, vec![0; 512]);
        assert!(!terminated);
        let stopped = Instant::now();
        transport.stop();
        assert!(
            stopped.elapsed() < Duration::from_millis(100),
            "the engine must not join termination"
        );
        if protocol == DmxProtocol::Sacn {
            let mut terminations = 0;
            while terminations < 3 {
                let (_, levels, terminated) = data(&receiver, protocol);
                assert_eq!(levels, vec![0; 512]);
                if terminated {
                    terminations += 1;
                }
                assert!(stopped.elapsed() < Duration::from_secs(1));
            }
        }
    }
}

#[test]
fn native_receive_reports_source_loss_once_holds_last_look_and_recovers() {
    let probe = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let listen_port = probe.local_addr().unwrap().port();
    drop(probe);
    let mut transport = spawn(DmxTransportConfig {
        protocol: DmxProtocol::Sacn,
        source_id: [77; 16],
        bind_ip: Ipv4Addr::LOCALHOST.into(),
        listen_port,
        receive_enabled: true,
        universe: 1,
        destination: Some((Ipv4Addr::LOCALHOST, listen_port).into()),
    });
    let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let packet = super::super::input::tests::sacn_packet(1, 0, 100, &[33, 44]);
    sender.send_to(&packet, (Ipv4Addr::LOCALHOST, listen_port)).unwrap();
    until(|| transport.has_frame());
    assert!(!transport.take_frame().unwrap().metadata.stale);
    let mut losses = 0;
    let started = Instant::now();
    loop {
        while let Some(status) = transport.take_status() {
            match status {
                DmxWorkerEvent::InputLost => losses += 1,
                other => panic!("unexpected source status: {other:?}"),
            }
        }
        if let Some(frame) = transport.take_frame() {
            assert!(frame.metadata.stale);
            assert_eq!(&frame.slots[..2], &[33, 44]);
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(4));
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(losses, 1);
    sender.send_to(&packet, (Ipv4Addr::LOCALHOST, listen_port)).unwrap();
    until(|| transport.has_frame());
    assert!(!transport.take_frame().unwrap().metadata.stale);
    let mut recoveries = 0;
    while let Some(status) = transport.take_status() {
        match status {
            DmxWorkerEvent::InputRecovered => recoveries += 1,
            other => panic!("unexpected source status: {other:?}"),
        }
    }
    assert_eq!(recoveries, 1);
    assert!(transport.is_running());
    transport.stop();
}

#[test]
fn superseded_native_shutdown_cannot_terminate_the_reconnected_sacn_source() {
    let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    receiver.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let config = DmxTransportConfig {
        protocol: DmxProtocol::Sacn,
        source_id: [77; 16],
        bind_ip: Ipv4Addr::LOCALHOST.into(),
        listen_port: sacn::proto::SACN_PORT,
        receive_enabled: false,
        universe: 1,
        destination: Some(receiver.local_addr().unwrap()),
    };
    let mut continuity = OutputContinuity::default();
    let first = continuity.claim(&config);
    let mut old = native::WorkerEndpoint::open(&config, first).unwrap();
    old.send(&DmxFrame::new(1, vec![45; 512]).unwrap()).unwrap();
    let (mut sequence, levels, terminated) = data(&receiver, DmxProtocol::Sacn);
    assert_eq!(levels, vec![45; 512]);
    assert!(!terminated);
    let second = continuity.claim(&config);
    let mut replacement = native::WorkerEndpoint::open(&config, second).unwrap();
    receiver.set_nonblocking(true).unwrap();
    replacement.service().unwrap();
    // Delayed cleanup starts only after the replacement has taken stream ownership.
    let stopped = Instant::now();
    old.shutdown(worker::StopReason::Cancelled);
    assert!(stopped.elapsed() < Duration::from_millis(100));
    for _ in 0..4 {
        let limit = Instant::now() + Duration::from_secs(2);
        let (next, levels, terminated) = loop {
            let mut bytes = [0; 2048];
            match receiver.recv_from(&mut bytes) {
                Ok((length, _)) => break decode(&bytes[..length], DmxProtocol::Sacn),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("loopback receive: {error}"),
            }
            assert!(Instant::now() < limit);
            replacement.service().unwrap();
            if let Some(deadline) = replacement.deadline() {
                thread::sleep(deadline.min(limit).saturating_duration_since(Instant::now()));
            }
        };
        assert_eq!(next, sequence.wrapping_add(1));
        assert_eq!(levels, vec![45; 512]);
        assert!(!terminated, "old cleanup must not remove the replacement source");
        sequence = next;
    }
    replacement.shutdown(worker::StopReason::Cancelled);
    receiver.set_nonblocking(false).unwrap();
    for _ in 0..3 {
        assert!(data(&receiver, DmxProtocol::Sacn).2);
    }
}
