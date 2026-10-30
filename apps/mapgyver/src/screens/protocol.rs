//! Single authored contract for backend operations, persistence and generated UI consumers.

use golden_core::model::NodeUuid;
use golden_video::Extent2D;
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

use crate::media::protocol::ChannelReference;
pub use golden_video::plan::{Point, Quad, UnitRect};

pub const SCREEN_NAMESPACE: &str = "mapgyver.screens";
pub const SCREEN_OPERATION: &str = "edit";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct ScreenId(pub NodeUuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct SurfaceId(pub NodeUuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct SurfaceReference {
    pub screen: ScreenId,
    pub surface: SurfaceId,
}

/// Physical desktop coordinates, deliberately independent of CSS and OS logical units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PhysicalRectangle {
    pub left: i32,
    pub top: i32,
    pub extent: Extent2D,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ScreenTarget {
    /// Authored identity survives a disconnected display; never substitute the primary display.
    Fullscreen {
        display: String,
    },
    Rectangle {
        bounds: PhysicalRectangle,
    },
    Spout {
        sender: String,
        extent: Extent2D,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(default)]
pub struct ScreenDefinition {
    /// An unconfigured Screen must not create a window or select a sender implicitly.
    pub target: Option<ScreenTarget>,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
pub enum Calibration {
    #[default]
    Off,
    Grid,
    ColorBars,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct SurfaceDefinition {
    /// Kept while calibration is active and while the source is missing/deleted.
    pub source: Option<ChannelReference>,
    pub crop: UnitRect,
    /// Ordered top-left, top-right, bottom-right, bottom-left in normalized Screen coordinates.
    pub quad: Quad,
    pub opacity: f32,
    pub calibration: Calibration,
    pub enabled: bool,
}

impl Default for SurfaceDefinition {
    fn default() -> Self {
        Self {
            source: None,
            crop: UnitRect::default(),
            quad: Quad::default(),
            opacity: 1.0,
            calibration: Calibration::Off,
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ScreenEdit {
    InspectManager,
    /// Backend allocates identities, labels and an initial full-screen Surface in one subtree.
    CreateScreen {
        label: Option<String>,
    },
    ConfigureScreen {
        screen: ScreenId,
        definition: ScreenDefinition,
    },
    RenameScreen {
        screen: ScreenId,
        label: String,
    },
    ReorderScreen {
        screen: ScreenId,
        after: Option<ScreenId>,
    },
    RemoveScreen {
        screen: ScreenId,
    },
    AddSurface {
        screen: ScreenId,
        label: Option<String>,
    },
    ConfigureSurface {
        reference: SurfaceReference,
        definition: SurfaceDefinition,
    },
    RenameSurface {
        reference: SurfaceReference,
        label: String,
    },
    ReorderSurface {
        reference: SurfaceReference,
        after: Option<SurfaceId>,
    },
    RemoveSurface {
        reference: SurfaceReference,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct SurfaceSnapshot {
    pub reference: SurfaceReference,
    pub label: String,
    pub definition: SurfaceDefinition,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ScreenSnapshot {
    pub id: ScreenId,
    pub label: String,
    pub definition: ScreenDefinition,
    pub surfaces: Vec<SurfaceSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ScreenManagerSnapshot {
    pub screens: Vec<ScreenSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ScreenEditResult {
    Manager {
        snapshot: ScreenManagerSnapshot,
    },
    ScreenCreated {
        screen: ScreenId,
        surface: SurfaceReference,
    },
    SurfaceCreated {
        reference: SurfaceReference,
    },
    Changed,
}

impl ScreenDefinition {
    pub fn validate(&self) -> Result<(), String> {
        match &self.target {
            Some(ScreenTarget::Fullscreen { display }) => text(display, 1024, "display identity")?,
            Some(ScreenTarget::Rectangle { bounds }) => {
                extent(bounds.extent)?;
                bounds
                    .left
                    .checked_add_unsigned(bounds.extent.width())
                    .ok_or("rectangle right edge overflows physical coordinates")?;
                bounds
                    .top
                    .checked_add_unsigned(bounds.extent.height())
                    .ok_or("rectangle bottom edge overflows physical coordinates")?;
            }
            Some(ScreenTarget::Spout { sender, extent: size }) => {
                // Golden's versioned frame counter reserves 16 bytes of the SDK name limit.
                text(sender, 239, "Spout sender")?;
                extent(*size)?;
            }
            None => {}
        }
        Ok(())
    }
}

impl SurfaceDefinition {
    pub fn validate(&self) -> Result<(), String> {
        golden_video::plan::ProjectiveMapping::new(self.crop, self.quad).map_err(|error| error.to_string())?;
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err("surface opacity must be finite and in 0..1".into());
        }
        Ok(())
    }
}

fn text(value: &str, maximum: usize, name: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > maximum || value.contains('\0') {
        return Err(format!("{name} must contain 1..{maximum} UTF-8 bytes without NUL"));
    }
    Ok(())
}

fn extent(value: Extent2D) -> Result<(), String> {
    if value.width() > 8192 || value.height() > 8192 {
        return Err("screen extent exceeds the 8192-pixel render limit".into());
    }
    Ok(())
}

pub fn export_typescript(directory: impl AsRef<std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
    let directory = directory.as_ref();
    let config = Config::default().with_out_dir(directory).with_large_int("number");
    ScreenEdit::export_all(&config)?;
    ScreenEditResult::export_all(&config)?;
    std::fs::write(
        directory.join("constants.ts"),
        format!(
            "// Generated by generate_screen_protocol; do not edit.\nexport const SCREEN_NAMESPACE = {SCREEN_NAMESPACE:?};\nexport const SCREEN_OPERATION = {SCREEN_OPERATION:?};\n"
        ),
    )?;
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "ts") {
            let text = std::fs::read_to_string(&path)?;
            let normalized = text.lines().map(str::trim_end).collect::<Vec<_>>().join("\n");
            std::fs::write(path, format!("{normalized}\n"))?;
        }
    }
    Ok(())
}
