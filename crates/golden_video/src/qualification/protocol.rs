//! Canonical metadata for the held, app-neutral native qualification fixture.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::preview::protocol::PreviewView;

/// Native artifact contract, including explicit isolated-source send ownership waits.
pub const CAPTURE_SCHEMA_VERSION: u32 = 8;

/// Per-lock wait on isolated 60 Hz fixture producers; renderer outputs continue to pass zero.
/// Two locks may each wait this long, excluding opaque SDK/driver work.
pub const SOURCE_SEND_ACCESS_TIMEOUT_MS: u32 = 2;

/// Host-to-fixture CSS layout updates, computed from authenticated native metrics.
pub const QUALIFICATION_LAYOUT_EVENT: &str = "golden-qualification-layout";
/// Host-to-fixture lifecycle commands; never a per-frame event.
pub const QUALIFICATION_COMMAND_EVENT: &str = "golden-qualification-command";
/// Fixture-to-host lifecycle and measured-geometry observations.
pub const QUALIFICATION_REPORT_EVENT: &str = "golden-qualification-fixture";

/// One opaque source registration and its requested CSS size, not physical-pixel evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QualificationView {
    /// Consumer-owned source routing key, with no app identity or native handle semantics.
    pub source: String,
    /// CSS width supplied by the host after applying its actual scale.
    pub css_width: f64,
    /// CSS height supplied by the host after applying its actual scale.
    pub css_height: f64,
}

/// Replaces the fixed pair of fixture views. The host owns physical-size validation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct QualificationLayout {
    /// Strictly increasing layout identity; obsolete host updates are ignored.
    pub revision: u32,
    /// Exactly two independently sized preview registrations.
    pub views: [QualificationView; 2],
}

/// Explicit fixture actions; held layouts never remove themselves on a timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum QualificationAction {
    /// Remeasure and report the current installed registration state.
    Ready,
    /// Hide both registrations while retaining their DOM layout and source keys.
    Hide,
    /// Restore the held registrations after hiding them.
    Show,
    /// Clear and disconnect the preview session, retaining DOM controls.
    Disconnect,
}

/// One host-requested lifecycle action with a monotonically increasing identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct QualificationCommand {
    /// Strictly increasing command identity used to correlate reports.
    pub id: u32,
    /// Requested action.
    pub action: QualificationAction,
}

/// Bounded lifecycle phase, independent of video frame delivery or presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum QualificationPhase {
    /// Host metadata listeners are installed; a layout may still be needed.
    Listening,
    /// The current layout has a matching native installation acknowledgment.
    Ready,
    /// Both held registrations are hidden and the clear was acknowledged.
    Hidden,
    /// Held registrations were restored and installation was acknowledged.
    Shown,
    /// Session disposal acknowledged a clear and completed disconnection.
    Disconnected,
    /// Startup, validation or installation failed; inspect the bounded error.
    Failed,
}

/// Lifecycle report only. Measured CSS rectangles require host metrics for physical validation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QualificationReport {
    /// Increasing report identity, unrelated to render frames.
    pub sequence: u32,
    /// Host command represented by this report, absent for layout/startup observations.
    pub command_id: Option<u32>,
    /// Current fixture lifecycle phase.
    pub phase: QualificationPhase,
    /// Latest applied host layout, if one has been provided.
    pub layout: Option<QualificationLayout>,
    /// Whether native installation/clearing was acknowledged for the current registration state.
    pub installed: bool,
    /// Host-acknowledged visible registration count.
    pub visible_views: u32,
    /// Last complete submitted CSS registration set, including hidden views.
    pub views: Vec<PreviewView>,
    /// Retirement masks currently retained in the DOM.
    pub retirement_masks: u32,
    /// Highest simultaneously observed mask count over this fixture lifetime.
    pub max_retirement_masks: u32,
    /// DOM-overlay button activations for the two stable views.
    pub overlay_clicks: [u32; 2],
    /// Bounded startup/installation error; absent for successful observations.
    pub error: Option<String>,
}

/// Generates the fixture types from their Rust declarations and canonical event names.
pub fn export_typescript(directory: impl AsRef<std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    let directory = directory.as_ref();
    let config = ts_rs::Config::default()
        .with_out_dir(directory)
        .with_large_int("number");
    QualificationLayout::export_all(&config)?;
    QualificationCommand::export_all(&config)?;
    QualificationReport::export_all(&config)?;
    std::fs::write(
        directory.join("constants.ts"),
        format!(
            "// Generated by generate_qualification_protocol; do not edit.\n\
             export const QUALIFICATION_LAYOUT_EVENT = {QUALIFICATION_LAYOUT_EVENT:?};\n\
             export const QUALIFICATION_COMMAND_EVENT = {QUALIFICATION_COMMAND_EVENT:?};\n\
             export const QUALIFICATION_REPORT_EVENT = {QUALIFICATION_REPORT_EVENT:?};\n"
        ),
    )?;
    std::fs::write(
        directory.join("capture-schema.json"),
        format!(
            "{{\"capture_schema_version\":{CAPTURE_SCHEMA_VERSION},\"source_send_access_timeout_ms\":{SOURCE_SEND_ACCESS_TIMEOUT_MS}}}\n"
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
