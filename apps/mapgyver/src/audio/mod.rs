//! MapGyver owns authored output routing; Golden Audio owns the streams and rendering.
mod configuration;
#[cfg(feature = "video-files")]
mod playback;
mod preferences;
mod runtime;
pub(crate) mod voice;

pub use preferences::{AudioPreferencesNode, OutputNode};
pub(crate) use preferences::{inspect, tree};
pub(crate) use runtime::start;

pub(crate) const NAMESPACE: &str = "mapgyver.audio";

#[cfg(test)]
mod tests;
