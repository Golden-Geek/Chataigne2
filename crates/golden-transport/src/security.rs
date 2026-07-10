use std::{
    collections::BTreeSet,
    net::IpAddr,
    sync::atomic::{AtomicUsize, Ordering},
};

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkAccess {
    Loopback,
    OpenLan,
    Authenticated,
}

#[derive(Clone)]
pub struct NetworkPolicy {
    pub access: NetworkAccess,
    pub bind_address: IpAddr,
    pub tls_enabled: bool,
    pub authentication_token: Option<String>,
    pub allowed_origins: BTreeSet<String>,
    pub advertised_hosts: BTreeSet<String>,
    pub maximum_clients: usize,
    pub maximum_payload_bytes: usize,
}

impl NetworkPolicy {
    pub fn validate(&self) -> Result<(), NetworkPolicyError> {
        if self.maximum_clients == 0 || self.maximum_payload_bytes == 0 {
            return Err(NetworkPolicyError::ZeroLimit);
        }
        if self.access == NetworkAccess::Loopback && !self.bind_address.is_loopback() {
            return Err(NetworkPolicyError::LoopbackBindingRequired);
        }
        if self.access == NetworkAccess::Authenticated {
            if !self.tls_enabled {
                return Err(NetworkPolicyError::TlsRequired);
            }
            if self.authentication_token.as_ref().is_none_or(|token| token.len() < 32) {
                return Err(NetworkPolicyError::StrongTokenRequired);
            }
        }
        if !self.bind_address.is_loopback() {
            if self.allowed_origins.is_empty() || self.allowed_origins.contains("*") {
                return Err(NetworkPolicyError::ExplicitOriginsRequired);
            }
            if self.advertised_hosts.is_empty() || self.advertised_hosts.contains("*") {
                return Err(NetworkPolicyError::ExplicitHostsRequired);
            }
        }
        Ok(())
    }

    pub fn authorize(&self, origin: &str, token: Option<&str>) -> bool {
        let origin_allowed = self.bind_address.is_loopback() || self.allowed_origins.contains(origin);
        let token_allowed = self
            .authentication_token
            .as_deref()
            .is_none_or(|expected| token.is_some_and(|provided| constant_time_eq(expected, provided)));
        origin_allowed && token_allowed
    }

    pub fn validate_host(&self, host: &str) -> bool {
        self.bind_address.is_loopback() || self.advertised_hosts.contains(host)
    }

    pub fn validate_payload(&self, bytes: usize) -> Result<(), AdmissionError> {
        if bytes > self.maximum_payload_bytes {
            Err(AdmissionError::PayloadTooLarge {
                bytes,
                maximum: self.maximum_payload_bytes,
            })
        } else {
            Ok(())
        }
    }
}

fn constant_time_eq(expected: &str, provided: &str) -> bool {
    if expected.len() != provided.len() {
        return false;
    }
    expected
        .bytes()
        .zip(provided.bytes())
        .fold(0_u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

pub struct ConnectionLimiter {
    maximum: usize,
    active: AtomicUsize,
}

impl ConnectionLimiter {
    pub const fn new(maximum: usize) -> Self {
        Self {
            maximum,
            active: AtomicUsize::new(0),
        }
    }

    pub fn try_acquire(&self) -> Result<ConnectionPermit<'_>, AdmissionError> {
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.maximum).then_some(active + 1)
            })
            .map_err(|_| AdmissionError::ClientLimit)?;
        Ok(ConnectionPermit { limiter: self })
    }

    pub fn active(&self) -> usize {
        self.active.load(Ordering::Acquire)
    }
}

pub struct ConnectionPermit<'a> {
    limiter: &'a ConnectionLimiter,
}

impl Drop for ConnectionPermit<'_> {
    fn drop(&mut self) {
        self.limiter.active.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum AdmissionError {
    #[error("maximum concurrent client count reached")]
    ClientLimit,
    #[error("payload contains {bytes} bytes, exceeding limit {maximum}")]
    PayloadTooLarge { bytes: usize, maximum: usize },
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum NetworkPolicyError {
    #[error("network client and payload limits must be non-zero")]
    ZeroLimit,
    #[error("loopback access must bind to a loopback address")]
    LoopbackBindingRequired,
    #[error("TLS is required when binding beyond loopback")]
    TlsRequired,
    #[error("a token of at least 32 bytes is required when binding beyond loopback")]
    StrongTokenRequired,
    #[error("explicit non-wildcard origins are required when binding beyond loopback")]
    ExplicitOriginsRequired,
    #[error("explicit non-wildcard advertised hosts are required when binding beyond loopback")]
    ExplicitHostsRequired,
}
