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
}
