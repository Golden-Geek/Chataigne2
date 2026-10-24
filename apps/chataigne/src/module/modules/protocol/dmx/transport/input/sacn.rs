use super::*;
use ::sacn::{
    Cid, DmxMerger, NetintId, Packet, ReceiverConfig, Universe,
    merger::SourceId,
    packet::Payload,
    receiver::{MergedDataRef, MergedPacketOutcome, ReceiverPollEvent},
};

// Isolate synchronization by CID: the upstream merged receiver's sync table is
// address-only and does not reject old sync sequences. Reuse its complete source
// and sync state machines per CID, then its merger for the bounded combined look.
::sacn::static_storage! {
    struct SourceCapacity {
        rx_universes: 1,
        rx_sources_per_universe: 1,
        rx_sync_addresses: 8,
        tx_universes: 0,
        det_sources: 0,
        det_universes_per_source: 0,
    }
}
::sacn::static_storage! {
    struct MergeCapacity {
        rx_universes: 0,
        rx_sources_per_universe: 8,
        rx_sync_addresses: 0,
        tx_universes: 0,
        det_sources: 0,
        det_universes_per_source: 0,
    }
}

struct Source {
    cid: Cid,
    id: SourceId,
    core: ::sacn::Receiver<SourceCapacity>,
    info: Option<InputSource>,
    sync_sequences: [Option<(u16, u8, Duration)>; 8],
}

pub(super) struct Receiver {
    universe: Universe,
    sources: [Option<Box<Source>>; MAX_SOURCES],
    merger: DmxMerger<MergeCapacity>,
    last: Option<InputFrame>,
}

impl Receiver {
    pub(super) fn new(universe: u16) -> Self {
        Self {
            universe: Universe::new(universe).expect("validated universe"),
            sources: std::array::from_fn(|_| None),
            merger: DmxMerger::default(),
            last: None,
        }
    }

    pub(super) fn handle(&mut self, now: Duration, from: SocketAddr, bytes: &[u8]) -> ReceivePoll {
        if bytes.len() > ::sacn::packet::MAX_PACKET_SIZE {
            return ReceivePoll::Invalid("Oversized sACN packet ignored");
        }
        let packet = match Packet::parse(bytes) {
            Ok(packet) => packet,
            Err(_) => return ReceivePoll::Invalid("Malformed sACN packet ignored"),
        };
        match packet.payload {
            Payload::Data(data) => {
                if data.priority > 200
                    || Universe::new(data.universe).is_err()
                    || (data.sync_address != 0 && Universe::new(data.sync_address).is_err())
                {
                    return ReceivePoll::Invalid("Invalid sACN priority or universe");
                }
                if data.universe != self.universe.get() || data.preview || ![0, 0xdd].contains(&data.start_code) {
                    return ReceivePoll::Ignored;
                }
                if data.start_code == 0xdd && data.values.iter().any(|priority| *priority > 200) {
                    return ReceivePoll::Invalid("Invalid sACN per-address priority");
                }
            }
            Payload::Sync(_) => {}
            _ => return ReceivePoll::Ignored,
        }
        let index = self
            .sources
            .iter()
            .position(|source| source.as_ref().is_some_and(|source| source.cid == packet.cid));
        let index = match index {
            Some(index) => index,
            None => {
                if !matches!(packet.payload, Payload::Data(data) if !data.stream_terminated) {
                    return ReceivePoll::Ignored;
                }
                let Some(index) = self.sources.iter().position(Option::is_none) else {
                    return ReceivePoll::Invalid("sACN receive source limit exceeded (8)");
                };
                let id = self.merger.add_source().expect("eight matching source slots");
                self.sources[index] = Some(Box::new(Source::new(packet.cid, id, self.universe, now)));
                index
            }
        };
        let source = self.sources[index].as_mut().unwrap();
        let synchronized = if let Payload::Sync(sync) = packet.payload {
            if !source
                .core
                .sync_group_interest()
                .any(|universe| universe.get() == sync.sync_address)
            {
                return ReceivePoll::Ignored;
            }
            let sequence = sync.sequence_number.get();
            for entry in &mut source.sync_sequences {
                if entry.is_some_and(|(_, _, seen)| now.saturating_sub(seen) >= Duration::from_millis(2500)) {
                    *entry = None;
                }
            }
            let history = source
                .sync_sequences
                .iter()
                .position(|entry| entry.is_some_and(|(universe, _, _)| universe == sync.sync_address))
                .or_else(|| source.sync_sequences.iter().position(Option::is_none));
            let Some(history) = history else {
                return ReceivePoll::Invalid("sACN source synchronization history limit exceeded (8)");
            };
            if let Some((_, old, _)) = source.sync_sequences[history] {
                // E1.31 6.7.2 uses the signed difference, accepting a restart/wrap
                // at least twenty behind. The crate keeps this comparator private.
                let difference = i16::from(sequence) - i16::from(old);
                if difference <= 0 && difference > -20 {
                    return ReceivePoll::Ignored;
                }
            }
            source.sync_sequences[history] = Some((sync.sync_address, sequence, now));
            true
        } else {
            false
        };
        let changed =
            match source
                .core
                .handle_packet(::sacn::time::Instant::from_epoch(now), from, NetintId::UNKNOWN, &packet)
            {
                MergedPacketOutcome::Data { merged, .. } | MergedPacketOutcome::Passthrough { merged, .. } => {
                    merged.is_some_and(|data| update(&mut self.merger, source.id, &mut source.info, data))
                }
                MergedPacketOutcome::Sync(release) => release
                    .merged_frames()
                    .next()
                    .is_some_and(|data| update(&mut self.merger, source.id, &mut source.info, data)),
                MergedPacketOutcome::SyncLimitExceeded { .. } => {
                    return ReceivePoll::Invalid("sACN source synchronization history limit exceeded (8)");
                }
                _ => false,
            };
        if changed {
            self.publish(synchronized)
                .map_or(ReceivePoll::Ignored, ReceivePoll::Frame)
        } else {
            // A source's first data is accepted during its zero-length sampling
            // window. End that window after feeding the packet, so the core neither
            // loses the first NULL packet to its PAP handshake nor invents a retry.
            self.poll(now).map_or(ReceivePoll::Ignored, ReceivePoll::Frame)
        }
    }

    pub(super) fn poll(&mut self, now: Duration) -> Option<InputFrame> {
        let mut changed = false;
        for slot in &mut self.sources {
            let Some(source) = slot.as_mut() else {
                continue;
            };
            let mut retired = false;
            {
                let mut poll = source.core.poll(::sacn::time::Instant::from_epoch(now));
                while let Some(event) = poll.next_event() {
                    match event {
                        ReceiverPollEvent::SourcesLost { sources, .. } => {
                            retired |= sources.iter().any(|lost| lost.cid == source.cid);
                        }
                        ReceiverPollEvent::MergedDataChanged { universe } => {
                            if let Some(data) = poll.merged(universe) {
                                changed |= update(&mut self.merger, source.id, &mut source.info, data);
                            }
                        }
                        _ => {}
                    }
                }
            }
            if retired {
                self.merger.remove_source(source.id);
                *slot = None;
                changed = true;
            }
        }
        if changed { self.publish(false) } else { None }
    }

    fn publish(&mut self, synchronized: bool) -> Option<InputFrame> {
        let sources: Vec<_> = self
            .sources
            .iter()
            .flatten()
            .filter_map(|source| source.info.clone())
            .collect();
        let stale = sources.is_empty();
        let output = self.merger.output();
        let slots = if stale {
            self.last.as_ref()?.slots.clone()
        } else {
            output.levels().to_vec()
        };
        let frame = InputFrame {
            universe: self.universe.get(),
            slots,
            metadata: InputMetadata {
                sequence: None,
                priority: if sources.len() == 1 { sources[0].priority } else { None },
                sources,
                priorities: Some(output.priorities().to_vec()),
                synchronized,
                stale,
            },
        };
        self.last = Some(frame.clone());
        Some(frame)
    }

    pub(super) fn sync_groups(&self) -> Vec<u16> {
        let mut groups: Vec<_> = self
            .sources
            .iter()
            .flatten()
            .flat_map(|source| source.core.sync_group_interest().map(Universe::get))
            .collect();
        groups.sort_unstable();
        groups.dedup();
        groups
    }
}

impl Source {
    fn new(cid: Cid, id: SourceId, universe: Universe, now: Duration) -> Self {
        // The module already selects one interface. Do not delay ordinary live input
        // for interface discovery or a potential PAP stream; keep actual PAP handling.
        let config = ReceiverConfig::new()
            .with_sample_period(Duration::ZERO)
            .with_per_address_priority_wait_time(Duration::ZERO)
            .with_source_limit(1)
            .with_filter_preview(true);
        let mut core = ::sacn::Receiver::with_config(config);
        let now = ::sacn::time::Instant::from_epoch(now);
        core.listen(now, universe).expect("one fixed receive universe");
        Self {
            cid,
            id,
            core,
            info: None,
            sync_sequences: [None; 8],
        }
    }
}

fn update(
    merger: &mut DmxMerger<MergeCapacity>,
    id: SourceId,
    info: &mut Option<InputSource>,
    data: MergedDataRef<'_, SourceCapacity>,
) -> bool {
    let Some(source) = data.active_sources().next() else {
        return false;
    };
    *info = Some(InputSource {
        address: source.addr,
        cid: Some(uuid::Uuid::from_bytes(source.cid.into_bytes())),
        physical: None,
        priority: Some(source.universe_priority),
    });
    merger.set_deferred(true);
    merger.update_levels(id, data.levels());
    merger.update_per_address_priorities(id, data.priorities());
    merger.set_deferred(false);
    true
}
