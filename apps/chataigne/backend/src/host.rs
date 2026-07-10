use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr};

use golden_host::{HostConfig, HostConfigError, HostLaunchPlan, HostMode};
use golden_transport::{NetworkAccess, NetworkPolicy};
use thiserror::Error;

pub fn chataigne_host(mode: HostMode, open_lan: bool, port: u16) -> Result<HostLaunchPlan, ChataigneHostError> {
    let network = if open_lan {
        NetworkPolicy {
            access: NetworkAccess::OpenLan,
            bind_address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            tls_enabled: false,
            authentication_token: None,
            allowed_origins: BTreeSet::from(["http://localhost".into(), "http://chataigne.local".into()]),
            advertised_hosts: BTreeSet::from(["localhost".into(), "chataigne.local".into()]),
            maximum_clients: 64,
            maximum_payload_bytes: 4 * 1_048_576,
        }
    } else {
        NetworkPolicy {
            access: NetworkAccess::Loopback,
            bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            tls_enabled: false,
            authentication_token: None,
            allowed_origins: BTreeSet::new(),
            advertised_hosts: BTreeSet::new(),
            maximum_clients: 16,
            maximum_payload_bytes: 4 * 1_048_576,
        }
    };
    Ok(HostConfig {
        application: "Chataigne".into(),
        mode,
        network,
        advertise_mdns: open_lan,
    }
    .plan(port)?)
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ChataigneHostError {
    #[error(transparent)]
    InvalidConfiguration(#[from] HostConfigError),
}
