use super::*;
use sacn::{Packet, packet::Payload};
use std::net::Ipv4Addr;

fn config(protocol: DmxProtocol) -> DmxTransportConfig {
    DmxTransportConfig {
        protocol,
        source_id: [42; 16],
        bind_ip: Ipv4Addr::LOCALHOST.into(),
        listen_port: protocol.default_port(),
        receive_enabled: false,
        universe: 1,
        destination: Some((Ipv4Addr::LOCALHOST, 45678).into()),
    }
}

#[derive(Debug)]
struct Sent {
    at: Duration,
    destination: SocketAddr,
    sequence: u8,
    levels: Vec<u8>,
    cid: Option<Cid>,
    terminated: bool,
}

fn capture(
    protocol: DmxProtocol,
    at: Duration,
    bytes: &[u8],
    destination: SocketAddr,
    sent: &mut Vec<Sent>,
) -> Result<(), String> {
    let (sequence, levels, cid, terminated) = match protocol {
        DmxProtocol::ArtNet => {
            let ArtCommand::Output(output) = ArtCommand::from_buffer(bytes).unwrap() else {
                panic!("expected ArtDMX");
            };
            assert_eq!(u16::from(output.port_address), 0);
            (output.sequence, output.data.as_ref().to_vec(), None, false)
        }
        DmxProtocol::Sacn => {
            let packet = Packet::parse(bytes).unwrap();
            let Payload::Data(data) = packet.payload else {
                panic!("unicast must never emit discovery");
            };
            assert_eq!(data.universe, 1);
            assert_eq!(data.start_code, 0);
            (
                data.sequence_number.get(),
                data.values.to_vec(),
                Some(packet.cid),
                data.stream_terminated,
            )
        }
    };
    sent.push(Sent {
        at,
        destination,
        sequence,
        levels,
        cid,
        terminated,
    });
    Ok(())
}

#[test]
fn static_looks_refresh_past_receiver_loss_without_new_commands_for_both_protocols() {
    for protocol in [DmxProtocol::ArtNet, DmxProtocol::Sacn] {
        let config = config(protocol);
        let mut stream = ClockedOutput::new(&config);
        let mut sent = Vec::new();
        let frame = DmxFrame::new(1, vec![7, 23, 99, 128]).unwrap();
        stream
            .send(&frame, Duration::ZERO, &mut |bytes, destination| {
                capture(protocol, Duration::ZERO, bytes, destination, &mut sent)
            })
            .unwrap();
        // Follow the production clock for eight seconds, beyond receiver loss timeouts.
        while let Some(at) = stream.deadline() {
            if at > Duration::from_secs(8) {
                break;
            }
            stream
                .poll(at, &mut |bytes, destination| {
                    capture(protocol, at, bytes, destination, &mut sent)
                })
                .unwrap();
        }
        assert!(sent.len() >= 9, "{protocol:?}: {sent:?}");
        assert!(sent.last().unwrap().at >= Duration::from_secs(7));
        for packet in &sent {
            assert_eq!(&packet.levels[..4], &frame.slots);
            assert_eq!(packet.destination, config.destination.unwrap());
            assert!(!packet.terminated);
            if protocol == DmxProtocol::Sacn {
                assert_eq!(packet.cid, Some(Cid::from_bytes(config.source_id)));
            }
        }
        for pair in sent.windows(2) {
            assert!(pair[1].at - pair[0].at <= Duration::from_millis(900));
            assert!(pair[1].at - pair[0].at >= Duration::from_millis(23));
            assert_eq!(pair[1].sequence, pair[0].sequence.wrapping_add(1));
        }
    }
}

#[test]
fn every_explicit_frame_including_identical_looks_and_blackout_emits_at_the_send_clock() {
    for protocol in [DmxProtocol::ArtNet, DmxProtocol::Sacn] {
        let mut stream = ClockedOutput::new(&config(protocol));
        let mut sent = Vec::new();
        for index in 0..260 {
            let at = stream.command_at();
            let levels = if index == 259 { vec![0; 512] } else { vec![77; 512] };
            let frame = DmxFrame::with_metadata(1, 123, 0, levels.clone()).unwrap();
            stream
                .send(&frame, at, &mut |bytes, destination| {
                    capture(protocol, at, bytes, destination, &mut sent)
                })
                .unwrap();
            assert_eq!(sent.len(), index + 1);
            assert_eq!(sent.last().unwrap().levels, levels);
        }
        for pair in sent.windows(2) {
            let expected = match protocol {
                DmxProtocol::ArtNet => {
                    if pair[0].sequence == 255 {
                        1
                    } else {
                        pair[0].sequence + 1
                    }
                }
                DmxProtocol::Sacn => pair[0].sequence.wrapping_add(1),
            };
            assert_eq!(pair[1].sequence, expected);
            assert!(pair[1].at - pair[0].at >= Duration::from_millis(23));
        }
        let mut submissions = 0;
        assert!(
            stream
                .send(&DmxFrame::new(1, vec![1]).unwrap(), Duration::ZERO, &mut |_, _| {
                    submissions += 1;
                    Ok(())
                })
                .is_err()
        );
        assert_eq!(submissions, 0);
    }
}

#[test]
fn stable_cid_survives_endpoint_recreation_but_no_previous_look_is_replayed() {
    let config = config(DmxProtocol::Sacn);
    for _ in 0..2 {
        let mut stream = ClockedOutput::new(&config);
        let mut sent = Vec::new();
        assert!(
            !stream
                .poll(Duration::from_secs(10), &mut |_, _| panic!(
                    "reopening must not replay a command"
                ))
                .unwrap()
        );
        stream
            .send(
                &DmxFrame::new(1, vec![11]).unwrap(),
                Duration::from_secs(10),
                &mut |bytes, destination| {
                    capture(
                        DmxProtocol::Sacn,
                        Duration::from_secs(10),
                        bytes,
                        destination,
                        &mut sent,
                    )
                },
            )
            .unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].cid, Some(Cid::from_bytes(config.source_id)));
    }
}

#[test]
fn sacn_termination_uses_three_spaced_packets_on_the_original_unicast_route() {
    let config = config(DmxProtocol::Sacn);
    let mut stream = ClockedOutput::new(&config);
    let mut sent = Vec::new();
    let mut now = Duration::ZERO;
    stream
        .send(
            &DmxFrame::new(1, vec![19, 20]).unwrap(),
            now,
            &mut |bytes, destination| capture(DmxProtocol::Sacn, now, bytes, destination, &mut sent),
        )
        .unwrap();
    stream.terminate();
    let mut polls = 0;
    while let Some(at) = stream.deadline() {
        now = at.max(now);
        stream
            .poll(now, &mut |bytes, destination| {
                capture(DmxProtocol::Sacn, now, bytes, destination, &mut sent)
            })
            .unwrap();
        polls += 1;
        assert!(polls <= 5, "termination must finish without discovery timer churn");
    }
    let termination = &sent[1..];
    assert_eq!(termination.len(), 3);
    assert!(termination[0].at >= Duration::from_millis(23));
    for packet in termination {
        assert!(packet.terminated);
        assert_eq!(packet.destination, config.destination.unwrap());
        assert_eq!(packet.cid, Some(Cid::from_bytes(config.source_id)));
        assert_eq!(packet.levels, [19, 20]);
    }
    for pair in termination.windows(2) {
        assert_eq!(pair[1].at - pair[0].at, Duration::from_millis(23));
    }
    assert!(
        !stream
            .poll(Duration::from_secs(20), &mut |_, _| panic!("terminated stream resumed"))
            .unwrap()
    );
}

#[test]
fn submission_errors_propagate_instead_of_acknowledging_delivery() {
    for protocol in [DmxProtocol::ArtNet, DmxProtocol::Sacn] {
        let mut stream = ClockedOutput::new(&config(protocol));
        let error = stream
            .send(&DmxFrame::new(1, vec![1]).unwrap(), Duration::ZERO, &mut |_, _| {
                Err("injected submission failure".into())
            })
            .unwrap_err();
        assert_eq!(error, "injected submission failure");
    }
}

#[test]
fn the_source_timer_respects_the_data_clock_without_rearming_a_past_deadline() {
    let mut stream = ClockedOutput::new(&config(DmxProtocol::Sacn));
    stream
        .send(&DmxFrame::new(1, vec![1]).unwrap(), Duration::ZERO, &mut |_, _| Ok(()))
        .unwrap();
    assert_eq!(stream.deadline(), Some(Duration::from_millis(23)));
    assert!(
        !stream
            .poll(Duration::from_millis(22), &mut |_, _| panic!("early source burst"))
            .unwrap()
    );
    assert_eq!(stream.deadline(), Some(Duration::from_millis(23)));
    let mut emitted = 0;
    assert!(
        stream
            .poll(Duration::from_millis(23), &mut |_, _| {
                emitted += 1;
                Ok(())
            })
            .unwrap()
    );
    assert_eq!(emitted, 1);
    assert_eq!(stream.deadline(), Some(Duration::from_millis(46)));
}
