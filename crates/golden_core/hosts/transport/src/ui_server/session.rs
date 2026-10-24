use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Instant,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use golden_protocol::{UI_PROTOCOL_VERSION, UiSessionCapabilities, UiSessionRequest, UiSessionResponse};

use super::{HttpRequest, access::RequestAdmission};

pub(super) const SESSION_PROTOCOL: &str = "golden-ui";
pub(super) const TOKEN_PROTOCOL_PREFIX: &str = "gc-session.";
const SESSION_LIFETIME_MS: u64 = 60 * 60 * 1_000;
const MAX_SESSIONS: usize = 128;

#[derive(Clone, Copy, Debug)]
pub(super) enum Capability {
    Observe,
    Author,
    ProjectFiles,
    Scripts,
    Session,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SessionFailure {
    pub(super) status: &'static str,
    pub(super) message: &'static str,
}

impl SessionFailure {
    fn unauthorized() -> Self {
        Self {
            status: "401 Unauthorized",
            message: "session is missing, expired or revoked",
        }
    }
    pub(super) fn forbidden() -> Self {
        Self {
            status: "403 Forbidden",
            message: "session capability is not granted",
        }
    }
    fn unavailable() -> Self {
        Self {
            status: "503 Service Unavailable",
            message: "session admission is unavailable or at capacity",
        }
    }
}

#[derive(Debug)]
pub(super) struct SessionGrant {
    epoch: Instant,
    expires_at_ms: AtomicU64,
    active: AtomicBool,
    capabilities: UiSessionCapabilities,
}

impl SessionGrant {
    pub(super) fn require(&self, capability: Capability) -> Result<(), SessionFailure> {
        self.check_at(elapsed_ms(self.epoch))?;
        let allowed = match capability {
            Capability::Observe => self.capabilities.observe,
            Capability::Author => self.capabilities.author,
            Capability::ProjectFiles => self.capabilities.project_files,
            Capability::Scripts => self.capabilities.scripts,
            Capability::Session => true,
        };
        if allowed {
            Ok(())
        } else {
            Err(SessionFailure::forbidden())
        }
    }

    pub(super) fn check(&self) -> Result<(), SessionFailure> {
        self.check_at(elapsed_ms(self.epoch))
    }

    fn check_at(&self, now: u64) -> Result<(), SessionFailure> {
        if !self.active.load(Ordering::Acquire) || now >= self.expires_at_ms.load(Ordering::Acquire) {
            Err(SessionFailure::unauthorized())
        } else {
            Ok(())
        }
    }
}

struct SessionRecord {
    admission: RequestAdmission,
    grant: Arc<SessionGrant>,
}

pub(super) struct SessionRegistry {
    epoch: Instant,
    allowed: UiSessionCapabilities,
    records: Mutex<HashMap<String, SessionRecord>>,
}

impl fmt::Debug for SessionRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionRegistry")
            .field("allowed", &self.allowed)
            .finish_non_exhaustive()
    }
}

impl SessionRegistry {
    pub(super) fn new(allowed: UiSessionCapabilities) -> Self {
        Self {
            epoch: Instant::now(),
            allowed,
            records: Mutex::new(HashMap::new()),
        }
    }

    pub(super) fn bootstrap(
        &self,
        admission: &RequestAdmission,
        request: UiSessionRequest,
        previous: Option<&str>,
    ) -> Result<UiSessionResponse, SessionFailure> {
        self.bootstrap_at(admission, request, previous, elapsed_ms(self.epoch))
    }

    fn bootstrap_at(
        &self,
        admission: &RequestAdmission,
        request: UiSessionRequest,
        previous: Option<&str>,
        now: u64,
    ) -> Result<UiSessionResponse, SessionFailure> {
        let capabilities = request.capabilities.unwrap_or(self.allowed);
        if !capabilities.is_subset_of(self.allowed) {
            return Err(SessionFailure::forbidden());
        }
        let mut records = self.records.lock().map_err(|_| SessionFailure::unavailable())?;
        if let Some(token) = previous {
            let record = records.get(token).ok_or_else(SessionFailure::unauthorized)?;
            if &record.admission != admission {
                return Err(SessionFailure::unauthorized());
            }
            record.grant.check_at(now)?;
            if record.grant.capabilities != capabilities {
                return Err(SessionFailure::forbidden());
            }
            record
                .grant
                .expires_at_ms
                .store(now.saturating_add(SESSION_LIFETIME_MS), Ordering::Release);
            return Ok(response(token.to_owned(), capabilities));
        }
        records.retain(|_, record| {
            let live = record.grant.check_at(now).is_ok();
            if !live {
                record.grant.active.store(false, Ordering::Release);
            }
            live
        });
        if records.len() >= MAX_SESSIONS {
            return Err(SessionFailure::unavailable());
        }
        let mut random = [0; 32];
        getrandom::fill(&mut random).map_err(|_| SessionFailure::unavailable())?;
        let token = URL_SAFE_NO_PAD.encode(random);
        if records.contains_key(&token) {
            return Err(SessionFailure::unavailable());
        }
        records.insert(
            token.clone(),
            SessionRecord {
                admission: admission.clone(),
                grant: Arc::new(SessionGrant {
                    epoch: self.epoch,
                    expires_at_ms: AtomicU64::new(now.saturating_add(SESSION_LIFETIME_MS)),
                    active: AtomicBool::new(true),
                    capabilities,
                }),
            },
        );
        Ok(response(token, capabilities))
    }

    pub(super) fn authenticate(
        &self,
        admission: &RequestAdmission,
        request: &HttpRequest,
    ) -> Result<Arc<SessionGrant>, SessionFailure> {
        let token = request_token(request)?.ok_or_else(SessionFailure::unauthorized)?;
        let records = self.records.lock().map_err(|_| SessionFailure::unavailable())?;
        let record = records.get(token).ok_or_else(SessionFailure::unauthorized)?;
        if &record.admission != admission {
            return Err(SessionFailure::unauthorized());
        }
        record.grant.check()?;
        Ok(record.grant.clone())
    }

    pub(super) fn revoke(&self, request: &HttpRequest) -> Result<(), SessionFailure> {
        let token = request_token(request)?.ok_or_else(SessionFailure::unauthorized)?;
        let mut records = self.records.lock().map_err(|_| SessionFailure::unavailable())?;
        let record = records.remove(token).ok_or_else(SessionFailure::unauthorized)?;
        record.grant.active.store(false, Ordering::Release);
        Ok(())
    }
}

impl Drop for SessionRegistry {
    fn drop(&mut self) {
        let records = self.records.get_mut().unwrap_or_else(|poisoned| poisoned.into_inner());
        for record in records.values() {
            record.grant.active.store(false, Ordering::Release);
        }
    }
}

fn elapsed_ms(epoch: Instant) -> u64 {
    u64::try_from(epoch.elapsed().as_millis()).unwrap_or(u64::MAX)
}
fn response(token: String, capabilities: UiSessionCapabilities) -> UiSessionResponse {
    UiSessionResponse {
        websocket_protocols: vec![SESSION_PROTOCOL.into(), format!("{TOKEN_PROTOCOL_PREFIX}{token}")],
        token,
        capabilities,
        expires_in_ms: SESSION_LIFETIME_MS as u32,
        protocol_version: UI_PROTOCOL_VERSION.into(),
    }
}

pub(super) fn request_token(request: &HttpRequest) -> Result<Option<&str>, SessionFailure> {
    let header = request
        .headers
        .get("authorization")
        .map(|value| {
            let (scheme, token) = value.split_once(' ').ok_or_else(SessionFailure::unauthorized)?;
            if !scheme.eq_ignore_ascii_case("bearer") {
                return Err(SessionFailure::unauthorized());
            }
            Ok(token)
        })
        .transpose()?;
    let mut protocol = None;
    if request.path == super::UI_WEBSOCKET_PATH
        && let Some(protocols) = request.headers.get("sec-websocket-protocol")
    {
        for entry in protocols.split(',').map(str::trim) {
            if let Some(token) = entry.strip_prefix(TOKEN_PROTOCOL_PREFIX)
                && protocol.replace(token).is_some()
            {
                return Err(SessionFailure::unauthorized());
            }
        }
    }
    if header.is_some() && protocol.is_some() && header != protocol {
        return Err(SessionFailure::unauthorized());
    }
    let token = header.or(protocol);
    if token.is_some_and(|token| {
        token.len() != 43
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    }) {
        return Err(SessionFailure::unauthorized());
    }
    Ok(token)
}

#[cfg(test)]
pub(super) mod tests;
