use super::{transport::HttpTransportConfig, HttpModule};
use crate::app::module::common::http::{parse_header_lines, HttpAuthConfig, HttpAuthMode, HttpHeader};
use golden_core::{node::NodeId, parameter::ParamValue, process_ctx::ProcessTreeSnapshot};
use std::{borrow::Cow, mem::size_of, time::Duration};

const BASE_INPUT_BYTES: usize = 8 * 1024;
const HEADER_INPUT_BYTES: usize = 64 * 1024;
const AUTH_INPUT_BYTES: usize = 8 * 1024;
const CONFIG_BYTES: usize = 128 * 1024;
const MAX_HEADERS: usize = 256;
const NATIVE_BASE_BYTES: usize = 32 * 1024;

/// Borrowed during admission; owned only after a generation is configured. Keep
/// authored input identity separate from normalized native configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct HttpSettings<'a> {
    base_address: Cow<'a, str>,
    headers: Cow<'a, str>,
    auth: AuthSettings<'a>,
    timeout: Duration,
    retries: u32,
    retry_delay: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum AuthSettings<'a> {
    None,
    Basic {
        username: Cow<'a, str>,
        password: Cow<'a, str>,
    },
    Bearer(Cow<'a, str>),
    Header {
        name: Cow<'a, str>,
        value: Cow<'a, str>,
    },
}

impl<'a> HttpSettings<'a> {
    pub(super) fn read(module: &HttpModule, snapshot: &'a ProcessTreeSnapshot) -> Result<Self, String> {
        let base_address = text(snapshot, module.base_address.id(), "Base address", BASE_INPUT_BYTES)?;
        let headers = text(
            snapshot,
            module.default_headers.id(),
            "Default Headers",
            HEADER_INPUT_BYTES,
        )?;
        let auth = if !module.auth_enabled(snapshot).ok_or("HTTP Auth folder is unavailable")? {
            AuthSettings::None
        } else {
            let mode = match value(snapshot, module.auth_mode.id())? {
                ParamValue::Enum(mode) if mode.len() <= 64 => {
                    HttpAuthMode::from_variant(mode).ok_or_else(|| "invalid HTTP auth mode".to_string())?
                }
                _ => return Err("invalid HTTP auth mode".into()),
            };
            match mode {
                HttpAuthMode::None => AuthSettings::None,
                HttpAuthMode::Basic => AuthSettings::Basic {
                    username: text(snapshot, module.username.id(), "auth username", AUTH_INPUT_BYTES)?.into(),
                    password: text(snapshot, module.password.id(), "auth password", AUTH_INPUT_BYTES)?.into(),
                },
                HttpAuthMode::Bearer => {
                    AuthSettings::Bearer(text(snapshot, module.token.id(), "auth token", AUTH_INPUT_BYTES)?.into())
                }
                HttpAuthMode::Header => AuthSettings::Header {
                    name: text(snapshot, module.header_name.id(), "auth header name", AUTH_INPUT_BYTES)?.into(),
                    value: text(
                        snapshot,
                        module.header_value.id(),
                        "auth header value",
                        AUTH_INPUT_BYTES,
                    )?
                    .into(),
                },
            }
        };
        let timeout = integer(snapshot, module.timeout_ms.id(), "timeout", 1, i32::MAX)?;
        let retries = integer(snapshot, module.retry_count.id(), "retry count", 0, 10)?;
        let delay = integer(snapshot, module.retry_delay_ms.id(), "retry delay", 0, 60_000)?;
        Ok(Self {
            base_address: base_address.into(),
            headers: headers.into(),
            auth,
            timeout: Duration::from_millis(timeout as u64),
            retries: retries as u32,
            retry_delay: Duration::from_millis(delay as u64),
        })
    }

    pub(super) fn owned(&self) -> HttpSettings<'static> {
        HttpSettings {
            base_address: self.base_address.to_string().into(),
            headers: self.headers.to_string().into(),
            auth: match &self.auth {
                AuthSettings::None => AuthSettings::None,
                AuthSettings::Basic { username, password } => AuthSettings::Basic {
                    username: username.to_string().into(),
                    password: password.to_string().into(),
                },
                AuthSettings::Bearer(token) => AuthSettings::Bearer(token.to_string().into()),
                AuthSettings::Header { name, value } => AuthSettings::Header {
                    name: name.to_string().into(),
                    value: value.to_string().into(),
                },
            },
            timeout: self.timeout,
            retries: self.retries,
            retry_delay: self.retry_delay,
        }
    }

    pub(super) fn prepare(&self) -> Result<HttpTransportConfig, String> {
        // Count meaningful lines before the parser allocates any header records.
        if self
            .headers
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .take(MAX_HEADERS + 1)
            .count()
            > MAX_HEADERS
        {
            return Err("HTTP configuration supports at most 256 default headers".into());
        }
        let auth = match &self.auth {
            AuthSettings::None => HttpAuthConfig::default(),
            AuthSettings::Basic { username, password } => HttpAuthConfig {
                mode: HttpAuthMode::Basic,
                username: username.to_string(),
                password: password.to_string(),
                ..Default::default()
            },
            AuthSettings::Bearer(token) => HttpAuthConfig {
                mode: HttpAuthMode::Bearer,
                token: token.to_string(),
                ..Default::default()
            },
            AuthSettings::Header { name, value } => HttpAuthConfig {
                mode: HttpAuthMode::Header,
                header_name: name.to_string(),
                header_value: value.to_string(),
                ..Default::default()
            },
        };
        let config = HttpTransportConfig {
            base_address: normalize_base_address(&self.base_address)?,
            default_headers: parse_header_lines(&self.headers, "HTTP default")?,
            auth,
            timeout: self.timeout,
            retry_count: self.retries,
            retry_delay: self.retry_delay,
        };
        config.validate_limits()?;
        Ok(config)
    }
}

impl HttpTransportConfig {
    /// Native entrypoints also validate independently of node parameter constraints.
    pub(super) fn validate_limits(&self) -> Result<(), String> {
        if self.base_address.capacity() > NATIVE_BASE_BYTES || self.base_address.trim().is_empty() {
            return Err("HTTP native base address exceeds 32 KiB or is empty".into());
        }
        if self.default_headers.len() > MAX_HEADERS {
            return Err("HTTP configuration supports at most 256 default headers".into());
        }
        let strings = [
            &self.auth.username,
            &self.auth.password,
            &self.auth.token,
            &self.auth.header_name,
            &self.auth.header_value,
        ];
        if strings.iter().any(|value| value.capacity() > AUTH_INPUT_BYTES) {
            return Err("HTTP native auth field exceeds 8 KiB".into());
        }
        let mut bytes = size_of::<Self>()
            + self.base_address.capacity()
            + self.default_headers.capacity().saturating_mul(size_of::<HttpHeader>());
        bytes = bytes.saturating_add(strings.iter().map(|value| value.capacity()).sum::<usize>());
        if bytes > CONFIG_BYTES {
            return Err("HTTP native configuration exceeds 128 KiB".into());
        }
        for header in &self.default_headers {
            bytes = bytes
                .saturating_add(header.name.capacity())
                .saturating_add(header.value.capacity());
            if bytes > CONFIG_BYTES {
                return Err("HTTP native configuration exceeds 128 KiB".into());
            }
        }
        if self.timeout < Duration::from_millis(1)
            || self.timeout > Duration::from_millis(i32::MAX as u64)
            || self.retry_count > 10
            || self.retry_delay > Duration::from_secs(60)
        {
            return Err("HTTP timeout/retry settings exceed supported ranges".into());
        }
        match self.auth.mode {
            HttpAuthMode::Basic if self.auth.username.trim().is_empty() => {
                return Err("HTTP Basic auth requires a username".into())
            }
            HttpAuthMode::Bearer if self.auth.token.trim().is_empty() => {
                return Err("HTTP Bearer auth requires a token".into())
            }
            HttpAuthMode::Header => {
                reqwest::header::HeaderName::from_bytes(self.auth.header_name.as_bytes())
                    .map_err(|_| "HTTP auth header name is invalid".to_string())?;
                reqwest::header::HeaderValue::from_str(&self.auth.header_value)
                    .map_err(|_| "HTTP auth header value is invalid".to_string())?;
            }
            _ => {}
        }
        Ok(())
    }
}

fn value(snapshot: &ProcessTreeSnapshot, id: NodeId) -> Result<&ParamValue, String> {
    snapshot
        .node(id)
        .and_then(|node| node.param_value.as_ref())
        .ok_or_else(|| "HTTP configuration parameter is unavailable".into())
}

fn text<'a>(snapshot: &'a ProcessTreeSnapshot, id: NodeId, label: &str, maximum: usize) -> Result<&'a str, String> {
    match value(snapshot, id)? {
        ParamValue::Str(value) if value.len() <= maximum => Ok(value),
        ParamValue::Str(_) => Err(format!("HTTP {label} exceeds {maximum} UTF-8 bytes")),
        _ => Err(format!("HTTP {label} must be a string")),
    }
}

fn integer(snapshot: &ProcessTreeSnapshot, id: NodeId, label: &str, minimum: i32, maximum: i32) -> Result<i32, String> {
    match value(snapshot, id)? {
        ParamValue::Int(value) if (minimum..=maximum).contains(value) => Ok(*value),
        _ => Err(format!("HTTP {label} must be between {minimum} and {maximum}")),
    }
}

fn normalize_base_address(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("HTTP Base address cannot be empty".into());
    }
    let normalized = if trimmed.ends_with('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}/")
    };
    let parsed = reqwest::Url::parse(&normalized).map_err(|_| "invalid HTTP Base address".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("HTTP Base address must use http:// or https:// and include a host".into());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("HTTP Base address must not include a query string or fragment".into());
    }
    Ok(parsed.as_str().into())
}
