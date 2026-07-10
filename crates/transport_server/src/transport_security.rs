use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use serde::Serialize;

/// Capacity and timeout limits for the open studio-network transport.
#[derive(Clone, Debug)]
pub struct UiTransportLimits {
    /// Maximum simultaneous accepted HTTP and WebSocket connections.
    pub max_connections: usize,
    /// Maximum complete HTTP request size, including headers and body.
    pub max_http_request_bytes: usize,
    /// Maximum WebSocket frame and reassembled message size.
    pub max_websocket_message_bytes: usize,
    /// Maximum intents accepted in one WebSocket batch.
    pub max_intents_per_batch: usize,
    /// Maximum active subscriptions retained for one WebSocket client.
    pub max_subscriptions_per_client: usize,
    /// Maximum client messages accepted during one rate interval.
    pub max_messages_per_interval: usize,
    /// Interval used by the per-client message-rate limit.
    pub message_rate_interval: Duration,
    /// Maximum queued outbound messages for one WebSocket client.
    pub outbound_queue_capacity: usize,
    /// Maximum queued commands waiting for the shared WebSocket hub.
    pub hub_command_queue_capacity: usize,
    /// Timeout for partial HTTP requests and WebSocket handshakes.
    pub handshake_timeout: Duration,
    /// Timeout for blocking HTTP response writes.
    pub write_timeout: Duration,
    /// Maximum accepted request/subscription/client identifier length.
    pub max_identifier_bytes: usize,
    /// Maximum JSON object/array nesting accepted before deserialization.
    pub max_json_depth: usize,
    /// Maximum one string token accepted in a JSON request.
    pub max_json_string_bytes: usize,
    /// Maximum project path or upload file-name length accepted at the host boundary.
    pub max_path_bytes: usize,
}

impl Default for UiTransportLimits {
    fn default() -> Self {
        Self {
            max_connections: 64,
            max_http_request_bytes: 8 * 1024 * 1024,
            max_websocket_message_bytes: 1024 * 1024,
            max_intents_per_batch: 256,
            max_subscriptions_per_client: 64,
            max_messages_per_interval: 240,
            message_rate_interval: Duration::from_secs(1),
            outbound_queue_capacity: 128,
            hub_command_queue_capacity: 4_096,
            handshake_timeout: Duration::from_secs(3),
            write_timeout: Duration::from_secs(3),
            max_identifier_bytes: 256,
            max_json_depth: 64,
            max_json_string_bytes: 8 * 1024 * 1024,
            max_path_bytes: 4_096,
        }
    }
}

/// Browser-origin, host, discovery, and capacity configuration for the open transport.
#[derive(Clone, Debug, Default)]
pub struct UiTransportSecurityConfig {
    /// Explicit browser origins allowed in addition to same-origin requests.
    pub allowed_origins: Vec<String>,
    /// Explicit host names accepted in addition to the actual local socket address.
    pub allowed_hosts: Vec<String>,
    /// Optional LAN discovery name exposed by connection-info and discovery hooks.
    pub advertised_name: Option<String>,
    /// Transport capacity and timeout limits.
    pub limits: UiTransportLimits,
}

#[derive(Default)]
pub(crate) struct TransportMetrics {
    active_connections: AtomicUsize,
    active_websockets: AtomicUsize,
    rejected_connections: AtomicU64,
    rejected_origins: AtomicU64,
    rejected_hosts: AtomicU64,
    oversized_requests: AtomicU64,
    rate_limited_messages: AtomicU64,
    dropped_outbound_messages: AtomicU64,
    resync_requests: AtomicU64,
    protocol_errors: AtomicU64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransportMetricsSnapshot {
    pub active_connections: usize,
    pub active_websockets: usize,
    pub rejected_connections: u64,
    pub rejected_origins: u64,
    pub rejected_hosts: u64,
    pub oversized_requests: u64,
    pub rate_limited_messages: u64,
    pub dropped_outbound_messages: u64,
    pub resync_requests: u64,
    pub protocol_errors: u64,
}

impl TransportMetrics {
    pub(crate) fn snapshot(&self) -> TransportMetricsSnapshot {
        TransportMetricsSnapshot {
            active_connections: self.active_connections.load(Ordering::Relaxed),
            active_websockets: self.active_websockets.load(Ordering::Relaxed),
            rejected_connections: self.rejected_connections.load(Ordering::Relaxed),
            rejected_origins: self.rejected_origins.load(Ordering::Relaxed),
            rejected_hosts: self.rejected_hosts.load(Ordering::Relaxed),
            oversized_requests: self.oversized_requests.load(Ordering::Relaxed),
            rate_limited_messages: self.rate_limited_messages.load(Ordering::Relaxed),
            dropped_outbound_messages: self.dropped_outbound_messages.load(Ordering::Relaxed),
            resync_requests: self.resync_requests.load(Ordering::Relaxed),
            protocol_errors: self.protocol_errors.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn try_acquire_connection(self: &Arc<Self>, limit: usize) -> Option<ConnectionPermit> {
        let acquired = self
            .active_connections
            .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |current| {
                (current < limit).then_some(current + 1)
            })
            .is_ok();
        if acquired {
            Some(ConnectionPermit {
                metrics: Arc::clone(self),
            })
        } else {
            self.rejected_connections.fetch_add(1, Ordering::Relaxed);
            None
        }
    }

    pub(crate) fn websocket_opened(&self) {
        self.active_websockets.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn websocket_closed(&self) {
        self.active_websockets.fetch_sub(1, Ordering::Relaxed);
    }

    pub(crate) fn rejected_origin(&self) {
        self.rejected_origins.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn rejected_host(&self) {
        self.rejected_hosts.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn oversized_request(&self) {
        self.oversized_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn rate_limited(&self) {
        self.rate_limited_messages.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn dropped_outbound(&self, count: u64) {
        self.dropped_outbound_messages.fetch_add(count, Ordering::Relaxed);
    }

    pub(crate) fn resync_requested(&self) {
        self.resync_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn protocol_error(&self) {
        self.protocol_errors.fetch_add(1, Ordering::Relaxed);
    }
}

pub(crate) struct ConnectionPermit {
    metrics: Arc<TransportMetrics>,
}

impl Drop for ConnectionPermit {
    fn drop(&mut self) {
        self.metrics.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

pub(crate) fn validate_browser_request(
    headers: &HashMap<String, String>,
    local_addr: SocketAddr,
    security: &UiTransportSecurityConfig,
) -> Result<Option<String>, BrowserRequestRejection> {
    let host = headers
        .get("host")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or(BrowserRequestRejection::Host("missing Host header"))?;
    validate_host(host, local_addr, security)?;

    let Some(origin) = headers.get("origin").map(|value| value.trim()) else {
        return Ok(None);
    };
    if origin.is_empty() || origin.eq_ignore_ascii_case("null") {
        return Err(BrowserRequestRejection::Origin("opaque browser origin is not allowed"));
    }
    let origin_authority = origin_authority(origin).ok_or(BrowserRequestRejection::Origin("invalid Origin header"))?;
    if authority_eq(origin_authority, host)
        || security.allowed_origins.iter().any(|allowed| {
            allowed
                .trim_end_matches('/')
                .eq_ignore_ascii_case(origin.trim_end_matches('/'))
        })
    {
        return Ok(Some(origin.to_owned()));
    }
    Err(BrowserRequestRejection::Origin("foreign browser origin is not allowed"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowserRequestRejection {
    Host(&'static str),
    Origin(&'static str),
}

pub(crate) fn validate_json_shape(body: &[u8], max_depth: usize, max_string_bytes: usize) -> Result<(), &'static str> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut string_bytes = 0usize;
    for byte in body {
        if in_string {
            if escaped {
                escaped = false;
                string_bytes = string_bytes.saturating_add(1);
                continue;
            }
            match byte {
                b'\\' => escaped = true,
                b'"' => {
                    in_string = false;
                    string_bytes = 0;
                }
                _ => {
                    string_bytes = string_bytes.saturating_add(1);
                    if string_bytes > max_string_bytes {
                        return Err("JSON string exceeds the configured size limit");
                    }
                }
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth = depth.saturating_add(1);
                if depth > max_depth {
                    return Err("JSON nesting exceeds the configured depth limit");
                }
            }
            b'}' | b']' => {
                depth = depth.checked_sub(1).ok_or("JSON delimiters are unbalanced")?;
            }
            _ => {}
        }
    }
    if in_string || depth != 0 {
        return Err("JSON payload is incomplete");
    }
    Ok(())
}

fn validate_host(
    host: &str,
    local_addr: SocketAddr,
    security: &UiTransportSecurityConfig,
) -> Result<(), BrowserRequestRejection> {
    let (host_name, port) = split_host_port(host).ok_or(BrowserRequestRejection::Host("invalid Host header"))?;
    if port.is_some_and(|port| port != local_addr.port()) {
        return Err(BrowserRequestRejection::Host(
            "Host port does not match the listening socket",
        ));
    }
    let normalized = host_name.trim_matches(['[', ']']);
    let actual_ip = local_addr.ip();
    let advertised_hostname = security
        .advertised_name
        .as_deref()
        .and_then(normalize_advertised_hostname);
    let accepted = normalized.eq_ignore_ascii_case("localhost")
        || normalized.parse::<IpAddr>().is_ok_and(|ip| ip == actual_ip)
        || security
            .advertised_name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(normalized))
        || advertised_hostname
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(normalized))
        || security
            .allowed_hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(normalized));
    accepted
        .then_some(())
        .ok_or(BrowserRequestRejection::Host("Host is not a local or configured name"))
}

fn normalize_advertised_hostname(name: &str) -> Option<String> {
    let label = name
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || value == '-' {
                value.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(63)
        .collect::<String>();
    (!label.is_empty()).then(|| format!("{label}.local"))
}

fn split_host_port(authority: &str) -> Option<(&str, Option<u16>)> {
    if authority.starts_with('[') {
        let end = authority.find(']')?;
        let host = &authority[..=end];
        let tail = &authority[end + 1..];
        return if tail.is_empty() {
            Some((host, None))
        } else {
            Some((host, Some(tail.strip_prefix(':')?.parse().ok()?)))
        };
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.chars().all(|value| value.is_ascii_digit()) => {
            Some((host, Some(port.parse().ok()?)))
        }
        _ => Some((authority, None)),
    }
}

fn origin_authority(origin: &str) -> Option<&str> {
    let (_, remainder) = origin.split_once("://")?;
    let authority = remainder.split('/').next()?.trim();
    (!authority.is_empty()).then_some(authority)
}

fn authority_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[cfg(test)]
#[path = "transport_security_tests.rs"]
mod tests;
