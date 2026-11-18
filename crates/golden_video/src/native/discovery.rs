//! Worker-local sender enumeration. Discovery never opens a graphics device or a receiver.

use std::{
    collections::BTreeSet,
    ffi::{CStr, c_char},
    num::NonZeroUsize,
    ptr::{self, NonNull},
};

use spout2_sys as sdk;
use thiserror::Error;

use super::provenance::{self, SenderIdentity};
use super::spout_layout::legacy_shared_handle;

/// Absolute bound on one SDK enumeration, independent of the registry's advertised count.
pub const MAX_DISCOVERY_SENDERS: usize = 256;

/// Bounded exact sender names, observed independently of authored subscriptions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpoutSenderCatalog {
    /// Exact UTF-8 names, deduplicated in SDK enumeration order.
    pub names: Vec<String>,
    /// Subset of names whose native registrations match this process's renderer outputs.
    pub internal_names: Vec<String>,
    /// Some entries were omitted by the bound, invalid metadata or publication churn.
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
    /// registry access can take an SDK mutex. Names and handles are a coarse observation of
    /// the registry. If a local output publishes or retires during the observation, this
    /// snapshot is empty and truncated so an unconfirmed sender is never marked external.
    pub fn snapshot(&mut self, maximum: NonZeroUsize) -> Result<SpoutSenderCatalog, DiscoveryError> {
        let revision = provenance::revision();
        let (senders, truncated) = collect(
            maximum,
            // SAFETY: worker-local live instance; discovery requires no graphics initialization.
            || unsafe { sdk::spout_dx_get_sender_count(self.raw.as_ptr()) },
            |index, buffer| unsafe {
                sdk::spout_dx_get_sender_name_at(self.raw.as_ptr(), index, buffer.as_mut_ptr(), buffer.len() as i32)
            },
            |name| {
                let mut handle = ptr::null_mut();
                // SAFETY: bounded UTF-8 C string, worker-local instance and valid output pointer.
                let found = unsafe {
                    sdk::spout_dx_get_sender_info(
                        self.raw.as_ptr(),
                        name.as_ptr(),
                        ptr::null_mut(),
                        ptr::null_mut(),
                        &mut handle,
                        ptr::null_mut(),
                    )
                };
                if found == 0 {
                    None
                } else {
                    legacy_shared_handle(handle as usize)
                }
            },
        )?;
        let Some(internal_names) = provenance::internal_names(revision, &senders) else {
            return Ok(SpoutSenderCatalog {
                names: Vec::new(),
                internal_names: Vec::new(),
                truncated: true,
            });
        };
        Ok(SpoutSenderCatalog {
            names: senders.into_iter().map(|sender| sender.name).collect(),
            internal_names,
            truncated,
        })
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
    mut sender_handle: impl FnMut(&CStr) -> Option<u32>,
) -> Result<(Vec<SenderIdentity>, bool), DiscoveryError> {
    if maximum.get() > MAX_DISCOVERY_SENDERS {
        return Err(DiscoveryError::Limit);
    }
    let count = usize::try_from(count()).map_err(|_| DiscoveryError::Unavailable)?;
    let mut names = BTreeSet::new();
    let mut senders = Vec::new();
    let mut truncated = count > maximum.get();
    for index in 0..count.min(maximum.get()) {
        let mut buffer = [0; 256];
        let length = name_at(index as i32, &mut buffer);
        let bytes = buffer.map(|byte| byte as u8);
        let name = usize::try_from(length)
            .ok()
            .filter(|&length| length > 0 && length < bytes.len())
            .and_then(|length| CStr::from_bytes_with_nul(&bytes[..=length]).ok())
            .and_then(|c_name| c_name.to_str().ok().map(|name| (c_name, name)));
        let Some((c_name, name)) = name else {
            truncated = true;
            continue;
        };
        if !names.insert(name.to_owned()) {
            continue;
        }
        let Some(shared_handle) = sender_handle(c_name) else {
            truncated = true;
            continue;
        };
        senders.push(SenderIdentity {
            name: name.to_owned(),
            shared_handle,
        });
    }
    Ok((senders, truncated))
}

#[cfg(test)]
mod tests;
