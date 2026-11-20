use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Representation requested for a browser preview frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum UiBrowserPreviewQuality {
    /// Reduced resolution, JPEG quality and frame cadence.
    Low,
    /// Full browser preview resolution and cadence.
    High,
}

/// Renews demand and requests the latest matching representation.
#[derive(Debug, Serialize, Deserialize, TS)]
pub struct UiBrowserPreviewRequest {
    /// Opaque app-defined preview source key.
    pub source: String,
    /// One of the available encoded representations.
    pub quality: UiBrowserPreviewQuality,
    /// ETag of the frame already displayed, if any.
    pub after: Option<String>,
    /// Return the image even if unchanged so the browser can sample transfer time.
    #[serde(default)]
    pub refresh: bool,
}

/// Intrinsic content size associated with a frame, before browser quality downsampling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiBrowserPreviewExtent {
    /// Source width in pixels.
    pub width: u32,
    /// Source height in pixels.
    pub height: u32,
}

/// Geometry of a source used by a composited preview, from the same installed render plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UiBrowserPreviewDependency {
    /// Opaque app-defined identity of the dependent source.
    pub source: String,
    /// Intrinsic dependent image size.
    pub extent: UiBrowserPreviewExtent,
}
