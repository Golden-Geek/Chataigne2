#[cfg(feature = "desktop")]
mod cpal;
mod mock;
mod null;
mod traits;

#[cfg(feature = "desktop")]
pub use cpal::{
    NativeAudioBackendInfo, compiled_cpal_backend_catalog, compiled_cpal_backends, cpal_backend_by_id,
    probe_cpal_backends,
};
pub use mock::{MockBackend, MockBackendControl, MockBackendEvent, MockBackendEventKind};
pub use null::NullBackend;
pub use traits::{
    AudioBackend, AudioCallbackTimestamp, AudioStream, AudioStreamHandler, BackendDescriptor, BackendPolicy,
    StreamRequest,
};
