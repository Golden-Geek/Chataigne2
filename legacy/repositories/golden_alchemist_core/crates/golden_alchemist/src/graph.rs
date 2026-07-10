use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::{
    ALCHEMIST_SCHEMA_VERSION, ANodeId, ANodeTypeId, AlchemistGraphId, ExposedSurface, RuntimeValue, SocketId,
    TypeBindings,
};

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ANodeConfig {
    pub fields: IndexMap<SmolStr, RuntimeValue>,
}

impl ANodeConfig {
    pub fn set(&mut self, field: impl Into<SmolStr>, value: RuntimeValue) {
        self.fields.insert(field.into(), value);
    }

    #[must_use]
    pub fn get(&self, field: &str) -> Option<&RuntimeValue> {
        self.fields.get(field)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ANodeUiState {
    pub position: [f64; 2],
    pub size: Option<[f64; 2]>,
    pub collapsed: bool,
}

impl Default for ANodeUiState {
    fn default() -> Self {
        Self {
            position: [0.0; 2],
            size: None,
            collapsed: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ANodeInstance {
    pub id: ANodeId,
    pub type_id: ANodeTypeId,
    pub label: String,
    #[cfg_attr(feature = "serde", serde(default = "default_node_enabled"))]
    pub enabled: bool,
    pub config: ANodeConfig,
    pub input_defaults: IndexMap<SocketId, RuntimeValue>,
    pub type_bindings: TypeBindings,
    pub forced_type_bindings: TypeBindings,
    pub ui: ANodeUiState,
}

impl ANodeInstance {
    #[must_use]
    pub fn new(type_id: ANodeTypeId, label: impl Into<String>) -> Self {
        Self {
            id: ANodeId::new(),
            type_id,
            label: label.into(),
            enabled: true,
            config: ANodeConfig::default(),
            input_defaults: IndexMap::new(),
            type_bindings: TypeBindings::default(),
            forced_type_bindings: TypeBindings::default(),
            ui: ANodeUiState::default(),
        }
    }
}

const fn default_node_enabled() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OutputSocketRef {
    pub node: ANodeId,
    pub socket: SocketId,
}

impl OutputSocketRef {
    #[must_use]
    pub fn new(node: ANodeId, socket: impl Into<SocketId>) -> Self {
        Self {
            node,
            socket: socket.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InputSocketRef {
    pub node: ANodeId,
    pub socket: SocketId,
}

impl InputSocketRef {
    #[must_use]
    pub fn new(node: ANodeId, socket: impl Into<SocketId>) -> Self {
        Self {
            node,
            socket: socket.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AEdge {
    pub from: OutputSocketRef,
    pub to: InputSocketRef,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphComment {
    pub text: String,
    pub position: [f64; 2],
    pub size: [f64; 2],
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphGroup {
    pub label: String,
    pub nodes: Vec<ANodeId>,
    pub position: [f64; 2],
    pub size: [f64; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphLayout {
    pub comments: Vec<GraphComment>,
    pub groups: Vec<GraphGroup>,
    pub viewport_origin: [f64; 2],
    pub viewport_zoom: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphMetadata {
    pub label: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlchemistGraph {
    pub schema_version: u32,
    pub id: AlchemistGraphId,
    pub nodes: IndexMap<ANodeId, ANodeInstance>,
    pub edges: Vec<AEdge>,
    pub exposed: ExposedSurface,
    pub layout: GraphLayout,
    pub metadata: GraphMetadata,
}

impl Default for AlchemistGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl AlchemistGraph {
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: ALCHEMIST_SCHEMA_VERSION,
            id: AlchemistGraphId::new(),
            nodes: IndexMap::new(),
            edges: Vec::new(),
            exposed: ExposedSurface::default(),
            layout: GraphLayout {
                viewport_zoom: 1.0,
                ..GraphLayout::default()
            },
            metadata: GraphMetadata::default(),
        }
    }

    pub fn add_node(&mut self, node: ANodeInstance) -> Result<ANodeId, GraphEditError> {
        if self.nodes.contains_key(&node.id) {
            return Err(GraphEditError::DuplicateNode(node.id));
        }
        let id = node.id;
        self.nodes.insert(id, node);
        Ok(id)
    }

    pub fn remove_node(&mut self, node: ANodeId) -> Result<ANodeInstance, GraphEditError> {
        let removed = self
            .nodes
            .shift_remove(&node)
            .ok_or(GraphEditError::MissingNode(node))?;
        self.edges.retain(|edge| edge.from.node != node && edge.to.node != node);
        Ok(removed)
    }

    pub fn connect(&mut self, from: OutputSocketRef, to: InputSocketRef) -> Result<(), GraphEditError> {
        self.require_node(from.node)?;
        self.require_node(to.node)?;
        if self.edges.iter().any(|edge| edge.from == from && edge.to == to) {
            return Err(GraphEditError::DuplicateEdge);
        }
        if self.edges.iter().any(|edge| edge.to == to) {
            return Err(GraphEditError::InputAlreadyConnected(to));
        }
        self.edges.push(AEdge { from, to });
        Ok(())
    }

    pub fn disconnect(&mut self, from: &OutputSocketRef, to: &InputSocketRef) -> Result<AEdge, GraphEditError> {
        let index = self
            .edges
            .iter()
            .position(|edge| &edge.from == from && &edge.to == to)
            .ok_or(GraphEditError::MissingEdge)?;
        Ok(self.edges.remove(index))
    }

    fn require_node(&self, node: ANodeId) -> Result<(), GraphEditError> {
        if self.nodes.contains_key(&node) {
            Ok(())
        } else {
            Err(GraphEditError::MissingNode(node))
        }
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum GraphEditError {
    #[error("node `{0}` is already present")]
    DuplicateNode(ANodeId),
    #[error("node `{0}` is not present")]
    MissingNode(ANodeId),
    #[error("edge is already present")]
    DuplicateEdge,
    #[error("input `{0:?}` already has a connection")]
    InputAlreadyConnected(InputSocketRef),
    #[error("edge is not present")]
    MissingEdge,
}
