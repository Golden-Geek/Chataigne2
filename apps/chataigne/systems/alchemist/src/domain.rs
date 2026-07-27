use golden_graph::{
    GraphChangeSet, GraphDiagnostic, GraphDocument, GraphDocumentData, GraphDomain, GraphEdge, GraphEdgeId,
    GraphEnvelope, GraphId, GraphNode, GraphNodeId, GraphPortId, GraphPresentation, GraphRevision, GraphTransaction,
    GraphTransactionError, NodePresentation, PortDefinition, PortDirection, PortRef, PortSchema,
};
use indexmap::IndexMap;
use uuid::Uuid;

use crate::{
    AEdge, ALCHEMIST_SCHEMA_VERSION, ANodeConfig, ANodeId, ANodeInstance, ANodeRegistry, ANodeTypeId, ANodeUiState,
    AlchemistGraphId, ExposedSurface, FormulaPropertySchema, GraphMetadata, InputSocketRef, OutputSocketRef,
    RuntimeValue, SignatureCtx, SocketId, TypeBindings, TypeConstraint, ValueTypeId, ValueTypeRegistry,
    primitive_node_registry,
};

const PORT_NAMESPACE: Uuid = Uuid::from_u128(0x4b95_7068_e36a_53e7_9578_1558_f053_66ec);
const EDGE_NAMESPACE: Uuid = Uuid::from_u128(0x1228_70ae_b9ea_53b7_922a_c4b5_0af5_8cb0);

/// Alchemist-owned graph-level payload carried by the generic graph document.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlchemistGraphData {
    pub schema_version: u32,
    pub exposed: ExposedSurface,
    pub metadata: GraphMetadata,
    pub viewport_origin: [f64; 2],
    pub viewport_zoom: f64,
}

/// ANode semantics without identities or editor presentation, which are owned by `golden_graph`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlchemistNodeData {
    pub type_id: ANodeTypeId,
    pub label: String,
    pub enabled: bool,
    pub config: ANodeConfig,
    pub input_defaults: IndexMap<SocketId, RuntimeValue>,
    pub type_bindings: TypeBindings,
    pub forced_type_bindings: TypeBindings,
}

impl AlchemistNodeData {
    #[must_use]
    pub fn from_instance(instance: &ANodeInstance) -> Self {
        Self {
            type_id: instance.type_id.clone(),
            label: instance.label.clone(),
            enabled: instance.enabled,
            config: instance.config.clone(),
            input_defaults: instance.input_defaults.clone(),
            type_bindings: instance.type_bindings.clone(),
            forced_type_bindings: instance.forced_type_bindings.clone(),
        }
    }

    #[must_use]
    pub fn into_instance(self, id: ANodeId, ui: ANodeUiState) -> ANodeInstance {
        ANodeInstance {
            id,
            type_id: self.type_id,
            label: self.label,
            enabled: self.enabled,
            config: self.config,
            input_defaults: self.input_defaults,
            type_bindings: self.type_bindings,
            forced_type_bindings: self.forced_type_bindings,
            ui,
        }
    }

    #[must_use]
    pub fn to_instance(&self, id: ANodeId) -> ANodeInstance {
        self.clone().into_instance(id, ANodeUiState::default())
    }

    fn signature_instance(&self) -> ANodeInstance {
        self.clone()
            .into_instance(ANodeId::from_uuid(Uuid::nil()), ANodeUiState::default())
    }
}

/// Semantic metadata for a port declared by an ANode signature.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlchemistPortData {
    pub socket: SocketId,
    pub label: String,
    pub constraint: TypeConstraint,
}

/// Alchemist connections have no payload beyond their generic endpoints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlchemistEdgeData;

pub type AlchemistGraphDocument = GraphDocument<AlchemistGraphData, AlchemistNodeData, AlchemistEdgeData>;
pub type AlchemistGraphEnvelope = GraphEnvelope<AlchemistGraphData, AlchemistNodeData, AlchemistEdgeData>;
pub type AlchemistGraphTransaction = GraphTransaction<AlchemistGraphData, AlchemistNodeData, AlchemistEdgeData>;
pub type AlchemistGraphTransactionError = GraphTransactionError;

#[cfg(feature = "serde")]
pub(crate) mod document_serde {
    use golden_graph::GraphEnvelope;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

    use super::{ALCHEMIST_SCHEMA_VERSION, AlchemistGraphDocument, AlchemistGraphDomain, AlchemistGraphEnvelope};

    pub fn serialize<S>(document: &AlchemistGraphDocument, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        GraphEnvelope::from_document(AlchemistGraphDomain::DOMAIN_ID, ALCHEMIST_SCHEMA_VERSION, document)
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<AlchemistGraphDocument, D::Error>
    where
        D: Deserializer<'de>,
    {
        AlchemistGraphEnvelope::deserialize(deserializer)?
            .into_document(AlchemistGraphDomain::DOMAIN_ID, ALCHEMIST_SCHEMA_VERSION)
            .map_err(D::Error::custom)
    }
}

/// Real Alchemist semantics plugged into the app-agnostic graph contract.
#[derive(Clone)]
pub struct AlchemistGraphDomain {
    nodes: ANodeRegistry,
    value_types: ValueTypeRegistry,
    properties: Option<FormulaPropertySchema>,
}

impl AlchemistGraphDomain {
    pub const DOMAIN_ID: &'static str = "golden.alchemist";

    #[must_use]
    pub fn new(
        nodes: ANodeRegistry,
        value_types: ValueTypeRegistry,
        properties: Option<FormulaPropertySchema>,
    ) -> Self {
        Self {
            nodes,
            value_types,
            properties,
        }
    }

    #[must_use]
    pub fn with_primitives() -> Self {
        Self::new(primitive_node_registry(), ValueTypeRegistry::with_primitives(), None)
    }

    #[must_use]
    pub const fn node_id(id: ANodeId) -> GraphNodeId {
        GraphNodeId::from_uuid(id.as_uuid())
    }

    #[must_use]
    pub const fn anode_id(id: GraphNodeId) -> ANodeId {
        ANodeId::from_uuid(id.as_uuid())
    }

    #[must_use]
    pub fn input_port_id(socket: &SocketId) -> GraphPortId {
        port_id(PortDirection::Input, socket)
    }

    #[must_use]
    pub fn output_port_id(socket: &SocketId) -> GraphPortId {
        port_id(PortDirection::Output, socket)
    }

    #[must_use]
    pub fn new_document() -> AlchemistGraphDocument {
        Self::new_document_with_identity(AlchemistGraphId::new(), String::new())
    }

    #[must_use]
    pub fn new_document_with_identity(id: AlchemistGraphId, label: impl Into<String>) -> AlchemistGraphDocument {
        let metadata = GraphMetadata {
            label: label.into(),
            ..GraphMetadata::default()
        };
        GraphDocumentData {
            id: GraphId::from_uuid(id.as_uuid()),
            revision: GraphRevision::default(),
            data: AlchemistGraphData {
                schema_version: ALCHEMIST_SCHEMA_VERSION,
                exposed: ExposedSurface::default(),
                metadata,
                viewport_origin: [0.0, 0.0],
                viewport_zoom: 1.0,
            },
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
            presentation: GraphPresentation::default(),
        }
        .into_document()
        .expect("a fresh Alchemist graph document satisfies its structural invariants")
    }

    pub fn insert_node(transaction: &mut AlchemistGraphTransaction, node: ANodeInstance) {
        let id = Self::node_id(node.id);
        let presentation = node_presentation(&node.ui);
        transaction.insert_node(
            GraphNode {
                id,
                data: AlchemistNodeData::from_instance(&node),
            },
            Some(presentation),
        );
    }

    pub fn connect(
        transaction: &mut AlchemistGraphTransaction,
        document: &AlchemistGraphDocument,
        from: OutputSocketRef,
        to: InputSocketRef,
    ) {
        let semantic_edge = AEdge { from, to };
        let from = PortRef::new(
            Self::node_id(semantic_edge.from.node),
            Self::output_port_id(&semantic_edge.from.socket),
        );
        let to = PortRef::new(
            Self::node_id(semantic_edge.to.node),
            Self::input_port_id(&semantic_edge.to.socket),
        );
        transaction.connect(GraphEdge {
            id: edge_id(AlchemistGraphId::from_uuid(document.id().as_uuid()), &semantic_edge),
            from,
            to,
            data: AlchemistEdgeData,
        });
    }

    /// Validate every current node without turning localized edits into whole-document scans.
    #[must_use]
    pub fn validate_document(&self, graph: &AlchemistGraphDocument) -> Vec<GraphDiagnostic> {
        let changes = GraphChangeSet {
            inserted_nodes: graph.nodes().map(|node| node.id).collect(),
            graph_data_changed: true,
            ..GraphChangeSet::default()
        };
        self.validate_graph(graph, &changes)
    }

    pub(crate) fn semantic_edges(
        &self,
        document: &AlchemistGraphDocument,
    ) -> Result<Vec<AEdge>, AlchemistGraphSemanticError> {
        document
            .edges()
            .map(|edge| {
                Ok(AEdge {
                    from: OutputSocketRef::new(
                        Self::anode_id(edge.from.node),
                        socket_for(document, self, edge.from, PortDirection::Output)?,
                    ),
                    to: InputSocketRef::new(
                        Self::anode_id(edge.to.node),
                        socket_for(document, self, edge.to, PortDirection::Input)?,
                    ),
                })
            })
            .collect()
    }

    fn ports_for_node(&self, node: &AlchemistNodeData) -> PortSchema<AlchemistPortData> {
        let Some(declaration) = self.nodes.get(&node.type_id) else {
            return PortSchema { ports: Vec::new() };
        };
        let instance = node.signature_instance();
        let signature = declaration.signature(
            &SignatureCtx {
                value_types: &self.value_types,
                properties: self.properties.as_ref(),
            },
            &instance,
            &instance.type_bindings,
        );
        let mut ports = Vec::with_capacity(signature.inputs.len() + signature.outputs.len());
        ports.extend(signature.inputs.into_iter().map(|socket| PortDefinition {
            id: Self::input_port_id(&socket.id),
            direction: PortDirection::Input,
            data: AlchemistPortData {
                socket: socket.id,
                label: socket.label,
                constraint: socket.constraint,
            },
        }));
        ports.extend(signature.outputs.into_iter().map(|socket| PortDefinition {
            id: Self::output_port_id(&socket.id),
            direction: PortDirection::Output,
            data: AlchemistPortData {
                socket: socket.id,
                label: socket.label,
                constraint: socket.constraint,
            },
        }));
        PortSchema { ports }
    }
}

impl GraphDomain for AlchemistGraphDomain {
    type GraphData = AlchemistGraphData;
    type NodeData = AlchemistNodeData;
    type PortData = AlchemistPortData;
    type EdgeData = AlchemistEdgeData;

    fn domain_id(&self) -> &'static str {
        Self::DOMAIN_ID
    }

    fn schema_version(&self) -> u32 {
        ALCHEMIST_SCHEMA_VERSION
    }

    fn node_ports(&self, node: &Self::NodeData, _graph: &AlchemistGraphDocument) -> PortSchema<Self::PortData> {
        self.ports_for_node(node)
    }

    fn validate_connection(
        &self,
        graph: &AlchemistGraphDocument,
        from: PortRef,
        to: PortRef,
    ) -> Result<(), GraphDiagnostic> {
        let from_node = graph
            .node(from.node)
            .ok_or_else(|| GraphDiagnostic::error("missing_source_node", "source node is not present"))?;
        let to_node = graph
            .node(to.node)
            .ok_or_else(|| GraphDiagnostic::error("missing_target_node", "target node is not present"))?;
        let from_port = self
            .ports_for_node(&from_node.data)
            .get(from.port)
            .cloned()
            .ok_or_else(|| GraphDiagnostic::error("missing_source_port", "source port is not declared"))?;
        let to_port = self
            .ports_for_node(&to_node.data)
            .get(to.port)
            .cloned()
            .ok_or_else(|| GraphDiagnostic::error("missing_target_port", "target port is not declared"))?;

        if let Some(value_type) = resolved_constraint_type(&from_port.data.constraint, &from_node.data) {
            let accepts = resolved_constraint_type(&to_port.data.constraint, &to_node.data).map_or_else(
                || {
                    to_port
                        .data
                        .constraint
                        .accepts_value_type(&value_type, &self.value_types)
                },
                |expected| expected == value_type || self.value_types.can_convert_automatically(&value_type, &expected),
            );
            if !accepts {
                return Err(GraphDiagnostic::error(
                    "incompatible_socket_types",
                    format!(
                        "output `{}` cannot connect to input `{}`",
                        from_port.data.label, to_port.data.label
                    ),
                )
                .at_node(to.node));
            }
        }
        Ok(())
    }

    fn validate_graph(&self, graph: &AlchemistGraphDocument, changes: &GraphChangeSet) -> Vec<GraphDiagnostic> {
        let mut diagnostics = Vec::new();
        if changes.graph_data_changed && graph.data().schema_version > ALCHEMIST_SCHEMA_VERSION {
            diagnostics.push(GraphDiagnostic::error(
                "unsupported_alchemist_schema",
                format!(
                    "Alchemist graph schema {} is newer than supported schema {}",
                    graph.data().schema_version,
                    ALCHEMIST_SCHEMA_VERSION
                ),
            ));
        }
        for node_id in changes.inserted_nodes.iter().chain(&changes.updated_nodes) {
            let Some(node) = graph.node(*node_id) else {
                continue;
            };
            if self.nodes.get(&node.data.type_id).is_none() {
                diagnostics.push(
                    GraphDiagnostic::error(
                        "unknown_anode_type",
                        format!("ANode type `{}` is not registered", node.data.type_id),
                    )
                    .at_node(*node_id),
                );
            }
        }
        diagnostics
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AlchemistGraphSemanticError {
    #[error("node `{0}` is missing while materializing an Alchemist connection")]
    MissingNode(GraphNodeId),
    #[error("port `{port:?}` is missing while materializing an Alchemist connection")]
    MissingPort { port: PortRef },
    #[error("port `{port:?}` has the wrong direction while materializing an Alchemist connection")]
    WrongPortDirection { port: PortRef },
}

fn port_id(direction: PortDirection, socket: &SocketId) -> GraphPortId {
    let prefix = match direction {
        PortDirection::Input => "input:",
        PortDirection::Output => "output:",
    };
    GraphPortId::from_uuid(Uuid::new_v5(
        &PORT_NAMESPACE,
        format!("{prefix}{}", socket.as_str()).as_bytes(),
    ))
}

fn edge_id(graph: AlchemistGraphId, edge: &AEdge) -> GraphEdgeId {
    GraphEdgeId::from_uuid(Uuid::new_v5(
        &EDGE_NAMESPACE,
        format!(
            "{}:{}:{}>{}:{}",
            graph, edge.from.node, edge.from.socket, edge.to.node, edge.to.socket
        )
        .as_bytes(),
    ))
}

fn node_presentation(ui: &ANodeUiState) -> NodePresentation {
    NodePresentation {
        position: ui.position,
        size: ui.size,
        collapsed: ui.collapsed,
    }
}

fn resolved_constraint_type(constraint: &TypeConstraint, node: &AlchemistNodeData) -> Option<ValueTypeId> {
    match constraint {
        TypeConstraint::Exact(value_type) => Some(value_type.clone()),
        TypeConstraint::Generic(variable) => node
            .forced_type_bindings
            .get(variable)
            .or_else(|| node.type_bindings.get(variable))
            .map(|binding| binding.value_type.clone()),
        TypeConstraint::OneOf(constraints) if constraints.len() == 1 => resolved_constraint_type(&constraints[0], node),
        _ => None,
    }
}

fn socket_for(
    document: &AlchemistGraphDocument,
    domain: &AlchemistGraphDomain,
    port: PortRef,
    expected_direction: PortDirection,
) -> Result<SocketId, AlchemistGraphSemanticError> {
    let node = document
        .node(port.node)
        .ok_or(AlchemistGraphSemanticError::MissingNode(port.node))?;
    let definition = domain
        .node_ports(&node.data, document)
        .get(port.port)
        .cloned()
        .ok_or(AlchemistGraphSemanticError::MissingPort { port })?;
    if definition.direction != expected_direction {
        return Err(AlchemistGraphSemanticError::WrongPortDirection { port });
    }
    Ok(definition.data.socket)
}
