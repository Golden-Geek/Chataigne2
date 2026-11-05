//! Single authored contract for backend operations, persistence and generated UI consumers.

use golden_core::model::NodeUuid;
use golden_video::Extent2D;
use serde::{Deserialize, Serialize};
use ts_rs::{Config, TS};

use crate::media::protocol::ChannelReference;
pub use golden_video::plan::{Point, Quad, UnitRect};

pub const SCREEN_NAMESPACE: &str = "mapgyver.screens";
pub const SCREEN_OPERATION: &str = "edit";
pub const DISPLAY_CATALOG_EVENT: &str = "mapgyver-displays";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DisplayInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub edid: Option<String>,
    /// Current OS GDI display number; zero means unavailable.
    #[serde(default)]
    pub index: u32,
    #[serde(default)]
    pub primary: bool,
    pub bounds: PhysicalRectangle,
}

/// Authored destination evidence, retained unchanged while runtime routing recovers elsewhere.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct DisplayIdentityHint {
    pub identity: String,
    pub edid: Option<String>,
    pub name: String,
    /// Last known OS GDI number, or zero when unknown.
    pub index: u32,
    pub bounds: Option<PhysicalRectangle>,
}

impl From<&DisplayInfo> for DisplayIdentityHint {
    fn from(display: &DisplayInfo) -> Self {
        Self {
            identity: display.id.clone(),
            edid: display.edid.clone(),
            name: display.name.clone(),
            index: display.index,
            bounds: Some(display.bounds),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct DisplayCatalogSnapshot {
    pub revision: u32,
    pub displays: Vec<DisplayInfo>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ScreenNativeRequest {
    #[serde(rename = "mapgyverDisplays")]
    Displays { refresh: bool },
}

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct ScreenDefinition {
    /// An unconfigured Screen must not create a window or select a sender implicitly.
    pub target: Option<ScreenTarget>,
    pub enabled: bool,
}

impl Default for ScreenDefinition {
    fn default() -> Self {
        Self {
            target: Some(ScreenTarget::Spout {
                sender: "Screen".into(),
                extent: Extent2D::new(1920, 1080).expect("default extent"),
            }),
            enabled: true,
        }
    }
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
    InspectEditor {
        query: ScreenEditorQuery,
    },
    /// Bounded metadata for one mounted outliner row; never includes descendants.
    InspectRow {
        node: NodeUuid,
    },
    InspectManager,
    /// Bounded live editor projection, independent of the full manager catalog.
    InspectSurface {
        reference: SurfaceReference,
    },
    InspectScreen {
        screen: ScreenId,
    },
    /// Backend allocates identities, labels and an initial full-screen Surface in one subtree.
    CreateScreen {
        label: Option<String>,
    },
    ConfigureScreen {
        screen: ScreenId,
        definition: ScreenDefinition,
    },
    /// Disable a live output without allowing a retired window/project to affect a new target.
    DisableOutput {
        #[ts(type = "string")]
        project: uuid::Uuid,
        screen: ScreenId,
        target: ScreenTarget,
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
    /// Resolve a Media/Channel node and atomically create a default Surface or replace its source.
    DropSource {
        screen: ScreenId,
        surface: Option<SurfaceId>,
        #[ts(type = "string")]
        source: NodeUuid,
    },
    /// Resolve the complete Screen's topmost Surface at drop time, independent of editor paging.
    DropSourceAt {
        screen: ScreenId,
        point: Point,
        source: NodeUuid,
    },
    ConfigureSurface {
        reference: SurfaceReference,
        definition: SurfaceDefinition,
    },
    /// Applies explicit normalized geometry input to the current backend definition.
    EditSurfaceGeometry {
        reference: SurfaceReference,
        edit: SurfaceGeometryEdit,
    },
    /// Backend computes the quad from current output/source dimensions; other settings stay authored.
    FitSurface {
        reference: SurfaceReference,
        mode: SurfaceFitMode,
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

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum SurfaceGeometryEdit {
    /// Translate a starting quad by the current pointer displacement, without accumulating samples.
    QuadTranslation {
        quad: Quad,
        origin: Point,
        point: Point,
    },
    /// Cumulative output-pixel input from an explicit starting corner coordinate.
    /// Coalescing newer samples does not lose repeated keys; viewport pixels are never used.
    QuadCornerPixels {
        corner: u8,
        origin: Point,
        pixels: PixelOffset,
    },
    /// Corner indices are top-left, top-right, bottom-right, bottom-left.
    QuadCorner {
        corner: u8,
        point: Point,
    },
    CropCorner {
        corner: u8,
        point: Point,
    },
    CropOrigin {
        point: Point,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PixelOffset {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub enum SurfaceFitMode {
    Fill,
    Fit,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct SurfaceSnapshot {
    pub reference: SurfaceReference,
    pub label: String,
    pub definition: SurfaceDefinition,
    /// Presentation status only; invalid intermediate warps remain authored and editable.
    pub mapping_valid: bool,
}

impl SurfaceSnapshot {
    pub(super) fn new(reference: SurfaceReference, label: String, definition: SurfaceDefinition) -> Self {
        let mapping_valid = golden_video::plan::ProjectiveMapping::new(definition.crop, definition.quad).is_ok();
        Self {
            reference,
            label,
            definition,
            mapping_valid,
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum ScreenEditorKind {
    Screen,
    Surface,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ScreenEditorTarget {
    Screen { screen: ScreenId },
    Surface { reference: SurfaceReference },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub struct ScreenEditorQuery {
    pub editor: ScreenEditorKind,
    pub selected: Option<NodeUuid>,
    pub document: Option<NodeUuid>,
    pub locked: Option<ScreenEditorTarget>,
    pub retained: Option<ScreenEditorTarget>,
    pub surface: Option<SurfaceId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct ScreenEditorPage {
    pub document: NodeUuid,
    pub document_matches: bool,
    pub target: Option<ScreenEditorTarget>,
    /// First 128 Surface definitions; native rendering retains the complete Screen.
    pub screen: Option<ScreenSnapshot>,
    pub has_more_surfaces: bool,
    /// Retained identity remains available for Undo even when its definition is missing.
    pub selected_surface: Option<SurfaceId>,
    /// Independent lookup keeps off-page selected geometry editable.
    pub surface: Option<SurfaceSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ScreenRowSnapshot {
    pub node: NodeUuid,
    pub detail: String,
    pub extent: Option<Extent2D>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ScreenEditResult {
    Editor {
        page: ScreenEditorPage,
    },
    Row {
        /// None when deletion/project replacement overtook a mounted row's read.
        snapshot: Option<ScreenRowSnapshot>,
    },
    Manager {
        snapshot: ScreenManagerSnapshot,
    },
    Surface {
        snapshot: SurfaceSnapshot,
    },
    ScreenDefinition {
        screen: ScreenId,
        definition: ScreenDefinition,
    },
    ScreenCreated {
        screen: ScreenId,
        surface: SurfaceReference,
    },
    SurfaceCreated {
        reference: SurfaceReference,
    },
    SurfaceChanged {
        snapshot: SurfaceSnapshot,
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
                text(sender, 239, "Shared Texture sender")?;
                extent(*size)?;
            }
            None => {}
        }
        Ok(())
    }
}

impl SurfaceDefinition {
    pub fn validate(&self) -> Result<(), String> {
        if !self.crop.is_valid() {
            return Err("surface crop must have positive extents contained in 0..1".into());
        }
        // Authored corners must survive crossed, concave and collapsed intermediate states.
        // Executable homographies are checked separately at the rendering boundary.
        if self
            .quad
            .corners
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err("surface corner positions must be finite".into());
        }
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
    DisplayCatalogSnapshot::export_all(&config)?;
    DisplayIdentityHint::export_all(&config)?;
    ScreenNativeRequest::export_all(&config)?;
    std::fs::write(
        directory.join("constants.ts"),
        format!(
            "// Generated by generate_screen_protocol; do not edit.\nexport const SCREEN_NAMESPACE = {SCREEN_NAMESPACE:?};\nexport const SCREEN_OPERATION = {SCREEN_OPERATION:?};\nexport const DISPLAY_CATALOG_EVENT = {DISPLAY_CATALOG_EVENT:?};\n"
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
