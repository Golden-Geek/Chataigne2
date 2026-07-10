use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr};

use golden_transport::NetworkAccess;

use super::*;

#[test]
fn headless_and_desktop_use_the_same_public_endpoint_plan() {
    let network = NetworkPolicy {
        access: NetworkAccess::OpenLan,
        bind_address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        tls_enabled: false,
        authentication_token: None,
        allowed_origins: BTreeSet::from(["http://studio.local".into()]),
        advertised_hosts: BTreeSet::from(["studio.local".into()]),
        maximum_clients: 64,
        maximum_payload_bytes: 1_048_576,
    };
    let headless = HostConfig {
        application: "Chataigne".into(),
        mode: HostMode::Headless,
        network: network.clone(),
        advertise_mdns: true,
    }
    .plan(4242)
    .unwrap();
    let desktop = HostConfig {
        application: "Chataigne".into(),
        mode: HostMode::Desktop,
        network,
        advertise_mdns: true,
    }
    .plan(4242)
    .unwrap();
    assert_eq!(headless.connection_url, desktop.connection_url);
}
