//! Reusable host planning for desktop and headless Golden applications.

use golden_transport::{NetworkPolicy, NetworkPolicyError};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum HostMode {
    Headless,
    Desktop,
}

#[derive(Clone)]
pub struct HostConfig {
    pub application: SmolStr,
    pub mode: HostMode,
    pub network: NetworkPolicy,
    pub advertise_mdns: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostLaunchPlan {
    pub application: SmolStr,
    pub mode: HostMode,
    pub advertise_mdns: bool,
    pub connection_url: String,
}

impl HostConfig {
    pub fn plan(&self, port: u16) -> Result<HostLaunchPlan, HostConfigError> {
        if self.application.is_empty() || port == 0 {
            return Err(HostConfigError::InvalidApplicationOrPort);
        }
        self.network.validate()?;
        let scheme = if self.network.tls_enabled { "wss" } else { "ws" };
        Ok(HostLaunchPlan {
            application: self.application.clone(),
            mode: self.mode,
            advertise_mdns: self.advertise_mdns,
            connection_url: format!("{scheme}://{}:{port}/api", self.network.bind_address),
        })
    }
}

pub trait DesktopShell {
    type Error;

    fn open_project_dialog(&self) -> Result<Option<String>, Self::Error>;
    fn save_project_dialog(&self, suggested_name: &str) -> Result<Option<String>, Self::Error>;
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum HostConfigError {
    #[error("host application name and port must be valid")]
    InvalidApplicationOrPort,
    #[error(transparent)]
    Network(#[from] NetworkPolicyError),
}

#[cfg(test)]
mod tests;
