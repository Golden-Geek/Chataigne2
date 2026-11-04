use super::super::protocol::ChannelReference;
use golden_core::model::NodeUuid;
use golden_video::Extent2D;
pub use golden_video::plan::BlendMode;
pub use golden_video::plan::Point;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
pub struct InstanceId(pub NodeUuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct InstanceReference {
    pub composition: ChannelReference,
    pub instance: InstanceId,
}

/// Position locates the source pivot in canvas texels. Pivot is a source fraction; rotation is degrees.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default, deny_unknown_fields)]
pub struct Transform2D {
    pub position: [f32; 2],
    pub scale: [f32; 2],
    pub rotation_degrees: f32,
    pub pivot: [f32; 2],
}

impl Default for Transform2D {
    fn default() -> Self {
        Self {
            position: [0.0; 2],
            scale: [1.0; 2],
            rotation_degrees: 0.0,
            pivot: [0.5; 2],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
// Canvas read projection. Source is the owned media output, not a second authored binding.
pub struct InstanceDefinition {
    pub source: Option<ChannelReference>,
    pub transform: Transform2D,
    pub opacity: f32,
    pub blend: BlendMode,
    pub enabled: bool,
}

impl Default for InstanceDefinition {
    fn default() -> Self {
        Self {
            source: None,
            transform: Transform2D::default(),
            opacity: 1.0,
            blend: BlendMode::Normal,
            enabled: true,
        }
    }
}

impl InstanceDefinition {
    pub(crate) fn for_canvas(canvas: Extent2D) -> Self {
        let mut definition = Self::default();
        definition.transform.position = [canvas.width() as f32 / 2.0, canvas.height() as f32 / 2.0];
        definition
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self
            .transform
            .position
            .into_iter()
            .chain(self.transform.scale)
            .chain(self.transform.pivot)
            .chain([self.transform.rotation_degrees, self.opacity])
            .all(f32::is_finite)
            || self.transform.scale.into_iter().any(|value| value == 0.0)
            || !(0.0..=1.0).contains(&self.opacity)
        {
            return Err("instance transform must be finite with nonzero scale and opacity in 0..1".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(tag = "kind")]
pub enum CompositionEdit {
    Inspect,
    CreateMedia {
        media_kind: super::super::protocol::MediaKind,
        label: Option<String>,
        /// Canvas texels; defaults and placement Parameters are materialized by the backend.
        position: Point,
    },
    AddInstances {
        sources: Vec<ChannelReference>,
    },
    /// Drop an authored Media/Channel node onto the canvas or replace one instance's source.
    DropSource {
        #[ts(type = "string")]
        source: NodeUuid,
        instance: Option<InstanceId>,
        #[serde(default)]
        copy: bool,
        /// Canvas texels for a new instance; replacement preserves its existing transform.
        position: Point,
    },
    DuplicateInstances {
        instances: Vec<InstanceId>,
    },
    ConfigureInstance {
        instance: InstanceId,
        definition: InstanceDefinition,
    },
    EditInstanceGeometry {
        instance: InstanceId,
        edit: InstanceGeometryEdit,
    },
    RenameInstance {
        instance: InstanceId,
        label: String,
    },
    ReorderInstance {
        instance: InstanceId,
        after: Option<InstanceId>,
    },
    RemoveInstance {
        instance: InstanceId,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub enum InstanceGeometryMode {
    Translate,
    Rotate,
    Resize {
        handle: InstanceResizeHandle,
        /// Source extent observed when the gesture starts; backend validates current dimensions.
        extent: Extent2D,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub enum InstanceResizeHandle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

/// Canvas-texel pointer samples and the authored transform at gesture start, never accumulated deltas.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub struct InstanceGeometryEdit {
    pub mode: InstanceGeometryMode,
    pub origin: Transform2D,
    pub start: Point,
    pub point: Point,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct InstanceSnapshot {
    pub reference: InstanceReference,
    pub label: String,
    pub definition: InstanceDefinition,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
pub struct CompositionSnapshot {
    pub reference: ChannelReference,
    pub canvas: Extent2D,
    pub instances: Vec<InstanceSnapshot>,
}

/// Backend-resolved panel ownership. Locks are scoped to the persisted project root UUID.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, TS)]
pub struct CompositionEditorQuery {
    pub selected: Option<NodeUuid>,
    pub document: Option<NodeUuid>,
    pub locked: Option<ChannelReference>,
    pub instance: Option<InstanceId>,
    pub start: u32,
    pub count: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct CompositionEditorPage {
    pub media_types: Vec<super::super::protocol::MediaCreationChoice>,
    pub document: NodeUuid,
    pub document_matches: bool,
    pub channel: Option<crate::media::protocol::ChannelSnapshot>,
    /// Contains only the requested ordered page; total describes full membership.
    pub composition: Option<CompositionSnapshot>,
    pub start: u32,
    pub total: u32,
    /// Independent bounded lookup, including when the object is outside the visible page.
    pub selected: Option<InstanceSnapshot>,
}
