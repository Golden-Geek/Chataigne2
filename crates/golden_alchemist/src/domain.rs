use std::collections::BTreeSet;

use golden_graph::{
    GraphChangeSet, GraphComment as CommonGraphComment, GraphCommentId, GraphDiagnostic, GraphDocument,
    GraphDocumentData, GraphDomain, GraphEdge, GraphEdgeId, GraphEnvelope, GraphGroup as CommonGraphGroup,
    GraphGroupId, GraphId, GraphNode, GraphNodeId, GraphPersistenceError, GraphPortId, GraphPresentation,
    GraphRevision, NodePresentation, PortDefinition, PortDirection, PortRef, PortSchema,
};
use indexmap::IndexMap;
use uuid::Uuid;

use crate::{
    AEdge, ALCHEMIST_SCHEMA_VERSION, ANodeConfig, ANodeId, ANodeInstance, ANodeRegistry, ANodeTypeId, ANodeUiState,
    AlchemistGraph, AlchemistGraphId, ExposedSurface, FormulaPropertySchema, GraphComment, GraphGroup, GraphLayout,
    GraphMetadata, InputSocketRef, OutputSocketRef, RuntimeValue, SignatureCtx, SocketId, TypeBindings, TypeConstraint,
    ValueTypeId, ValueTypeRegistry, primitive_node_registry,
};

const PORT_NAMESPACE: Uuid = Uuid::from_u128(0x4b95_7068_e36a_53e7_9578_1558_f053_66ec);
const EDGE_NAMESPACE: Uuid = Uuid::from_u128(0x1228_70ae_b9ea_53b7_922a_c4b5_0af5_8cb0);
const COMMENT_NAMESPACE: Uuid = Uuid::from_u128(0xddb6_aa2c_d541_5f01_886b_a278_3709_a38b);
const GROUP_NAMESPACE: Uuid = Uuid::from_u128(0xf58a_cc33_3260_5de4_971f_677b_3ea7_7869);

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

/// Governed Phase 3 bridge between the working Alchemist model and `golden_graph`.
pub struct AlchemistGraphAdapter;

impl AlchemistGraphAdapter {
    pub fn to_document(graph: &AlchemistGraph) -> Result<AlchemistGraphDocument, AlchemistGraphAdapterError> {
        let graph_id = GraphId::from_uuid(graph.id.as_uuid());
        let mut nodes = IndexMap::with_capacity(graph.nodes.len());
        let mut presentation = GraphPresentation::default();
        for (key, node) in &graph.nodes {
            if *key != node.id {
                return Err(AlchemistGraphAdapterError::NodeKeyMismatch {
                    key: *key,
                    node: node.id,
                });
            }
            let id = AlchemistGraphDomain::node_id(node.id);
            nodes.insert(
                id,
                GraphNode {
                    id,
                    data: AlchemistNodeData::from_instance(node),
                },
            );
            presentation.nodes.insert(id, node_presentation(&node.ui));
        }

        let mut edges = IndexMap::with_capacity(graph.edges.len());
        for edge in &graph.edges {
            let from = PortRef::new(
                AlchemistGraphDomain::node_id(edge.from.node),
                AlchemistGraphDomain::output_port_id(&edge.from.socket),
            );
            let to = PortRef::new(
                AlchemistGraphDomain::node_id(edge.to.node),
                AlchemistGraphDomain::input_port_id(&edge.to.socket),
            );
            let id = edge_id(graph.id, edge);
            if edges
                .insert(
                    id,
                    GraphEdge {
                        id,
                        from,
                        to,
                        data: AlchemistEdgeData,
                    },
                )
                .is_some()
            {
                return Err(AlchemistGraphAdapterError::DuplicateConnection { from, to });
            }
        }

        for (index, comment) in graph.layout.comments.iter().enumerate() {
            let id = comment_id(graph.id, index);
            presentation.comments.insert(
                id,
                CommonGraphComment {
                    id,
                    text: comment.text.clone(),
                    position: comment.position,
                    size: comment.size,
                },
            );
        }
        for (index, group) in graph.layout.groups.iter().enumerate() {
            let id = group_id(graph.id, index);
            presentation.groups.insert(
                id,
                CommonGraphGroup {
                    id,
                    label: group.label.clone(),
                    nodes: group
                        .nodes
                        .iter()
                        .copied()
                        .map(AlchemistGraphDomain::node_id)
                        .collect::<BTreeSet<_>>(),
                    position: group.position,
                    size: group.size,
                },
            );
        }

        GraphDocumentData {
            id: graph_id,
            revision: GraphRevision::default(),
            data: AlchemistGraphData {
                schema_version: graph.schema_version,
                exposed: graph.exposed.clone(),
                metadata: graph.metadata.clone(),
                viewport_origin: graph.layout.viewport_origin,
                viewport_zoom: graph.layout.viewport_zoom,
            },
            nodes,
            edges,
            presentation,
        }
        .into_document()
        .map_err(Into::into)
    }

    pub fn to_legacy(
        document: &AlchemistGraphDocument,
        domain: &AlchemistGraphDomain,
    ) -> Result<AlchemistGraph, AlchemistGraphAdapterError> {
        let mut nodes = IndexMap::with_capacity(document.nodes().len());
        for node in document.nodes() {
            let id = AlchemistGraphDomain::anode_id(node.id);
            let ui = document
                .presentation()
                .nodes
                .get(&node.id)
                .map(anode_ui_state)
                .unwrap_or_default();
            nodes.insert(id, node.data.clone().into_instance(id, ui));
        }

        let mut edges = Vec::with_capacity(document.edges().len());
        for edge in document.edges() {
            edges.push(AEdge {
                from: OutputSocketRef::new(
                    AlchemistGraphDomain::anode_id(edge.from.node),
                    socket_for(document, domain, edge.from, PortDirection::Output)?,
                ),
                to: InputSocketRef::new(
                    AlchemistGraphDomain::anode_id(edge.to.node),
                    socket_for(document, domain, edge.to, PortDirection::Input)?,
                ),
            });
        }

        let comments = document
            .presentation()
            .comments
            .values()
            .map(|comment| GraphComment {
                text: comment.text.clone(),
                position: comment.position,
                size: comment.size,
            })
            .collect();
        let groups = document
            .presentation()
            .groups
            .values()
            .map(|group| GraphGroup {
                label: group.label.clone(),
                nodes: group
                    .nodes
                    .iter()
                    .copied()
                    .map(AlchemistGraphDomain::anode_id)
                    .collect(),
                position: group.position,
                size: group.size,
            })
            .collect();
        let data = document.data();
        Ok(AlchemistGraph {
            schema_version: data.schema_version,
            id: AlchemistGraphId::from_uuid(document.id().as_uuid()),
            nodes,
            edges,
            exposed: data.exposed.clone(),
            layout: GraphLayout {
                comments,
                groups,
                viewport_origin: data.viewport_origin,
                viewport_zoom: data.viewport_zoom,
            },
            metadata: data.metadata.clone(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AlchemistGraphAdapterError {
    #[error(transparent)]
    InvalidDocument(#[from] GraphPersistenceError),
    #[error("Alchemist node map key `{key}` differs from embedded node id `{node}`")]
    NodeKeyMismatch { key: ANodeId, node: ANodeId },
    #[error("node `{0}` is missing while materializing an Alchemist connection")]
    MissingNode(GraphNodeId),
    #[error("port `{port:?}` is missing while materializing an Alchemist connection")]
    MissingPort { port: PortRef },
    #[error("port `{port:?}` has the wrong direction while materializing an Alchemist connection")]
    WrongPortDirection { port: PortRef },
    #[error("Alchemist connection `{from:?}` -> `{to:?}` is duplicated")]
    DuplicateConnection { from: PortRef, to: PortRef },
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

fn comment_id(graph: AlchemistGraphId, index: usize) -> GraphCommentId {
    GraphCommentId::from_uuid(Uuid::new_v5(
        &COMMENT_NAMESPACE,
        format!("{graph}:comment:{index}").as_bytes(),
    ))
}

fn group_id(graph: AlchemistGraphId, index: usize) -> GraphGroupId {
    GraphGroupId::from_uuid(Uuid::new_v5(
        &GROUP_NAMESPACE,
        format!("{graph}:group:{index}").as_bytes(),
    ))
}

fn node_presentation(ui: &ANodeUiState) -> NodePresentation {
    NodePresentation {
        position: ui.position,
        size: ui.size,
        collapsed: ui.collapsed,
    }
}

fn anode_ui_state(presentation: &NodePresentation) -> ANodeUiState {
    ANodeUiState {
        position: presentation.position,
        size: presentation.size,
        collapsed: presentation.collapsed,
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
) -> Result<SocketId, AlchemistGraphAdapterError> {
    let node = document
        .node(port.node)
        .ok_or(AlchemistGraphAdapterError::MissingNode(port.node))?;
    let definition = domain
        .node_ports(&node.data, document)
        .get(port.port)
        .cloned()
        .ok_or(AlchemistGraphAdapterError::MissingPort { port })?;
    if definition.direction != expected_direction {
        return Err(AlchemistGraphAdapterError::WrongPortDirection { port });
    }
    Ok(definition.data.socket)
}
