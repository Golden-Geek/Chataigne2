use super::{DmxProtocol, DmxTransportConfig, ReceivePoll};
use serde::Serialize;
use std::{net::SocketAddr, time::Duration};

mod artnet;
mod sacn;
#[cfg(test)]
pub(super) mod tests;

pub(super) const MAX_SOURCES: usize = 8;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct InputSource {
    pub address: SocketAddr,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<uuid::Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physical: Option<u8>,
    pub priority: Option<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct InputMetadata {
    pub sequence: Option<u8>,
    pub priority: Option<u8>,
    pub sources: Vec<InputSource>,
    pub priorities: Option<Vec<u8>>,
    pub synchronized: bool,
    pub stale: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct InputFrame {
    pub universe: u16,
    pub slots: Vec<u8>,
    pub metadata: InputMetadata,
}

pub(super) struct Receiver(ProtocolReceiver);

enum ProtocolReceiver {
    ArtNet(Box<artnet::Receiver>),
    Sacn(Box<sacn::Receiver>),
}

impl Receiver {
    pub(super) fn new(config: &DmxTransportConfig) -> Self {
        Self(match config.protocol {
            DmxProtocol::ArtNet => ProtocolReceiver::ArtNet(Box::new(artnet::Receiver::new(config.universe))),
            DmxProtocol::Sacn => ProtocolReceiver::Sacn(Box::new(sacn::Receiver::new(config.universe))),
        })
    }

    pub(super) fn handle(&mut self, now: Duration, from: SocketAddr, bytes: &[u8]) -> ReceivePoll {
        match &mut self.0 {
            ProtocolReceiver::ArtNet(receiver) => receiver.handle(now, from, bytes),
            ProtocolReceiver::Sacn(receiver) => receiver.handle(now, from, bytes),
        }
    }

    pub(super) fn poll(&mut self, now: Duration) -> Option<InputFrame> {
        match &mut self.0 {
            ProtocolReceiver::ArtNet(receiver) => receiver.poll(now),
            ProtocolReceiver::Sacn(receiver) => receiver.poll(now),
        }
    }

    pub(super) fn sync_groups(&self) -> Vec<u16> {
        match &self.0 {
            ProtocolReceiver::ArtNet(_) => Vec::new(),
            ProtocolReceiver::Sacn(receiver) => receiver.sync_groups(),
        }
    }
}
