mod mock;
mod null;
mod traits;

pub use mock::{MockBackend, MockBackendControl};
pub use null::NullBackend;
pub use traits::{AudioBackend, AudioStream, BackendDescriptor, BackendPolicy, StreamRequest};
