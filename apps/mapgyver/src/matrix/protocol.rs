//! App-owned Matrix document and operation DTOs; runtime launch state is not persisted here.
use crate::media::protocol::{ChannelReference, MediaId};
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
identity!(LayerId, GroupId, ColumnId, CellId);
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct MatrixDefinition {
    pub canvas: Extent2D,
    pub empty_cell: EmptyCellPolicy,
    pub transition: TransitionDefinition,
}
impl Default for MatrixDefinition {
    fn default() -> Self {
        Self {
            canvas: crate::media::resolution::default_canvas(),
            empty_cell: EmptyCellPolicy::Transparent,
            transition: TransitionDefinition::default(),
        }
    }
}
impl MatrixDefinition {
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
    pub opacity: f32,
    pub blend: BlendMode,
    pub enabled: bool,
}
impl Default for CompositeSettings {
    fn default() -> Self {
        Self {
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
#[serde(default)]
pub struct ColumnDefinition {
    pub transition: Option<TransitionDefinition>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CellDefinition {
    pub column: ColumnId,
    #[serde(default)]
    pub source: Option<ChannelReference>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum ElementDefinition {
    Layer { settings: CompositeSettings },
    Group { settings: CompositeSettings },
    Column { definition: ColumnDefinition },
    Cell { definition: CellDefinition },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct MatrixItemSnapshot {
    pub id: MatrixItemId,
    pub parent: Option<GroupId>,
    pub label: String,
    pub settings: CompositeSettings,
    pub channel: ChannelReference,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct ColumnSnapshot {
    pub id: ColumnId,
    pub label: String,
    pub definition: ColumnDefinition,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CellSnapshot {
    pub id: CellId,
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
    pub column_start: u32,
    pub column_count: u16,
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
    pub source_missing: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct MatrixViewportSnapshot {
    /// Ephemeral media-manager instance; persisted IDs alone cannot fence project reloads.
    pub project: uuid::Uuid,
    pub media: MediaId,
    pub final_output: crate::media::protocol::ChannelSnapshot,
    pub revision: u64,
    pub row_total: u32,
    pub column_total: u32,
    pub row_start: u32,
    pub column_start: u32,
    pub rows: Vec<MatrixViewRow>,
    pub columns: Vec<ColumnSnapshot>,
    pub cells: Vec<MatrixViewCell>,
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
pub enum MatrixEdit {
    InspectTargets {
        query: MatrixTargetQuery,
    },
    InspectViewport {
        viewport: MatrixViewport,
    },
    LaunchColumn {
        column: ColumnId,
        mode: LaunchMode,
    },
    InspectPlayback,
    Inspect,
    Configure {
        definition: MatrixDefinition,
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
        count: u16,
    },
    ConfigureItem {
        item: MatrixItemId,
        settings: CompositeSettings,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub enum LaunchMode {
    Configured,
    Cut,
}

/// Committed desired state, distinct from UI selection and actual renderer installation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MatrixPlaybackSnapshot {
    pub desired_column: Option<ColumnId>,
    pub launch_id: Option<uuid::Uuid>,
    pub transition: TransitionDefinition,
}

/// Metadata of the Matrix content carried by one renderer plan, never UI selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct MatrixPlanStamp {
    pub playback: MatrixPlaybackSnapshot,
    pub revision: u64,
    /// Includes transition envelope participants, not a claim of positive opacity in every frame.
    pub columns: Vec<ColumnId>,
    pub transitioning: bool,
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
