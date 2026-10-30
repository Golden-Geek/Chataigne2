//! Worker-local sender enumeration. Discovery never opens a graphics device or a receiver.

use std::{collections::BTreeSet, ffi::c_char, num::NonZeroUsize, ptr::NonNull};

use spout2_sys as sdk;
use thiserror::Error;

/// Absolute bound on one SDK enumeration, independent of the registry's advertised count.
pub const MAX_DISCOVERY_SENDERS: usize = 256;

/// Bounded exact sender names, observed independently of authored subscriptions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpoutSenderCatalog {
    /// Exact UTF-8 names, sorted and deduplicated without lossy conversion.
    pub names: Vec<String>,
    /// Some entries were omitted by the bound, invalid encoding or registry churn.
    pub truncated: bool,
}

/// Failure before a catalog can be observed.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DiscoveryError {
    /// The SDK could not allocate an enumeration instance.
    #[error("Spout sender discovery is unavailable")]
    Unavailable,
    /// Callers must choose a finite bound within the public maximum.
    #[error("sender discovery limit exceeds 256 entries")]
    Limit,
}

/// An SDK instance owned and destroyed on its calling worker. No COM/texture handles escape.
pub struct SpoutDiscovery {
    raw: NonNull<sdk::spout_dx_t>,
}

impl SpoutDiscovery {
    /// Creates an enumeration instance without opening DirectX or selecting a sender.
    pub fn new() -> Result<Self, DiscoveryError> {
        // SAFETY: this uniquely owned instance stays on the constructing worker.
        let raw = NonNull::new(unsafe { sdk::spout_dx_create() }).ok_or(DiscoveryError::Unavailable)?;
        Ok(Self { raw })
    }

    /// Enumerates at most `maximum` names. Call off UI/engine/render submission threads:
    /// registry access can take an SDK mutex, and the registry may change during enumeration.
    pub fn snapshot(&mut self, maximum: NonZeroUsize) -> Result<SpoutSenderCatalog, DiscoveryError> {
        collect(
            maximum,
            // SAFETY: worker-local live instance; discovery requires no graphics initialization.
            || unsafe { sdk::spout_dx_get_sender_count(self.raw.as_ptr()) },
            |index, buffer| unsafe {
                sdk::spout_dx_get_sender_name_at(self.raw.as_ptr(), index, buffer.as_mut_ptr(), buffer.len() as i32)
            },
        )
    }
}

impl Drop for SpoutDiscovery {
    fn drop(&mut self) {
        // SAFETY: only this owner can destroy the worker-local instance.
        unsafe { sdk::spout_dx_destroy(self.raw.as_ptr()) };
    }
}

fn collect(
    maximum: NonZeroUsize,
    count: impl FnOnce() -> i32,
    mut name_at: impl FnMut(i32, &mut [c_char; 256]) -> i32,
) -> Result<SpoutSenderCatalog, DiscoveryError> {
    if maximum.get() > MAX_DISCOVERY_SENDERS {
        return Err(DiscoveryError::Limit);
    }
    let count = usize::try_from(count()).map_err(|_| DiscoveryError::Unavailable)?;
    let mut names = BTreeSet::new();
    let mut truncated = count > maximum.get();
    for index in 0..count.min(maximum.get()) {
        let mut buffer = [0; 256];
        let length = name_at(index as i32, &mut buffer);
        let bytes = buffer.map(|byte| byte as u8);
        let name = usize::try_from(length)
            .ok()
            .filter(|&length| length > 0 && length < bytes.len())
            .filter(|&length| bytes[length] == 0 && !bytes[..length].contains(&0))
            .and_then(|length| std::str::from_utf8(&bytes[..length]).ok());
        if let Some(name) = name {
            names.insert(name.to_owned());
        } else {
            truncated = true;
        }
    }
    Ok(SpoutSenderCatalog {
        names: names.into_iter().collect(),
        truncated,
    })
}

#[cfg(test)]
mod tests;
