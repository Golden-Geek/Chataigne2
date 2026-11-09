//! Resolve a preference once, then retain its exact identity for the renderer's lifetime.
use super::{AdapterId, AdapterInfo, PhysicalAdapterId, adapters, preferred_adapters};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Host-owned adapter preference, independent of rendering-device ownership.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdapterPreference {
    /// Use Windows' highest-performance available hardware adapter.
    #[default]
    Automatic,
    /// Require this exact adapter; absence never falls back to another device.
    Explicit(PhysicalAdapterId),
}

/// Failure to resolve a preference, before any rendering device is allocated.
#[derive(Debug, Error)]
pub enum AdapterSelectionError {
    /// Native adapter enumeration failed.
    #[error("GPU enumeration failed: {0}")]
    Enumeration(#[from] windows::core::Error),
    /// Automatic selection found no supported hardware adapter.
    #[error("no hardware GPU is available")]
    NoHardware,
    /// A chosen identity is absent or identifies a software adapter.
    #[error("selected GPU {0:?} is unavailable; explicit selection has no fallback")]
    Unavailable(PhysicalAdapterId),
    /// Multiple current adapters must not be resolved by enumeration order.
    #[error("selected physical GPU {0:?} has ambiguous runtime identities")]
    Ambiguous(PhysicalAdapterId),
    /// An automatic selection cannot be pinned without a physical identity.
    #[error("GPU {0:?} physical identity is unavailable (HRESULT {1:#x})")]
    IdentityUnavailable(AdapterId, i32),
}

/// Resolves an automatic preference or exact explicit identity without creating a device.
/// Hosts must retain the returned identity through renderer recreation and source recovery.
pub fn select_adapter(preference: AdapterPreference) -> Result<AdapterInfo, AdapterSelectionError> {
    let available = match &preference {
        AdapterPreference::Automatic => preferred_adapters()?,
        AdapterPreference::Explicit(_) => adapters()?,
    };
    select_from(&available, preference).cloned()
}

/// Resolve an already observed inventory with the same exact-device and ambiguity policy.
/// Hosts may reuse their coarse catalog without issuing native queries per frame.
pub fn select_from(
    available: &[AdapterInfo],
    preference: AdapterPreference,
) -> Result<&AdapterInfo, AdapterSelectionError> {
    let mut hardware = available.iter().filter(|adapter| !adapter.software);
    match preference {
        AdapterPreference::Automatic => {
            let selected = hardware.next().ok_or(AdapterSelectionError::NoHardware)?;
            selected
                .physical_id
                .as_ref()
                .map_err(|error| AdapterSelectionError::IdentityUnavailable(selected.id, *error))?;
            Ok(selected)
        }
        AdapterPreference::Explicit(key) => {
            let mut matches = hardware.filter(|adapter| adapter.physical_id.as_ref().ok() == Some(&key));
            let selected = matches
                .next()
                .ok_or_else(|| AdapterSelectionError::Unavailable(key.clone()))?;
            if matches.next().is_some() {
                return Err(AdapterSelectionError::Ambiguous(key));
            }
            Ok(selected)
        }
    }
}

impl AdapterId {
    /// Runtime-only LUID token for observations, never a persistent GPU preference.
    /// Windows assigns a new token after device re-enumeration.
    pub fn key(self) -> String {
        format!("dxgi:{:08x}:{:08x}", self.high as u32, self.low)
    }

    /// Parses the canonical token without accepting labels, indices or partial identities.
    pub fn from_key(value: &str) -> Option<Self> {
        let value = value.strip_prefix("dxgi:")?;
        let (high, low) = value.split_once(':')?;
        if high.len() != 8 || low.len() != 8 || !high.bytes().chain(low.bytes()).all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some(Self {
            high: u32::from_str_radix(high, 16).ok()? as i32,
            low: u32::from_str_radix(low, 16).ok()?,
        })
    }
}

#[cfg(test)]
mod tests;
