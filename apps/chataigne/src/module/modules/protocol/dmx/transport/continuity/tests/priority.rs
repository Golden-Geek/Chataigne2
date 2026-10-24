use super::*;

#[test]
fn recovery_uses_current_authored_priority_even_when_changed_during_opening() {
    let config = config(DmxProtocol::Sacn);
    let mut owner = OutputContinuity::default();
    owner.set_priority(100);
    let first = owner.claim(&config);
    let now = Instant::now();
    let mut packets = Vec::new();
    first
        .send(
            &DmxFrame::new(1, vec![33, 34]).unwrap(),
            now,
            &mut capture(&mut packets),
        )
        .unwrap();
    let failed = DmxFrame::with_metadata(1, 0, 150, vec![77, 78]).unwrap();
    assert!(
        first
            .send(&failed, now + Duration::from_millis(23), &mut |_, _| Err(
                "unconfirmed".into()
            ))
            .is_err()
    );
    first.suspend();
    let recovered = owner.claim(&config);
    // Authoring remains nonblocking while native socket opening is outstanding.
    owner.set_priority(0);
    recovered
        .poll(now + Duration::from_millis(46), &mut capture(&mut packets))
        .unwrap();
    assert_eq!(packets.len(), 2);
    let sacn::packet::Payload::Data(data) = sacn::Packet::parse(&packets[1].bytes).unwrap().payload else {
        panic!("data")
    };
    assert_eq!(data.priority, 0);
    assert_eq!(data.values, &[33, 34]);
    assert_eq!(data.sequence_number.get(), 2, "failed attempt consumed sequence one");
    assert_eq!(
        recovered
            .frame_with_priority(200, now + Duration::from_millis(69))
            .unwrap()
            .unwrap()
            .slots,
        vec![33, 34]
    );
}

#[test]
fn a_priority_control_never_creates_channels_or_consumes_authored_initialization() {
    for protocol in [DmxProtocol::ArtNet, DmxProtocol::Sacn] {
        let config = config(protocol);
        let mut owner = OutputContinuity::default();
        let session = owner.claim(&config);
        let now = Instant::now();
        assert!(session.needs_initial_look());
        assert!(session.frame_with_priority(200, now).unwrap().is_none());
        assert!(session.needs_initial_look());
        let mut packets = Vec::new();
        assert!(!session.poll(now, &mut capture(&mut packets)).unwrap());
        assert!(packets.is_empty());
        session
            .send(&DmxFrame::new(1, vec![9, 10]).unwrap(), now, &mut capture(&mut packets))
            .unwrap();
        let frame = session
            .frame_with_priority(200, now + Duration::from_millis(23))
            .unwrap()
            .unwrap();
        assert_eq!(frame.slots, vec![9, 10]);
        assert_eq!(frame.priority, 200);
        session
            .send(&frame, now + Duration::from_millis(23), &mut capture(&mut packets))
            .unwrap();
        assert_eq!(packets.len(), 2);
        assert_eq!(sequence(protocol, &packets[1]), sequence(protocol, &packets[0]) + 1);
    }
}
