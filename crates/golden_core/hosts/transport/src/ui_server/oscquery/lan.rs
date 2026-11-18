use if_addrs::{IfAddr, Ifv4Addr, Interface};
use mdns_sd::{IfKind, IfPredicate, ServiceDaemon, ServiceInfo};
use std::{
    io,
    net::{IpAddr, Ipv4Addr},
    time::{Duration, Instant},
};

pub(super) const SERVICE_TYPE: &str = "_oscjson._tcp.local.";
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub(super) struct Lan {
    hostname: String,
    instance: String,
    interfaces: Vec<Ifv4Addr>,
    refresh_at: Instant,
    service: Option<PublishedService>,
    failed: bool,
}

impl Lan {
    pub(super) fn new(name: &str) -> io::Result<Self> {
        let mut suffix = [0; 8];
        getrandom::fill(&mut suffix).map_err(|error| io::Error::other(error.to_string()))?;
        let suffix = u64::from_le_bytes(suffix);
        let name = &name[..name.floor_char_boundary(40)];
        Ok(Self {
            hostname: format!("golden-{suffix:016x}.local."),
            instance: format!("{name} ({suffix:016x})"),
            interfaces: Vec::new(),
            refresh_at: Instant::now(),
            service: None,
            failed: false,
        })
    }

    pub(super) fn hostname(&self) -> &str {
        &self.hostname
    }

    pub(super) fn permits(&self, peer: IpAddr) -> bool {
        match peer {
            IpAddr::V4(ip) if ip.is_loopback() => true,
            IpAddr::V4(ip) if !ip.is_multicast() && !ip.is_broadcast() && !ip.is_unspecified() => {
                self.interfaces.iter().any(|adapter| {
                    let mask = adapter.netmask.to_bits();
                    ip.to_bits() & mask == adapter.ip.to_bits() & mask
                })
            }
            _ => false,
        }
    }

    pub(super) fn stop(&mut self, now: Instant) {
        self.service = None;
        self.interfaces.clear();
        self.failed = false;
        self.refresh_at = now;
    }

    pub(super) fn refresh(&mut self, port: u16, now: Instant) {
        if now < self.refresh_at {
            return;
        }
        self.refresh_at = now + REFRESH_INTERVAL;
        self.interfaces = if_addrs::get_if_addrs()
            .unwrap_or_default()
            .into_iter()
            .filter(eligible)
            .filter_map(|interface| match interface.addr {
                IfAddr::V4(address) => Some(address),
                IfAddr::V6(_) => None,
            })
            .collect();
        if self.service.is_none() && !self.interfaces.is_empty() {
            match PublishedService::start(&self.instance, &self.hostname, port) {
                Ok(service) => {
                    self.service = Some(service);
                    self.failed = false;
                }
                Err(error) if !self.failed => {
                    self.failed = true;
                    eprintln!("[oscquery] Zeroconf registration failed: {error}; retrying");
                }
                Err(_) => {}
            }
        }
    }
}

fn eligible(interface: &Interface) -> bool {
    matches!(&interface.addr, IfAddr::V4(address) if !address.is_loopback()
        && !address.ip.is_unspecified() && address.netmask != Ipv4Addr::UNSPECIFIED)
        && interface.is_oper_up()
        && !interface.is_p2p()
}

struct PublishedService {
    daemon: ServiceDaemon,
    fullname: String,
}

impl PublishedService {
    fn start(instance: &str, hostname: &str, port: u16) -> mdns_sd::Result<Self> {
        let info = ServiceInfo::new(SERVICE_TYPE, instance, hostname, "", port, None)?.enable_addr_auto();
        let service = Self {
            daemon: ServiceDaemon::new()?,
            fullname: info.get_fullname().to_owned(),
        };
        service
            .daemon
            .disable_interface(IfKind::Predicate(IfPredicate::new(|interface| !eligible(interface))))?;
        service
            .daemon
            .set_ip_check_interval(REFRESH_INTERVAL.as_secs() as u32)?;
        service.daemon.register(info)?;
        Ok(service)
    }
}

impl Drop for PublishedService {
    fn drop(&mut self) {
        // The daemon dequeues the goodbye before processing the shutdown command.
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}
