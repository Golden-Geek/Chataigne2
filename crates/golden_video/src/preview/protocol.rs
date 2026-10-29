//! Metadata-only local preview negotiation. Runtime routes/leases are never project data.
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

/// Current local native-preview negotiation/control schema version.
pub const PREVIEW_PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
/// Rectangle in WebView client CSS pixels; scaling and native clipping belong to the host.
pub struct CssRect {
    /// Left edge, which may be negative for partially scrolled/clipped elements.
    pub x: f64,
    /// Top edge in the same client coordinate system.
    pub y: f64,
    /// Nonnegative CSS width; zero contributes no visible region.
    pub width: f64,
    /// Nonnegative CSS height; zero contributes no visible region.
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
/// One DOM preview registration, containing metadata only and no frame bytes/native handles.
pub struct PreviewView {
    /// Ephemeral view identity, unique within a submitted layout.
    pub id: Uuid,
    /// Opaque consumer-owned source key, never a GPU handle or app-domain declaration.
    pub source: String,
    /// Full preview element bounds before native clipping.
    pub rect: CssRect,
    /// Intersection of visible DOM clipping bounds in the same client CSS coordinates.
    pub clip: CssRect,
    /// Whether the view currently contributes native preview demand.
    pub visible: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
/// Local preview negotiation and revisioned layout/control requests.
pub enum PreviewRequest {
    /// Negotiates a session for one frontend document/client lifetime.
    Connect {
        /// Random identity replaced when the frontend document owner is replaced.
        client: Uuid,
        /// Schema version supported by that frontend.
        version: u32,
    },
    /// Replaces the complete desired layout and refreshes its liveness lease.
    Layout {
        /// Negotiated host-issued session identity.
        session: Uuid,
        /// Strictly increasing revision within that session.
        revision: u32,
        /// Complete ordered registration set; omitted views become hidden.
        views: Vec<PreviewView>,
    },
    /// Reads installation state for the exact current layout revision.
    Status {
        /// Negotiated session whose installation is being queried.
        session: Uuid,
        /// Exact accepted layout revision; obsolete queries are rejected.
        revision: u32,
    },
    /// Releases the session and hides its registered views.
    Disconnect {
        /// Session being released; another document's session cannot be disconnected.
        session: Uuid,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
/// Bounded reason that a preview control request could not be accepted.
pub enum PreviewRejection {
    /// Frontend and host protocol versions differ.
    Version,
    /// The presentation control owner has permanently closed.
    Closed,
    /// Session identity is missing, expired or belongs to another document.
    Session,
    /// Layout revision is obsolete or does not match the current status target.
    Revision,
    /// Submitted registration count exceeds fixed admission capacity.
    Capacity,
    /// Host or CSS geometry is invalid or outside supported coordinate bounds.
    Geometry,
    /// Opaque source key is empty, oversized or contains NUL.
    Source,
    /// A layout contains duplicate view identities.
    Duplicate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
/// Metadata-only negotiation, layout/status acknowledgement or rejection.
pub enum PreviewResponse {
    /// Session negotiation succeeded; native frames are not yet acknowledged.
    Connected {
        /// Host-issued session identity used by subsequent requests.
        session: Uuid,
        /// Host-supported preview control schema version.
        version: u32,
        /// Fixed maximum submitted views per layout, including hidden views.
        max_views: u32,
    },
    /// Layout accepted or status read for the exact requested revision.
    Applied {
        /// Frontend layout revision represented by this acknowledgement.
        revision: u32,
        /// Number of nonempty visible registrations after host clipping.
        visible_views: u32,
        /// Worker verified reliable installation and a matching submitted Video frame.
        /// This does not prove scanout, delivery, physical alignment or source availability.
        installed: bool,
    },
    /// The requested session was released and its views invalidated.
    Disconnected,
    /// The request was rejected; the frontend must not reveal an unacknowledged native hole.
    Rejected {
        /// Bounded failure reason for caller recovery/availability presentation.
        code: PreviewRejection,
    },
}

/// Exports canonical preview request/response types and version constants for local UI consumers.
pub fn export_typescript(directory: impl AsRef<std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    let directory = directory.as_ref();
    let config = ts_rs::Config::default()
        .with_out_dir(directory)
        .with_large_int("number");
    PreviewRequest::export_all(&config)?;
    PreviewResponse::export_all(&config)?;
    std::fs::write(
        directory.join("constants.ts"),
        format!(
            "// Generated by generate_preview_protocol; do not edit.\nexport const PREVIEW_PROTOCOL_VERSION = {PREVIEW_PROTOCOL_VERSION};\n"
        ),
    )?;
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "ts") {
            let text = std::fs::read_to_string(&path)?;
            let text = text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
            std::fs::write(path, format!("{text}\n"))?;
        }
    }
    Ok(())
}
