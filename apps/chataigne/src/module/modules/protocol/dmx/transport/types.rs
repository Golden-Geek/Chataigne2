use super::super::frame::{ARTNET_MAX_UNIVERSE, DmxFrame, SACN_MAX_UNIVERSE};
use crate::app::module::script_api::NativeCallbackPayload;
#[cfg(test)]
use golden_io::RetainedPayload;
use golden_io::RevisionGuard;
use std::{
    net::{IpAddr, SocketAddr},
    ops::Deref,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DmxProtocol {
    ArtNet,
    Sacn,
}
impl DmxProtocol {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::ArtNet => "Art-Net",
            Self::Sacn => "sACN",
        }
    }
    pub(crate) const fn default_port(self) -> u16 {
        match self {
            Self::ArtNet => 6454,
            Self::Sacn => sacn::proto::SACN_PORT,
        }
    }
    pub(crate) const fn maximum_universe(self) -> u16 {
        match self {
            Self::ArtNet => ARTNET_MAX_UNIVERSE,
            Self::Sacn => SACN_MAX_UNIVERSE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DmxTransportConfig {
    pub protocol: DmxProtocol,
    pub source_id: [u8; 16],
    pub bind_ip: IpAddr,
    pub listen_port: u16,
    pub receive_enabled: bool,
    pub universe: u16,
    pub destination: Option<SocketAddr>,
}

pub(super) enum ReceivePoll {
    Empty,
    Ignored,
    Invalid(&'static str),
    Frame(DmxFrame),
}

#[derive(Debug)]
pub(crate) struct ReceivedFrame {
    pub frame: DmxFrame,
    pub json: String,
    pub callback: NativeCallbackPayload,
    pub source: RevisionGuard,
}
impl Deref for ReceivedFrame {
    type Target = DmxFrame;
    fn deref(&self) -> &DmxFrame {
        &self.frame
    }
}

#[derive(Clone, Debug)]
pub(crate) enum DmxWorkerEvent {
    Ready,
    Warning(String),
    Error(String),
    #[cfg(test)]
    Frame(RetainedPayload<ReceivedFrame>),
}
