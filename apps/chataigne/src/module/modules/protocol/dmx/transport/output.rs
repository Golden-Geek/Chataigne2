use super::super::frame::DmxFrame;
use super::{DmxProtocol, DmxTransportConfig};
use artnet_protocol::{ArtCommand, Output};
use sacn::{Cid, Priority, Source, SourceConfig, Universe, UniverseConfig, source::Route};
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

#[cfg(test)]
mod tests;

sacn::static_storage! {
    struct SourceCapacity {
        rx_universes: 0,
        rx_sources_per_universe: 0,
        rx_sync_addresses: 0,
        tx_universes: 1,
        det_sources: 0,
        det_universes_per_source: 0,
    }
}

// Use a conservative full-universe DMX rate for both protocols. The sACN core's
// 22 ms quantum is an earliest poll hint, not our configured egress ceiling.
const DATA_INTERVAL: Duration = Duration::from_millis(23);
const ARTNET_REFRESH: Duration = Duration::from_millis(900);

enum Stream {
    ArtNet { last: Option<DmxFrame>, sequence: u8 },
    Sacn(Box<Source<SourceCapacity>>),
}

/// Owns one universe's retained look and protocol clock. The caller owns all I/O and time.
pub(super) struct ClockedOutput {
    stream: Stream,
    universe: u16,
    bind_ip: IpAddr,
    destination: Option<SocketAddr>,
    next_poll: Option<Duration>,
    command_at: Duration,
}

impl ClockedOutput {
    pub(super) fn new(config: &DmxTransportConfig) -> Self {
        let stream = match config.protocol {
            DmxProtocol::ArtNet => Stream::ArtNet {
                last: None,
                sequence: 1,
            },
            DmxProtocol::Sacn => Stream::Sacn(Box::new(Source::with_config(SourceConfig::new(
                Cid::from_bytes(config.source_id),
                "Chataigne2",
            )))),
        };
        Self {
            stream,
            universe: config.universe,
            bind_ip: config.bind_ip,
            destination: config.destination,
            next_poll: None,
            command_at: Duration::ZERO,
        }
    }

    pub(super) fn command_at(&self) -> Duration {
        self.command_at
    }
    pub(super) fn deadline(&self) -> Option<Duration> {
        self.next_poll.map(|deadline| deadline.max(self.command_at))
    }

    pub(super) fn send(
        &mut self,
        frame: &DmxFrame,
        now: Duration,
        emit: &mut impl FnMut(&[u8], SocketAddr) -> Result<(), String>,
    ) -> Result<(), String> {
        if now < self.command_at {
            return Err("DMX command dispatched before its protocol deadline".into());
        }
        match &mut self.stream {
            Stream::ArtNet { last, .. } => *last = Some(frame.clone()),
            Stream::Sacn(source) => {
                let universe = Universe::new(frame.universe).map_err(|error| error.to_string())?;
                let priority = Priority::new(frame.priority).map_err(|error| error.to_string())?;
                source
                    .add_universe(UniverseConfig::new(universe).with_priority(priority))
                    .map_err(|error| error.to_string())?;
                source.set_priority(universe, priority);
                source.update_levels(universe, &frame.slots);
                // Explicit sends, including identical frames and blackout, each need a real
                // submission before their receipt can complete. Never silently coalesce them.
                source.resend(universe);
            }
        }
        self.next_poll = Some(now);
        if !self.poll(now, emit)? {
            return Err("DMX protocol source did not emit the admitted frame".into());
        }
        Ok(())
    }

    pub(super) fn poll(
        &mut self,
        now: Duration,
        emit: &mut impl FnMut(&[u8], SocketAddr) -> Result<(), String>,
    ) -> Result<bool, String> {
        if self.deadline().is_none_or(|deadline| now < deadline) {
            return Ok(false);
        }
        let mut sent = false;
        match &mut self.stream {
            Stream::ArtNet { last, sequence } => {
                if let Some(frame) = last {
                    let destination = self.destination.ok_or("Art-Net destination is missing")?;
                    let packet = ArtCommand::Output(Output {
                        sequence: *sequence,
                        port_address: (frame.universe - 1)
                            .try_into()
                            .map_err(|error| format!("invalid Art-Net universe: {error}"))?,
                        data: frame.slots.clone().into(),
                        ..Output::default()
                    })
                    .write_to_buffer()
                    .map_err(|error| error.to_string())?;
                    emit(&packet, destination)?;
                    *sequence = if *sequence == 255 { 1 } else { *sequence + 1 };
                    self.command_at = now + DATA_INTERVAL;
                    self.next_poll = Some(now + ARTNET_REFRESH);
                    sent = true;
                }
            }
            Stream::Sacn(source) => {
                let mut poll = source.poll(sacn::time::Instant::from_epoch(now));
                self.next_poll = poll.deadline.map(|deadline| deadline.since_epoch());
                while let Some(transmission) = poll.next_transmission() {
                    let universe = match transmission.route {
                        Route::Universe(universe) | Route::Sync(universe) => universe.get(),
                        Route::Discovery => {
                            // Discovery is multicast-only. A unicast-only module must never
                            // advertise or terminate on a different destination implicitly.
                            if self.destination.is_some() {
                                continue;
                            }
                            sacn::proto::DISCOVERY_UNIVERSE
                        }
                        _ => return Err("Unsupported sACN transmission route".into()),
                    };
                    let destination = self.destination.unwrap_or_else(|| multicast(self.bind_ip, universe));
                    emit(transmission.data, destination)?;
                    if matches!(transmission.route, Route::Universe(_)) {
                        self.command_at = now + DATA_INTERVAL;
                        sent = true;
                    }
                }
            }
        }
        Ok(sent)
    }

    pub(super) fn terminate(&mut self) {
        if let Stream::Sacn(source) = &mut self.stream {
            source.remove_universe(Universe::new(self.universe).expect("validated DMX universe"));
            self.next_poll = Some(Duration::ZERO);
        } else {
            self.next_poll = None;
        }
    }
}

pub(super) fn multicast(bind_ip: IpAddr, universe: u16) -> SocketAddr {
    let ip = match bind_ip {
        IpAddr::V4(_) => IpAddr::V4(sacn::ipv4_multicast(universe)),
        IpAddr::V6(_) => IpAddr::V6(sacn::ipv6_multicast(universe)),
    };
    SocketAddr::new(ip, sacn::proto::SACN_PORT)
}
