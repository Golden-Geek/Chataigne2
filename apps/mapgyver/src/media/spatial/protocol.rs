use crate::media::protocol::{ChannelReference, MediaId};
use golden_core::node::NodeUuid;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub enum SpatialResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct SpatialRect {
    pub position: [f32; 2],
    pub size: golden_video::Extent2D,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub enum RectGeometryMode {
    Translate,
    Resize { handle: SpatialResizeHandle },
}

/// Immutable gesture basis and canvas-texel samples; backend owns rounding and resize anchors.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub struct RectGeometryEdit {
    pub origin: SpatialRect,
    pub mode: RectGeometryMode,
    pub start: golden_video::plan::Point,
    pub point: golden_video::plan::Point,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum SpatialEditorTarget {
    Composition { reference: ChannelReference },
    Matrix { media: MediaId },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub struct SpatialEditorQuery {
    pub selected: Option<NodeUuid>,
    pub document: Option<NodeUuid>,
    pub locked: Option<SpatialEditorTarget>,
    pub retained: Option<SpatialEditorTarget>,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct SpatialEditorBinding {
    pub document: NodeUuid,
    pub document_matches: bool,
    pub target: Option<SpatialEditorTarget>,
    pub present: bool,
    pub label: Option<String>,
}
