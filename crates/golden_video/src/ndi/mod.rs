//! Worker-owned discovery and frame capture through the optional Windows NDI 6 runtime.

use std::ffi::{CStr, CString};
use std::ptr::{NonNull, null};

use ndi_sdk::sys;
use thiserror::Error;

use crate::image::ImageError;

mod audio;
mod receiver;
mod runtime;
#[cfg(test)]
mod tests;

pub use audio::NdiAudioFrame;
pub use receiver::{NdiFrame, NdiReceiver, NdiTiming, NdiVideoFrame};
use runtime::Runtime;

/// A missing SDK or a failed NDI operation. Ordinary rendering does not depend on this runtime.
#[derive(Debug, Error)]
pub enum NdiError {
    /// The runtime could not be loaded or initialized.
    #[error("Install or update the NDI 6 Runtime for Windows x64: {0}")]
    Runtime(String),
    /// The initialized SDK could not create an endpoint.
    #[error("NDI could not create {0}")]
    Endpoint(&'static str),
    /// The receiver reported an interruption.
    #[error("NDI receiver lost its connection")]
    Disconnected,
    /// The SDK returned a frame that cannot be admitted to CPU image storage.
    #[error("Invalid NDI video frame: {0}")]
    Frame(&'static str),
    /// Audio layout is incompatible with bounded planar capture.
    #[error("Invalid NDI audio frame: {0}")]
    AudioFrame(&'static str),
    /// The copied frame failed validation as a Golden Video image.
    #[error(transparent)]
    Image(#[from] ImageError),
}

/// Owned discovery entry. Persist the name and use a fresh entry when opening a connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NdiSource {
    name: String,
    address: Option<CString>,
}

impl NdiSource {
    /// Exact SDK name, including the sender's machine qualifier.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// An NDI finder that stays on the worker that created it.
pub struct NdiDiscovery {
    runtime: &'static Runtime,
    instance: NonNull<sys::NDIlib_find_instance_type>,
}

impl NdiDiscovery {
    /// Initialize local and network discovery on the calling worker.
    pub fn new() -> Result<Self, NdiError> {
        let runtime = Runtime::get()?;
        let settings = sys::NDIlib_find_create_t {
            show_local_sources: true,
            p_groups: null(),
            p_extra_ips: null(),
        };
        // The SDK copies the settings. All used function pointers were validated at load.
        let instance = unsafe { runtime.api.find_create_v2.unwrap()(&settings) };
        let instance = NonNull::new(instance).ok_or(NdiError::Endpoint("a discovery instance"))?;
        Ok(Self { runtime, instance })
    }

    /// Copy the SDK's current catalog without waiting for discovery traffic.
    pub fn sources(&mut self) -> Vec<NdiSource> {
        let mut count = 0;
        // The array and its strings stay valid until the next query or finder destruction.
        let sources = unsafe { self.runtime.api.find_get_current_sources.unwrap()(self.instance.as_ptr(), &mut count) };
        if sources.is_null() || count == 0 {
            return Vec::new();
        }
        let sources = unsafe { std::slice::from_raw_parts(sources, count.min(4096) as usize) };
        sources
            .iter()
            .filter_map(|source| {
                if source.p_ndi_name.is_null() {
                    return None;
                }
                let name = unsafe { CStr::from_ptr(source.p_ndi_name) }.to_str().ok()?;
                if name.is_empty() {
                    return None;
                }
                let address = unsafe { source.__bindgen_anon_1.p_url_address };
                let address = (!address.is_null()).then(|| unsafe { CStr::from_ptr(address) }.to_owned());
                Some(NdiSource {
                    name: name.to_owned(),
                    address,
                })
            })
            .collect()
    }
}

impl Drop for NdiDiscovery {
    fn drop(&mut self) {
        // The live finder is uniquely owned by this value.
        unsafe { self.runtime.api.find_destroy.unwrap()(self.instance.as_ptr()) };
    }
}
