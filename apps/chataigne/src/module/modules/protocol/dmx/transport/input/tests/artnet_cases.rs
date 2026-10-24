use super::*;

fn sync() -> Vec<u8> {
    // artnet_protocol 0.4.4's unit Sync variant serializes only the opcode/header.
    // Append the standard protocol version and two auxiliary bytes for a real packet.
    let mut packet = ArtCommand::Sync.write_to_buffer().unwrap();
    packet.extend_from_slice(&[0, 14, 0, 0]);
    packet
}

#[test]
fn artnet_merges_two_physical_sources_and_bounds_additional_sources() {
    let mut receiver = Receiver::new(&config(DmxProtocol::ArtNet));
    let now = Duration::ZERO;
    frame(receiver.handle(now, address(1), &artnet_packet(1, 1, 0, &[10, 200])));
    let merged = frame(receiver.handle(now, address(1), &artnet_packet(1, 1, 1, &[200, 10])));
    assert_eq!(merged.slots, [200, 200]);
    assert_eq!(merged.metadata.sources.len(), 2);
    assert_eq!(merged.metadata.sequence, None);
    assert!(matches!(
        receiver.handle(now, address(2), &artnet_packet(1, 1, 0, &[255, 255])),
        ReceivePoll::Invalid(_)
    ));
    let fresh = frame(receiver.handle(now, address(1), &artnet_packet(1, 2, 0, &[11, 201])));
    assert_eq!(fresh.slots, [200, 201]);
}

#[test]
fn artnet_sequences_reject_duplicates_and_old_packets_wrap_and_allow_disabled_ordering() {
    let mut receiver = Receiver::new(&config(DmxProtocol::ArtNet));
    let now = Duration::ZERO;
    frame(receiver.handle(now, address(1), &artnet_packet(1, 254, 0, &[1, 1])));
    assert!(matches!(
        receiver.handle(now, address(1), &artnet_packet(1, 254, 0, &[99, 99])),
        ReceivePoll::Ignored
    ));
    assert!(matches!(
        receiver.handle(now, address(1), &artnet_packet(1, 253, 0, &[99, 99])),
        ReceivePoll::Ignored
    ));
    frame(receiver.handle(now, address(1), &artnet_packet(1, 255, 0, &[2, 2])));
    assert_eq!(
        frame(receiver.handle(now, address(1), &artnet_packet(1, 1, 0, &[3, 3]))).slots,
        [3, 3]
    );
    assert_eq!(
        frame(receiver.handle(now, address(1), &artnet_packet(1, 0, 0, &[4, 4]))).slots,
        [4, 4]
    );
    assert_eq!(
        frame(receiver.handle(now, address(1), &artnet_packet(1, 0, 0, &[5, 5]))).slots,
        [5, 5]
    );
}

#[test]
fn artnet_sync_releases_only_the_matching_controller_and_times_out_to_live() {
    let mut receiver = Receiver::new(&config(DmxProtocol::ArtNet));
    frame(receiver.handle(Duration::ZERO, address(1), &artnet_packet(1, 1, 0, &[1, 1])));
    assert!(matches!(
        receiver.handle(Duration::ZERO, address(2), &sync()),
        ReceivePoll::Ignored
    ));
    assert!(matches!(
        receiver.handle(Duration::ZERO, address(1), &sync()),
        ReceivePoll::Ignored
    ));
    assert!(matches!(
        receiver.handle(Duration::from_secs(1), address(1), &artnet_packet(1, 2, 0, &[2, 2])),
        ReceivePoll::Ignored
    ));
    let released = frame(receiver.handle(Duration::from_secs(1), address(1), &sync()));
    assert_eq!(released.slots, [2, 2]);
    assert!(released.metadata.synchronized);
    assert!(matches!(
        receiver.handle(Duration::from_secs(2), address(1), &artnet_packet(1, 3, 0, &[3, 3])),
        ReceivePoll::Ignored
    ));
    assert!(receiver.poll(Duration::from_millis(4999)).is_none());
    let released = receiver.poll(Duration::from_secs(5)).unwrap();
    assert_eq!(released.slots, [3, 3]);
    assert!(!released.metadata.synchronized);
    assert_eq!(
        frame(receiver.handle(Duration::from_secs(5), address(1), &artnet_packet(1, 4, 0, &[4, 4]))).slots,
        [4, 4]
    );
}

#[test]
fn artnet_ignores_sync_during_a_merge_across_different_controller_ips() {
    let mut receiver = Receiver::new(&config(DmxProtocol::ArtNet));
    frame(receiver.handle(Duration::ZERO, address(1), &artnet_packet(1, 1, 0, &[1, 2])));
    receiver.handle(Duration::ZERO, address(1), &sync());
    receiver.handle(Duration::ZERO, address(1), &artnet_packet(1, 2, 0, &[3, 4]));
    let merged = frame(receiver.handle(Duration::ZERO, address(2), &artnet_packet(1, 1, 0, &[5, 1])));
    assert_eq!(merged.slots, [5, 4]);
    assert!(matches!(
        receiver.handle(Duration::ZERO, address(2), &sync()),
        ReceivePoll::Ignored
    ));
    assert_eq!(
        frame(receiver.handle(Duration::ZERO, address(1), &artnet_packet(1, 3, 0, &[6, 2]))).slots,
        [6, 2]
    );
}

#[test]
fn artnet_source_loss_holds_the_last_merged_look_and_recovers_capacity() {
    let mut receiver = Receiver::new(&config(DmxProtocol::ArtNet));
    frame(receiver.handle(Duration::ZERO, address(1), &artnet_packet(1, 1, 0, &[10, 100])));
    frame(receiver.handle(Duration::from_secs(1), address(2), &artnet_packet(1, 1, 0, &[100, 10])));
    let remaining = receiver.poll(Duration::from_secs(10)).unwrap();
    assert_eq!(remaining.slots, [100, 10]);
    assert_eq!(remaining.metadata.sources.len(), 1);
    let held = receiver.poll(Duration::from_secs(11)).unwrap();
    assert_eq!(held.slots, [100, 10]);
    assert!(held.metadata.stale);
    assert!(held.metadata.sources.is_empty());
    assert!(receiver.poll(Duration::from_secs(100)).is_none());
    let recovered = frame(receiver.handle(Duration::from_secs(100), address(3), &artnet_packet(1, 1, 0, &[7, 8])));
    assert_eq!(recovered.slots, [7, 8]);
    assert!(!recovered.metadata.stale);
}

#[test]
fn artnet_bad_versions_do_not_enter_sync_or_mutate_a_live_look() {
    let mut receiver = Receiver::new(&config(DmxProtocol::ArtNet));
    frame(receiver.handle(Duration::ZERO, address(1), &artnet_packet(1, 1, 0, &[1, 2])));
    let mut old = sync();
    old[11] = 13;
    assert!(matches!(
        receiver.handle(Duration::ZERO, address(1), &old),
        ReceivePoll::Invalid(_)
    ));
    assert_eq!(
        frame(receiver.handle(Duration::ZERO, address(1), &artnet_packet(1, 2, 0, &[3, 4]))).slots,
        [3, 4]
    );
}
