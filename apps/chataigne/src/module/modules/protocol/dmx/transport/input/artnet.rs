use super::*;
use artnet_protocol::ArtCommand;
use std::net::IpAddr;

const LOSS: Duration = Duration::from_secs(10);
const SYNC_LOSS: Duration = Duration::from_secs(4);

struct Source {
    address: SocketAddr,
    physical: u8,
    seen: Duration,
    received_sequence: Option<u8>,
    sequence: u8,
    levels: Vec<u8>,
    pending: Option<(Vec<u8>, u8)>,
}

pub(super) struct Receiver {
    universe: u16,
    sources: [Option<Source>; 2],
    sync: Option<(IpAddr, Duration)>,
    recent_ip: Option<IpAddr>,
    last: Option<InputFrame>,
}

impl Receiver {
    pub(super) fn new(universe: u16) -> Self {
        Self {
            universe,
            sources: [None, None],
            sync: None,
            recent_ip: None,
            last: None,
        }
    }

    pub(super) fn handle(&mut self, now: Duration, from: SocketAddr, bytes: &[u8]) -> ReceivePoll {
        if bytes.len() > 1024 {
            return ReceivePoll::Invalid("Oversized Art-Net packet ignored");
        }
        let command = match ArtCommand::from_buffer(bytes) {
            Ok(command) => command,
            Err(_) => return ReceivePoll::Invalid("Malformed Art-Net packet ignored"),
        };
        if matches!(command, ArtCommand::Output(_) | ArtCommand::Sync)
            && u16::from_be_bytes([bytes[10], bytes[11]]) < 14
        {
            return ReceivePoll::Invalid("Unsupported Art-Net protocol version");
        }
        match command {
            ArtCommand::Output(output) => {
                if u16::from(output.port_address) + 1 != self.universe {
                    return ReceivePoll::Ignored;
                }
                if output.data.as_ref().len() > 512 {
                    return ReceivePoll::Invalid("ArtDMX data exceeds 512 channels");
                }
                let index = self.sources.iter().position(|source| {
                    source
                        .as_ref()
                        .is_some_and(|source| source.address.ip() == from.ip() && source.physical == output.physical)
                });
                let index = match index.or_else(|| self.sources.iter().position(Option::is_none)) {
                    Some(index) => index,
                    None => return ReceivePoll::Invalid("Art-Net receive source limit exceeded (2)"),
                };
                let source = self.sources[index].get_or_insert_with(|| Source {
                    address: from,
                    physical: output.physical,
                    seen: now,
                    received_sequence: None,
                    sequence: 0,
                    levels: Vec::new(),
                    pending: None,
                });
                if output.sequence != 0
                    && source.received_sequence.is_some_and(|old| {
                        let forward = (u16::from(output.sequence) + 255 - u16::from(old)) % 255;
                        forward == 0 || forward > 127
                    })
                {
                    return ReceivePoll::Ignored;
                }
                source.address = from;
                source.seen = now;
                source.received_sequence = (output.sequence != 0).then_some(output.sequence);
                self.recent_ip = Some(from.ip());
                let different_ips = self
                    .sources
                    .iter()
                    .flatten()
                    .any(|source| source.address.ip() != from.ip());
                if different_ips {
                    self.sync = None;
                    self.release();
                }
                let source = self.sources[index].as_mut().unwrap();
                if self.sync.is_some() {
                    source.pending = Some((output.data.as_ref().to_vec(), output.sequence));
                    ReceivePoll::Ignored
                } else {
                    source.levels = output.data.as_ref().to_vec();
                    source.sequence = output.sequence;
                    self.publish(false).map_or(ReceivePoll::Ignored, ReceivePoll::Frame)
                }
            }
            ArtCommand::Sync => {
                // The codec recognizes the opcode but supplies no ArtSync fields. Its
                // minimum header/version checks above cover the standard's 14-byte frame.
                if self.recent_ip != Some(from.ip())
                    || self
                        .sources
                        .iter()
                        .flatten()
                        .any(|source| source.address.ip() != from.ip())
                {
                    return ReceivePoll::Ignored;
                }
                let released = self.release();
                self.sync = Some((from.ip(), now));
                if released {
                    self.publish(true).map_or(ReceivePoll::Ignored, ReceivePoll::Frame)
                } else {
                    ReceivePoll::Ignored
                }
            }
            _ => ReceivePoll::Ignored,
        }
    }

    pub(super) fn poll(&mut self, now: Duration) -> Option<InputFrame> {
        let mut changed = false;
        if self.sync.is_some_and(|(_, seen)| now.saturating_sub(seen) >= SYNC_LOSS) {
            self.sync = None;
            changed |= self.release();
        }
        for source in &mut self.sources {
            if source
                .as_ref()
                .is_some_and(|source| now.saturating_sub(source.seen) >= LOSS)
            {
                *source = None;
                changed = true;
            }
        }
        if changed { self.publish(false) } else { None }
    }

    fn release(&mut self) -> bool {
        let mut changed = false;
        for source in self.sources.iter_mut().flatten() {
            if let Some((levels, sequence)) = source.pending.take() {
                source.levels = levels;
                source.sequence = sequence;
                changed = true;
            }
        }
        changed
    }

    fn publish(&mut self, synchronized: bool) -> Option<InputFrame> {
        let active: Vec<_> = self
            .sources
            .iter()
            .flatten()
            .filter(|source| !source.levels.is_empty())
            .collect();
        let mut slots = vec![0; active.iter().map(|source| source.levels.len()).max().unwrap_or(0)];
        for source in &active {
            for (merged, level) in slots.iter_mut().zip(&source.levels) {
                *merged = (*merged).max(*level);
            }
        }
        let stale = active.is_empty();
        if stale {
            slots.clone_from(&self.last.as_ref()?.slots);
        }
        let frame = InputFrame {
            universe: self.universe,
            slots,
            metadata: InputMetadata {
                sequence: if active.len() == 1 {
                    Some(active[0].sequence)
                } else {
                    None
                },
                priority: None,
                sources: active
                    .iter()
                    .map(|source| InputSource {
                        address: source.address,
                        cid: None,
                        physical: Some(source.physical),
                        priority: None,
                    })
                    .collect(),
                priorities: None,
                synchronized,
                stale,
            },
        };
        self.last = Some(frame.clone());
        Some(frame)
    }
}
