use std::{
    collections::HashSet,
    io,
    net::{IpAddr, SocketAddr},
};

use url::{Host, Url};

use super::HttpRequest;

const MAX_TRUST_ENTRIES: usize = 32;
const MAX_TRUST_ENTRY_BYTES: usize = 2_048;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RequestAdmission {
    pub(super) origin: Option<String>,
    pub(super) authority: String,
}

/// Host policy is configured once, rather than inferred from the client's Origin or DNS.
#[derive(Debug)]
pub(super) struct RequestAccessPolicy {
    allowed_origins: HashSet<String>,
    allowed_hosts: HashSet<String>,
}

impl RequestAccessPolicy {
    pub(super) fn new(origins: &[String], hosts: &[String]) -> io::Result<Self> {
        if origins.len() > MAX_TRUST_ENTRIES || hosts.len() > MAX_TRUST_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "UI trust lists admit at most 32 entries each",
            ));
        }
        let allowed_origins = origins
            .iter()
            .map(|value| parse_origin(value).map(|url| url.origin().ascii_serialization()))
            .collect::<Result<HashSet<_>, _>>()?;
        let allowed_hosts = hosts
            .iter()
            .map(|value| parse_authority(value).map(|url| url.authority().to_owned()))
            .collect::<Result<HashSet<_>, _>>()?;
        Ok(Self {
            allowed_origins,
            allowed_hosts,
        })
    }

    pub(super) fn admit(&self, request: &HttpRequest, local: SocketAddr) -> Result<RequestAdmission, &'static str> {
        let host = request.headers.get("host").ok_or("Host is required")?;
        let host = parse_authority(host).map_err(|_| "invalid Host authority")?;
        if host.port_or_known_default() != Some(local.port()) {
            return Err("Host port does not match the listener");
        }
        let local_ip = local.ip().to_canonical();
        let local_host = match host.host() {
            Some(Host::Ipv4(ip)) => IpAddr::V4(ip) == local_ip,
            Some(Host::Ipv6(ip)) => IpAddr::V6(ip).to_canonical() == local_ip,
            Some(Host::Domain("localhost")) => local_ip.is_loopback(),
            _ => false,
        };
        if !local_host && !self.allowed_hosts.contains(host.authority()) {
            return Err("Host is not an admitted listener address or configured authority");
        }

        let Some(origin) = request.headers.get("origin") else {
            if request.path.starts_with("/api/")
                && request
                    .headers
                    .get("sec-fetch-site")
                    .is_some_and(|site| !matches!(site.as_str(), "same-origin" | "none"))
            {
                return Err("cross-origin browser API requests require an admitted Origin");
            }
            // Native/headless peers share the configured listener's trusted-network boundary.
            return Ok(RequestAdmission {
                origin: None,
                authority: host.authority().to_owned(),
            });
        };
        let origin = parse_origin(origin)
            .map_err(|_| "invalid browser Origin")?
            .origin()
            .ascii_serialization();
        if origin != host.origin().ascii_serialization() && !self.allowed_origins.contains(&origin) {
            return Err("browser Origin is not admitted");
        }
        Ok(RequestAdmission {
            origin: Some(origin),
            authority: host.authority().to_owned(),
        })
    }
}

pub(super) fn validate_preflight(request: &HttpRequest) -> Result<(), &'static str> {
    if !request.headers.contains_key("origin") {
        return Err("preflight requires Origin");
    }
    if !request
        .headers
        .get("access-control-request-method")
        .is_some_and(|method| matches!(method.as_str(), "GET" | "POST"))
    {
        return Err("preflight method is not supported");
    }
    if let Some(headers) = request.headers.get("access-control-request-headers")
        && !headers.split(',').all(|name| {
            matches!(
                name.trim().to_ascii_lowercase().as_str(),
                "content-type" | "x-gc-ui-client-instance" | "authorization"
            )
        })
    {
        return Err("preflight headers are not supported");
    }
    Ok(())
}

fn parse_authority(value: &str) -> io::Result<Url> {
    if value.is_empty()
        || value.len() > MAX_TRUST_ENTRY_BYTES
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || b"/@?#\\,%".contains(&byte))
    {
        return Err(invalid_trust_entry());
    }
    parse_origin(&format!("http://{value}"))
}

fn parse_origin(value: &str) -> io::Result<Url> {
    if value.len() > MAX_TRUST_ENTRY_BYTES
        || !(value.starts_with("http://") || value.starts_with("https://"))
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace() || b"\\*".contains(&byte))
    {
        return Err(invalid_trust_entry());
    }
    let url = Url::parse(value).map_err(|_| invalid_trust_entry())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_trust_entry());
    }
    Ok(url)
}

fn invalid_trust_entry() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "UI trust entry must be a bounded HTTP(S) origin or Host authority",
    )
}
