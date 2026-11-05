//! App-owned Matrix document and operation DTOs; runtime launch state is not persisted here.
use crate::media::protocol::{ChannelReference, MediaFitMode, MediaId};
use golden_core::model::NodeUuid;
use golden_video::{Extent2D, plan::BlendMode};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

macro_rules! identity {
    ($($name:ident),+) => {$ (
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
        pub struct $name(pub NodeUuid);
    )+};
}
identity!(LayerId, GroupId, ColumnId, ColumnGroupId, CellId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ColumnItemId {
    Column { id: ColumnId },
    Group { id: ColumnGroupId },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MatrixItemId {
    Layer { id: LayerId },
    Group { id: GroupId },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
pub enum EmptyCellPolicy {
    #[default]
    Transparent,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
pub enum TransitionCurve {
    #[default]
    Linear,
    SmoothStep,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct TransitionDefinition {
    pub duration_ms: u32,
    pub curve: TransitionCurve,
}
impl Default for TransitionDefinition {
    fn default() -> Self {
        Self {
            duration_ms: 250,
            curve: TransitionCurve::Linear,
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
pub struct MatrixSettings {
    pub empty_cell: EmptyCellPolicy,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct MatrixDefinition {
    /// Read projection of the owning media Resolution, with the procedural default when disabled.
    pub canvas: Extent2D,
    pub empty_cell: EmptyCellPolicy,
}
impl Default for MatrixDefinition {
    fn default() -> Self {
        Self {
            canvas: crate::media::resolution::default_canvas(),
            empty_cell: EmptyCellPolicy::Transparent,
        }
    }
}
impl MatrixDefinition {
    pub fn settings(&self) -> MatrixSettings {
        MatrixSettings {
            empty_cell: self.empty_cell,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.canvas.width() > crate::media::resolution::MAX_CHANNEL_DIMENSION
            || self.canvas.height() > crate::media::resolution::MAX_CHANNEL_DIMENSION
        {
            return Err("Matrix canvas exceeds 16384 texels per axis".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MatrixOutput {
    Final { definition: MatrixDefinition },
    Layer { layer: LayerId },
    Group { group: GroupId },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct CompositeSettings {
    pub expose_channel: bool,
    pub opacity: f32,
    pub blend: BlendMode,
    pub enabled: bool,
}
impl Default for CompositeSettings {
    fn default() -> Self {
        Self {
            expose_channel: false,
            opacity: 1.0,
            blend: BlendMode::Normal,
            enabled: true,
        }
    }
}
impl CompositeSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err("Matrix opacity must be finite in 0..1".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, Default)]
#[serde(default, deny_unknown_fields)]
pub struct ColumnDefinition {
    pub transition: Option<TransitionDefinition>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CellDefinition {
    pub column: ColumnId,
    #[serde(default)]
    pub source: Option<ChannelReference>,
    /// This Cell's declared typed settings, including when unconfigured.
    pub application: crate::applications::protocol::ApplicationId,
    pub transition: Option<TransitionDefinition>,
    pub settings: CellSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
pub struct CellSettings {
    /// Top-left placement relative to the Layer canvas, in pixels.
    pub position: [f32; 2],
    /// None follows the source extent; authored placement never changes media Resolution.
    pub size: Option<Extent2D>,
    pub fit_mode: MediaFitMode,
    pub opacity: f32,
    pub enabled: bool,
}
impl Default for CellSettings {
    fn default() -> Self {
        Self {
            position: [0.0; 2],
            size: None,
            fit_mode: MediaFitMode::Fit,
            opacity: 1.0,
            enabled: true,
        }
    }
}
impl CellSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self
            .size
            .is_some_and(|size| size.width() > 16384 || size.height() > 16384)
        {
            return Err("Cell Size must contain whole pixels in 1..16384 per axis".into());
        }
        if !self.position.into_iter().all(f32::is_finite)
            || !self.opacity.is_finite()
            || !(0.0..=1.0).contains(&self.opacity)
        {
            return Err("Cell position must be finite and opacity must be in 0..1".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ElementDefinition {
    Layer { settings: CompositeSettings },
    Group { settings: CompositeSettings },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct MatrixItemSnapshot {
    pub id: MatrixItemId,
    pub parent: Option<GroupId>,
    pub label: String,
    pub settings: CompositeSettings,
    pub layer: Option<LayerSettings>,
    pub channel: ChannelReference,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
pub struct LayerSettings {
    /// Top-left position in the parent canvas, in pixels.
    pub position: [f32; 2],
    pub size: Extent2D,
    pub duration_ms: u32,
}
impl Default for LayerSettings {
    fn default() -> Self {
        Self {
            position: [0.0; 2],
            size: crate::media::resolution::default_canvas(),
            duration_ms: 250,
        }
    }
}
impl LayerSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.position.into_iter().all(f32::is_finite)
            || self.size.width() > crate::media::resolution::MAX_CHANNEL_DIMENSION
            || self.size.height() > crate::media::resolution::MAX_CHANNEL_DIMENSION
        {
            return Err("Layer position must be finite and canvas size must be in 1..16384".into());
        }
        Ok(())
    }
    pub(crate) fn transition(self) -> TransitionDefinition {
        TransitionDefinition {
            duration_ms: self.duration_ms,
            curve: TransitionCurve::Linear,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ColumnSnapshot {
    pub id: ColumnId,
    pub axis_index: u32,
    pub parent: Option<ColumnGroupId>,
    pub label: String,
    pub definition: ColumnDefinition,
    pub application: crate::applications::protocol::ApplicationId,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ColumnGroupSnapshot {
    pub id: ColumnGroupId,
    pub parent: Option<ColumnGroupId>,
    /// Position among this parent's Column/Column Group children.
    pub ordinal: u32,
    pub depth: u8,
    pub axis_start: u32,
    pub axis_count: u32,
    pub empty: bool,
    pub label: String,
    pub application: crate::applications::protocol::ApplicationId,
    /// Derived half-open range in the flattened Column projection; not authored storage.
    pub column_start: u32,
    pub column_count: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CellSnapshot {
    pub id: CellId,
    pub media: Option<MediaId>,
    pub layer: LayerId,
    pub definition: CellDefinition,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct MatrixSnapshot {
    pub media: MediaId,
    pub definition: MatrixDefinition,
    pub final_channel: ChannelReference,
    pub items: Vec<MatrixItemSnapshot>,
    pub columns: Vec<ColumnSnapshot>,
    pub column_groups: Vec<ColumnGroupSnapshot>,
    pub cells: Vec<CellSnapshot>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct CellAssignment {
    pub layer: LayerId,
    pub column: ColumnId,
    pub source: Option<ChannelReference>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub struct MatrixViewport {
    pub row_start: u32,
    pub row_count: u16,
    pub axis_start: u32,
    pub axis_count: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixViewRow {
    pub item: MatrixItemSnapshot,
    pub depth: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixViewCell {
    pub cell: CellSnapshot,
    pub source_label: Option<String>,
    /// Rendered identity; transparent Reference aliases share their effective target.
    pub source_preview: Option<crate::media::protocol::ChannelReference>,
    pub source_missing: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MatrixSelection {
    pub item: Option<MatrixItemId>,
    pub column: Option<ColumnId>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub struct MatrixEditorQuery {
    pub selected: Option<NodeUuid>,
    pub document: Option<NodeUuid>,
    pub locked: Option<MediaId>,
    pub retained: Option<MediaId>,
    pub viewport: MatrixViewport,
    pub selection: Option<MatrixSelection>,
    /// Last relevant domain object, independent of global Inspector selection.
    pub node: Option<NodeUuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixEditorPage {
    pub document: NodeUuid,
    pub document_matches: bool,
    pub media: Option<MediaId>,
    pub node: Option<NodeUuid>,
    pub selection: Option<MatrixSelection>,
    pub snapshot: Option<MatrixViewportSnapshot>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixSelectionSnapshot {
    pub selection: MatrixSelection,
    pub row_index: Option<u32>,
    pub column_index: Option<u32>,
    pub row: Option<MatrixViewRow>,
    pub column: Option<ColumnSnapshot>,
    pub cell: Option<MatrixViewCell>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixViewportSnapshot {
    pub label: String,
    pub media_types: Vec<crate::media::protocol::MediaCreationChoice>,
    /// Ephemeral media-manager instance; persisted IDs alone cannot fence project reloads.
    pub project: uuid::Uuid,
    pub media: MediaId,
    pub final_output: crate::media::protocol::ChannelSnapshot,
    pub revision: u64,
    pub row_total: u32,
    pub column_total: u32,
    pub axis_total: u32,
    pub axis_depth: u8,
    pub row_start: u32,
    pub axis_start: u32,
    pub rows: Vec<MatrixViewRow>,
    pub columns: Vec<ColumnSnapshot>,
    pub column_groups: Vec<ColumnGroupSnapshot>,
    pub cells: Vec<MatrixViewCell>,
    pub requested_column_groups: Vec<ColumnGroupId>,
    /// Inspector data is bounded independently of the virtualized grid window.
    pub selected: Option<Box<MatrixSelectionSnapshot>>,
    pub playback: MatrixPlaybackSnapshot,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MatrixTargetScope {
    Groups,
    Items { parent: Option<GroupId> },
    Columns,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MatrixTargetId {
    Item { item: MatrixItemId },
    Column { column: ColumnId },
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixTargetChoice {
    pub id: MatrixTargetId,
    pub label: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixTargetQuery {
    pub scope: MatrixTargetScope,
    pub search: String,
    pub start: u32,
    pub count: u16,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixTargetPage {
    pub choices: Vec<MatrixTargetChoice>,
    pub total: u32,
    pub start: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MatrixItemDropTarget {
    Root,
    Before { item: MatrixItemId },
    After { item: MatrixItemId },
    Inside { group: GroupId },
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ColumnItemDropTarget {
    Root,
    Before { item: ColumnItemId },
    After { item: ColumnItemId },
    Inside { group: ColumnGroupId },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub enum MatrixAxisItemKind {
    Column,
    ColumnGroup,
    Layer,
    LayerGroup,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum MatrixEdit {
    EditLayerGeometry {
        layer: LayerId,
        edit: crate::media::spatial::protocol::RectGeometryEdit,
    },
    /// Append to the selected object's hierarchy. Unrelated live selection uses the root;
    /// a removed context rejects rather than creating under a stale UI copy.
    CreateAxisItem {
        item: MatrixAxisItemKind,
        #[ts(type = "string | null")]
        context: Option<NodeUuid>,
    },
    DropColumnItem {
        item: ColumnItemId,
        target: ColumnItemDropTarget,
    },
    DropItem {
        item: MatrixItemId,
        target: MatrixItemDropTarget,
    },
    DropColumn {
        column: ColumnId,
        target: ColumnId,
        before: bool,
    },
    CreateCellMedia {
        layer: LayerId,
        column: ColumnId,
        media_kind: crate::media::protocol::MediaKind,
    },
    ConfigureCell {
        layer: LayerId,
        column: ColumnId,
        settings: CellSettings,
    },
    LaunchCell {
        layer: LayerId,
        column: ColumnId,
        mode: LaunchMode,
    },
    DropSource {
        layer: LayerId,
        column: ColumnId,
        #[ts(type = "string")]
        source: NodeUuid,
        #[serde(default)]
        copy: bool,
    },
    InspectTargets {
        query: MatrixTargetQuery,
    },
    InspectViewport {
        viewport: MatrixViewport,
        selection: Option<MatrixSelection>,
    },
    LaunchColumn {
        column: ColumnId,
        mode: LaunchMode,
    },
    InspectPlayback,
    ClearPlayback,
    Inspect,
    Configure {
        settings: MatrixSettings,
    },
    AddLayers {
        parent: Option<GroupId>,
        count: u16,
    },
    AddGroup {
        parent: Option<GroupId>,
        label: Option<String>,
    },
    AddColumns {
        parent: Option<ColumnGroupId>,
        count: u16,
    },
    AddColumnGroup {
        parent: Option<ColumnGroupId>,
        label: Option<String>,
    },
    RenameColumnGroup {
        group: ColumnGroupId,
        label: String,
    },
    MoveColumnItem {
        item: ColumnItemId,
        parent: Option<ColumnGroupId>,
        after: Option<ColumnItemId>,
    },
    UngroupColumns {
        group: ColumnGroupId,
    },
    RemoveColumnGroup {
        group: ColumnGroupId,
    },
    DuplicateColumnGroup {
        group: ColumnGroupId,
    },
    ConfigureItem {
        item: MatrixItemId,
        settings: CompositeSettings,
    },
    ConfigureLayer {
        layer: LayerId,
        settings: LayerSettings,
    },
    RenameItem {
        item: MatrixItemId,
        label: String,
    },
    MoveItem {
        item: MatrixItemId,
        parent: Option<GroupId>,
        after: Option<MatrixItemId>,
    },
    RemoveItem {
        item: MatrixItemId,
    },
    DuplicateItem {
        item: MatrixItemId,
    },
    RenameColumn {
        column: ColumnId,
        label: String,
    },
    ConfigureColumn {
        column: ColumnId,
        definition: ColumnDefinition,
    },
    ReorderColumn {
        column: ColumnId,
        after: Option<ColumnId>,
    },
    RemoveColumn {
        column: ColumnId,
    },
    DuplicateColumn {
        column: ColumnId,
    },
    SetCells {
        assignments: Vec<CellAssignment>,
    },
    ConfigureCellApplication {
        layer: LayerId,
        column: ColumnId,
        definition: crate::applications::protocol::ApplicationDefinition,
        pre: u16,
        post: u16,
    },
    ConfigureColumnApplication {
        column: ColumnId,
        definition: crate::applications::protocol::ApplicationDefinition,
        pre: u16,
        post: u16,
    },
    EnsureCell {
        layer: LayerId,
        column: ColumnId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum LaunchMode {
    Configured,
    Cut,
}

/// Committed desired state, distinct from UI selection and actual renderer installation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MatrixPlaybackSnapshot {
    pub request_id: Option<uuid::Uuid>,
    pub desired_column: Option<ColumnId>,
    pub launch_id: Option<uuid::Uuid>,
    pub transition: TransitionDefinition,
    pub layers: std::sync::Arc<[LayerPlaybackRequest]>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct LayerPlaybackRequest {
    pub layer: LayerId,
    pub column: Option<ColumnId>,
    pub launch_id: Option<uuid::Uuid>,
    pub transition: TransitionDefinition,
    pub individual: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MatrixLayerPlanStamp {
    pub layer: LayerId,
    pub columns: Vec<ColumnId>,
    pub transitioning: bool,
}

/// Metadata of the Matrix content carried by one renderer plan, never UI selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MatrixPlanStamp {
    pub playback: MatrixPlaybackSnapshot,
    pub revision: u64,
    /// Includes transition envelope participants, not a claim of positive opacity in every frame.
    pub columns: Vec<ColumnId>,
    /// Column-origin group ancestry carried by this plan, independent of current authoring.
    pub column_groups: Vec<ColumnGroupId>,
    pub transitioning: bool,
    pub layers: std::sync::Arc<[MatrixLayerPlanStamp]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum MatrixPreparationStage {
    Published,
    Preparing,
    Prepared,
    Installed,
    Rejected,
    Changed,
    Faulted,
    Recovering,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MatrixRenderStatus {
    pub project: uuid::Uuid,
    pub desired: MatrixPlanStamp,
    pub stage: MatrixPreparationStage,
    pub availability: crate::media::resolution::ChannelAvailability,
    pub installed: Option<MatrixPlanStamp>,
    /// Actual shared-plan frame submission, distinct from native output acceptance/presentation.
    pub submitted: Option<MatrixPlanStamp>,
    pub error: Option<String>,
}
