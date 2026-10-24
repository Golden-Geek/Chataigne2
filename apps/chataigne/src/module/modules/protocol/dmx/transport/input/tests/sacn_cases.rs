use super::*;
use ::sacn::{Packet, packet::Payload};

fn edited(source: u8, sequence: u8, edit: impl FnOnce(&mut ::sacn::packet::DataPacket<'_>)) -> Vec<u8> {
    let bytes = sacn_packet(source, sequence, 100, &[11, 22]);
    let mut packet = Packet::parse(&bytes).unwrap();
    let Payload::Data(ref mut data) = packet.payload else {
        panic!("expected data fixture");
    };
    edit(data);
    packet.to_vec().unwrap()
}

fn sync(source: u8, sequence: u8, universe: u16) -> Vec<u8> {
    let mut source = ::sacn::Source::new(::sacn::SourceConfig::new(
        ::sacn::Cid::from_bytes([source; 16]),
        "sync fixture",
    ));
    let data_universe = ::sacn::Universe::new(1).unwrap();
    source
        .add_universe(::sacn::UniverseConfig::new(data_universe).synchronized_on(
            ::sacn::Universe::new(universe).unwrap(),
            ::sacn::source::OnSyncLoss::RevertToLive,
        ))
        .unwrap();
    source.update_levels(data_universe, &[1, 2]);
    {
        let mut first = source.poll(::sacn::time::Instant::EPOCH);
        while first.next_transmission().is_some() {}
    }
    let mut poll = source.poll(::sacn::time::Instant::from_epoch(Duration::from_millis(5)));
    while let Some(transmission) = poll.next_transmission() {
        if !matches!(transmission.route, ::sacn::source::Route::Sync(_)) {
            continue;
        }
        let mut packet = Packet::parse(transmission.data).unwrap();
        let Payload::Sync(ref mut data) = packet.payload else {
            panic!("expected sync packet");
        };
        data.sequence_number = ::sacn::SequenceNumber::new(sequence);
        return packet.to_vec().unwrap();
    }
    panic!("source did not emit a sync packet");
}

#[test]
fn sacn_priority_and_equal_priority_htp_merge_preserve_source_identity() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    let first = frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 0, 0, &[10, 200])));
    assert_eq!(&first.slots[..2], &[10, 200]);
    assert_eq!(first.metadata.priority, Some(0));
    let high = frame(receiver.handle(Duration::ZERO, address(2), &sacn_packet(2, 0, 100, &[20, 30])));
    assert_eq!(&high.slots[..2], &[20, 30]);
    assert_eq!(high.metadata.sources.len(), 2);
    assert_eq!(high.metadata.sequence, None);
    assert_eq!(high.metadata.priority, None);
    let merged = frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 1, 100, &[10, 200])));
    assert_eq!(&merged.slots[..2], &[20, 200]);
    assert_eq!(&merged.metadata.priorities.as_ref().unwrap()[..2], &[100, 100]);
    assert!(
        merged
            .metadata
            .sources
            .iter()
            .any(|source| source.cid == Some(uuid::Uuid::from_bytes([1; 16])))
    );
}

#[test]
fn sacn_sequences_ignore_duplicates_and_old_data_then_wrap_through_zero() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 254, 100, &[1, 2])));
    assert!(matches!(
        receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 254, 100, &[99, 99])),
        ReceivePoll::Ignored
    ));
    assert!(matches!(
        receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 253, 100, &[99, 99])),
        ReceivePoll::Ignored
    ));
    frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 255, 100, &[2, 3])));
    assert_eq!(
        &frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 0, 100, &[3, 4]))).slots[..2],
        &[3, 4]
    );
}

#[test]
fn sacn_per_address_priorities_select_each_slot_and_zero_relinquishes_control() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 0, 100, &[11, 22])));
    frame(receiver.handle(Duration::ZERO, address(2), &sacn_packet(2, 0, 100, &[30, 10])));
    let pap_bytes = sacn_packet(1, 1, 100, &[150, 0]);
    let mut pap = Packet::parse(&pap_bytes).unwrap();
    if let Payload::Data(ref mut data) = pap.payload {
        data.start_code = 0xdd;
    }
    let merged = frame(receiver.handle(Duration::ZERO, address(1), &pap.to_vec().unwrap()));
    assert_eq!(&merged.slots[..2], &[11, 10]);
    assert_eq!(&merged.metadata.priorities.as_ref().unwrap()[..2], &[150, 100]);
}

#[test]
fn sacn_sync_requires_the_source_and_releases_held_frames_on_valid_sequence() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    frame(receiver.handle(Duration::ZERO, address(1), &edited(1, 0, |data| data.sync_address = 7)));
    receiver.handle(Duration::ZERO, address(1), &sync(1, 0, 7));
    assert!(matches!(
        receiver.handle(Duration::ZERO, address(1), &edited(1, 1, |data| data.sync_address = 7)),
        ReceivePoll::Ignored
    ));
    assert!(matches!(
        receiver.handle(Duration::ZERO, address(2), &sync(2, 1, 7)),
        ReceivePoll::Ignored
    ));
    let released = frame(receiver.handle(Duration::ZERO, address(1), &sync(1, 1, 7)));
    assert_eq!(&released.slots[..2], &[11, 22]);
    assert!(released.metadata.synchronized);
    assert_eq!(receiver.sync_groups(), [7]);
}

#[test]
fn sacn_loss_and_termination_recompute_then_hold_last_look_and_recover_source_capacity() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 0, 100, &[10, 200])));
    frame(receiver.handle(Duration::from_secs(1), address(2), &sacn_packet(2, 0, 200, &[20, 30])));
    let remaining = frame(receiver.handle(
        Duration::from_secs(1),
        address(2),
        &edited(2, 1, |data| data.stream_terminated = true),
    ));
    assert_eq!(&remaining.slots[..2], &[10, 200]);
    assert!(!remaining.metadata.stale);
    let held = receiver.poll(Duration::from_secs(4)).unwrap();
    assert_eq!(&held.slots[..2], &[10, 200]);
    assert!(held.metadata.stale);
    assert!(held.metadata.sources.is_empty());
    assert!(receiver.poll(Duration::from_secs(100)).is_none());
    let recovered = frame(receiver.handle(Duration::from_secs(100), address(3), &sacn_packet(3, 0, 100, &[7, 8])));
    assert!(!recovered.metadata.stale);
    assert_eq!(&recovered.slots[..2], &[7, 8]);
}

#[test]
fn sacn_source_overload_is_bounded_and_clears_after_sources_expire() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    for source in 1..=8 {
        frame(receiver.handle(
            Duration::ZERO,
            address(source),
            &sacn_packet(source, 0, 100, &[source, 0]),
        ));
    }
    assert!(matches!(
        receiver.handle(Duration::ZERO, address(9), &sacn_packet(9, 0, 100, &[255, 0])),
        ReceivePoll::Invalid(_)
    ));
    receiver.poll(Duration::from_secs(4)).unwrap();
    let recovered = frame(receiver.handle(Duration::from_secs(4), address(9), &sacn_packet(9, 0, 100, &[9, 0])));
    assert_eq!(recovered.metadata.sources.len(), 1);
    assert_eq!(&recovered.slots[..2], &[9, 0]);
}

#[test]
fn sacn_preview_and_unrelated_sync_or_start_codes_do_not_change_a_live_frame() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 0, 100, &[1, 2])));
    for bytes in [
        edited(1, 1, |data| data.preview = true),
        edited(1, 1, |data| data.start_code = 0xab),
        sync(1, 0, 99),
    ] {
        assert!(matches!(
            receiver.handle(Duration::ZERO, address(1), &bytes),
            ReceivePoll::Ignored
        ));
    }
    assert_eq!(
        &frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 1, 100, &[3, 4]))).slots[..2],
        &[3, 4]
    );
}

#[test]
fn sacn_sync_sequence_histories_are_independent_across_addresses_and_reject_old_releases() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    frame(receiver.handle(Duration::ZERO, address(1), &edited(1, 0, |data| data.sync_address = 7)));
    receiver.handle(Duration::ZERO, address(1), &sync(1, 10, 7));
    receiver.handle(
        Duration::ZERO,
        address(1),
        &edited(1, 1, |data| {
            data.sync_address = 7;
            data.values = &[31, 41];
        }),
    );
    for sequence in [9, 10] {
        assert!(matches!(
            receiver.handle(Duration::ZERO, address(1), &sync(1, sequence, 7)),
            ReceivePoll::Ignored
        ));
    }
    assert_eq!(
        &frame(receiver.handle(Duration::ZERO, address(1), &sync(1, 11, 7))).slots[..2],
        &[31, 41]
    );
    // Force_Synchronization=0 keeps the old coherent look held across an address
    // change; the first sync on the new address arms it, and the next releases it.
    assert!(matches!(
        receiver.handle(
            Duration::ZERO,
            address(1),
            &edited(1, 2, |data| {
                data.sync_address = 8;
                data.values = &[51, 61];
            }),
        ),
        ReceivePoll::Ignored
    ));
    receiver.handle(Duration::ZERO, address(1), &sync(1, 200, 8));
    receiver.handle(
        Duration::ZERO,
        address(1),
        &edited(1, 3, |data| {
            data.sync_address = 8;
            data.values = &[71, 81];
        }),
    );
    frame(receiver.handle(Duration::ZERO, address(1), &sync(1, 201, 8)));
    receiver.handle(
        Duration::ZERO,
        address(1),
        &edited(1, 4, |data| {
            data.sync_address = 7;
            data.values = &[91, 101];
        }),
    );
    assert!(matches!(
        receiver.handle(Duration::ZERO, address(1), &sync(1, 11, 7)),
        ReceivePoll::Ignored
    ));
    assert_eq!(
        &frame(receiver.handle(Duration::ZERO, address(1), &sync(1, 12, 7))).slots[..2],
        &[91, 101]
    );
}

#[test]
fn sacn_two_cids_on_the_same_sync_address_release_only_their_own_pending_data() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    for source in [1, 2] {
        frame(receiver.handle(
            Duration::ZERO,
            address(source),
            &edited(source, 0, |data| {
                data.sync_address = 7;
                data.values = &[1, 1];
            }),
        ));
        receiver.handle(Duration::ZERO, address(source), &sync(source, 0, 7));
    }
    receiver.handle(
        Duration::ZERO,
        address(1),
        &edited(1, 1, |data| {
            data.sync_address = 7;
            data.values = &[10, 1];
        }),
    );
    receiver.handle(
        Duration::ZERO,
        address(2),
        &edited(2, 1, |data| {
            data.sync_address = 7;
            data.values = &[1, 20];
        }),
    );
    assert_eq!(
        &frame(receiver.handle(Duration::ZERO, address(1), &sync(1, 1, 7))).slots[..2],
        &[10, 1]
    );
    assert_eq!(
        &frame(receiver.handle(Duration::ZERO, address(2), &sync(2, 1, 7))).slots[..2],
        &[10, 20]
    );
}

#[test]
fn sacn_sync_timeout_obeys_hold_or_live_policy_and_recovers() {
    for revert in [false, true] {
        let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
        frame(receiver.handle(
            Duration::ZERO,
            address(1),
            &edited(1, 0, |data| {
                data.sync_address = 7;
                data.force_sync = revert;
            }),
        ));
        receiver.handle(Duration::ZERO, address(1), &sync(1, 0, 7));
        receiver.handle(
            Duration::from_millis(100),
            address(1),
            &edited(1, 1, |data| {
                data.sync_address = 7;
                data.force_sync = revert;
                data.values = &[21, 32];
            }),
        );
        frame(receiver.handle(Duration::from_millis(200), address(1), &sync(1, 1, 7)));
        receiver.handle(
            Duration::from_millis(1000),
            address(1),
            &edited(1, 2, |data| {
                data.sync_address = 7;
                data.force_sync = revert;
                data.values = &[31, 42];
            }),
        );
        let released = receiver.poll(Duration::from_millis(2701));
        if revert {
            assert_eq!(&released.unwrap().slots[..2], &[31, 42]);
        } else {
            assert!(released.is_none());
        }
        let next = receiver.handle(
            Duration::from_millis(2800),
            address(1),
            &edited(1, 3, |data| {
                data.sync_address = 7;
                data.force_sync = revert;
                data.values = &[41, 52];
            }),
        );
        if revert {
            assert_eq!(&frame(next).slots[..2], &[41, 52]);
        } else {
            assert!(matches!(next, ReceivePoll::Ignored));
        }
        receiver.handle(Duration::from_millis(2810), address(1), &sync(1, 2, 7));
        receiver.handle(
            Duration::from_millis(2820),
            address(1),
            &edited(1, 4, |data| {
                data.sync_address = 7;
                data.force_sync = revert;
                data.values = &[51, 62];
            }),
        );
        assert_eq!(
            &frame(receiver.handle(Duration::from_millis(2830), address(1), &sync(1, 3, 7))).slots[..2],
            &[51, 62]
        );
    }
}

#[test]
fn sacn_expired_pap_falls_back_to_universe_priority_on_live_level_input() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 0, 100, &[11, 22])));
    frame(receiver.handle(Duration::ZERO, address(2), &sacn_packet(2, 0, 100, &[30, 10])));
    let bytes = sacn_packet(1, 1, 100, &[150, 0]);
    let mut packet = Packet::parse(&bytes).unwrap();
    if let Payload::Data(ref mut data) = packet.payload {
        data.start_code = 0xdd;
    }
    frame(receiver.handle(Duration::ZERO, address(1), &packet.to_vec().unwrap()));
    frame(receiver.handle(Duration::from_secs(2), address(1), &sacn_packet(1, 2, 100, &[11, 22])));
    frame(receiver.handle(Duration::from_secs(2), address(2), &sacn_packet(2, 1, 100, &[30, 10])));
    let fallback = frame(receiver.handle(
        Duration::from_millis(2600),
        address(1),
        &sacn_packet(1, 3, 100, &[11, 22]),
    ));
    assert_eq!(&fallback.slots[..2], &[30, 22]);
    assert_eq!(&fallback.metadata.priorities.as_ref().unwrap()[..2], &[100, 100]);
}

#[test]
fn invalid_pap_does_not_consume_sequence_or_source_admission() {
    let mut receiver = Receiver::new(&config(DmxProtocol::Sacn));
    frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 0, 100, &[10, 20])));
    for source in [1, 9] {
        let bytes = sacn_packet(source, 1, 100, &[201, 0]);
        let mut packet = Packet::parse(&bytes).unwrap();
        if let Payload::Data(ref mut data) = packet.payload {
            data.start_code = 0xdd;
        }
        assert!(matches!(
            receiver.handle(Duration::ZERO, address(source), &packet.to_vec().unwrap()),
            ReceivePoll::Invalid(_)
        ));
    }
    for source in 2..=8 {
        frame(receiver.handle(Duration::ZERO, address(source), &sacn_packet(source, 0, 100, &[0, 0])));
    }
    let valid = frame(receiver.handle(Duration::ZERO, address(1), &sacn_packet(1, 1, 100, &[11, 21])));
    assert_eq!(&valid.slots[..2], &[11, 21]);
    assert_eq!(valid.metadata.sources.len(), 8);
}
