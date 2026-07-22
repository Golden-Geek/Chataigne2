use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::{ANodeId, ANodeTypeId, RuntimeValue, SocketId, TypeBindings};

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphMetadata {
    pub label: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}
