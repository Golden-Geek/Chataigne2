use std::collections::BTreeSet;

use golden_graph::{
    ConnectionPolicy, GraphChangeSet, GraphDiagnostic, GraphDocument, GraphDocumentData, GraphDomain, GraphEdge,
    GraphEdgeId, GraphEnvelope, GraphId, GraphNodeId, GraphPortId, GraphPresentation, GraphRevision, GraphTransaction,
    GraphTransactionError, IncomingConnectionPolicy, PortDefinition, PortDirection, PortRef, PortSchema,
};
use indexmap::IndexMap;
use uuid::Uuid;

use crate::{
    ActiveConfiguration, EnterPolicy, Region, RegionId, STATECHART_SCHEMA_VERSION, StateId, StateKind, StateNode,
    StateUiLayout, StatechartId, TransitionId,
};

const INCOMING_PORT: Uuid = Uuid::from_u128(0x4edb_9836_556c_5b31_a9ec_d2cb_4b85_f8c2);
const OUTGOING_PORT: Uuid = Uuid::from_u128(0xd636_2819_78e9_53fd_89e0_553d_e4f4_4b63);

/// Statechart-owned graph metadata and hierarchy outside generic nodes and edges.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatechartGraphData {
    pub schema_version: u32,
    pub root_region: RegionId,
    pub regions: IndexMap<RegionId, Region>,
    pub active: ActiveConfiguration,
    pub next_transition_order: u64,
}

/// State semantics without the identity or editor layout owned by `golden_graph`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatechartNodeData {
    pub label: String,
    pub parent_region: RegionId,
    pub kind: StateKind,
}

impl StatechartNodeData {
    #[must_use]
    pub fn from_state(state: &StateNode) -> Self {
        Self {
            label: state.label.clone(),
            parent_region: state.parent_region,
            kind: state.kind.clone(),
        }
    }

    #[must_use]
    pub fn into_state(self, id: StateId, ui_layout: StateUiLayout) -> StateNode {
        StateNode {
            id,
            label: self.label,
            parent_region: self.parent_region,
            kind: self.kind,
            ui_layout,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StatechartPortData {
    Incoming,
    Outgoing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StatechartEdgeData {
    pub priority: i32,
    pub creation_order: u64,
}

pub type StatechartGraphDocument = GraphDocument<StatechartGraphData, StatechartNodeData, StatechartEdgeData>;
pub type StatechartGraphEnvelope = GraphEnvelope<StatechartGraphData, StatechartNodeData, StatechartEdgeData>;
pub type StatechartGraphTransaction = GraphTransaction<StatechartGraphData, StatechartNodeData, StatechartEdgeData>;
pub type StatechartGraphTransactionError = GraphTransactionError;

#[cfg(feature = "serde")]
pub(crate) mod document_serde {
    use golden_graph::GraphEnvelope;
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

    use super::{STATECHART_SCHEMA_VERSION, StatechartGraphDocument, StatechartGraphDomain, StatechartGraphEnvelope};

    pub fn serialize<S>(document: &StatechartGraphDocument, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        GraphEnvelope::from_document(StatechartGraphDomain::DOMAIN_ID, STATECHART_SCHEMA_VERSION, document)
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<StatechartGraphDocument, D::Error>
    where
        D: Deserializer<'de>,
    {
        StatechartGraphEnvelope::deserialize(deserializer)?
            .into_document(StatechartGraphDomain::DOMAIN_ID, STATECHART_SCHEMA_VERSION)
            .map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct StatechartGraphDomain;

impl StatechartGraphDomain {
    pub const DOMAIN_ID: &'static str = "golden.statechart";

    #[must_use]
    pub fn new_document(id: StatechartId, root_region: RegionId) -> StatechartGraphDocument {
        GraphDocumentData {
            id: GraphId::from_uuid(id.as_uuid()),
            revision: GraphRevision::default(),
            data: StatechartGraphData {
                schema_version: STATECHART_SCHEMA_VERSION,
                root_region,
                regions: IndexMap::from([(
                    root_region,
                    Region {
                        id: root_region,
                        parent_state: None,
                        states: Vec::new(),
                        initial: None,
                    },
                )]),
                active: ActiveConfiguration::default(),
                next_transition_order: 0,
            },
            nodes: IndexMap::new(),
            edges: IndexMap::new(),
            presentation: GraphPresentation::default(),
        }
        .into_document()
        .expect("a fresh statechart graph document satisfies structural invariants")
    }

    #[must_use]
    pub const fn node_id(id: StateId) -> GraphNodeId {
        GraphNodeId::from_uuid(id.as_uuid())
    }

    #[must_use]
    pub const fn state_id(id: GraphNodeId) -> StateId {
        StateId::from_uuid(id.as_uuid())
    }

    #[must_use]
    pub const fn edge_id(id: TransitionId) -> GraphEdgeId {
        GraphEdgeId::from_uuid(id.as_uuid())
    }

    #[must_use]
    pub const fn transition_id(id: GraphEdgeId) -> TransitionId {
        TransitionId::from_uuid(id.as_uuid())
    }

    #[must_use]
    pub const fn incoming_port() -> GraphPortId {
        GraphPortId::from_uuid(INCOMING_PORT)
    }

    #[must_use]
    pub const fn outgoing_port() -> GraphPortId {
        GraphPortId::from_uuid(OUTGOING_PORT)
    }

    #[must_use]
    pub fn validate_document(&self, graph: &StatechartGraphDocument) -> Vec<GraphDiagnostic> {
        let changes = GraphChangeSet {
            inserted_nodes: graph.nodes().map(|node| node.id).collect(),
            inserted_edges: graph.edges().map(|edge| edge.id).collect(),
            graph_data_changed: true,
            ..GraphChangeSet::default()
        };
        self.validate_graph(graph, &changes)
    }
}

impl GraphDomain for StatechartGraphDomain {
    type GraphData = StatechartGraphData;
    type NodeData = StatechartNodeData;
    type PortData = StatechartPortData;
    type EdgeData = StatechartEdgeData;

    fn domain_id(&self) -> &'static str {
        Self::DOMAIN_ID
    }

    fn schema_version(&self) -> u32 {
        STATECHART_SCHEMA_VERSION
    }

    fn node_ports(&self, _node: &Self::NodeData, _graph: &StatechartGraphDocument) -> PortSchema<Self::PortData> {
        PortSchema {
            ports: vec![
                PortDefinition {
                    id: Self::incoming_port(),
                    direction: PortDirection::Input,
                    data: StatechartPortData::Incoming,
                },
                PortDefinition {
                    id: Self::outgoing_port(),
                    direction: PortDirection::Output,
                    data: StatechartPortData::Outgoing,
                },
            ],
        }
    }

    fn connection_policy(&self, _graph: &StatechartGraphDocument, _from: PortRef, _to: PortRef) -> ConnectionPolicy {
        ConnectionPolicy {
            incoming: IncomingConnectionPolicy::Multiple,
            allow_parallel: true,
        }
    }

    fn validate_connection(
        &self,
        _graph: &StatechartGraphDocument,
        _from: PortRef,
        _to: PortRef,
    ) -> Result<(), GraphDiagnostic> {
        Ok(())
    }

    fn validate_graph(&self, graph: &StatechartGraphDocument, changes: &GraphChangeSet) -> Vec<GraphDiagnostic> {
        let mut diagnostics = Vec::new();
        if changes.graph_data_changed {
            validate_graph_data(graph, &mut diagnostics);
        } else {
            for node in changes.inserted_nodes.iter().chain(&changes.updated_nodes) {
                if let Some(node) = graph.node(*node) {
                    validate_node(graph, node.id, &node.data, &mut diagnostics);
                }
            }
        }
        for edge in changes.inserted_edges.iter().chain(&changes.updated_edges) {
            if let Some(edge) = graph.edge(*edge) {
                validate_edge(edge, &mut diagnostics);
            }
        }
        diagnostics
    }
}

fn validate_graph_data(graph: &StatechartGraphDocument, diagnostics: &mut Vec<GraphDiagnostic>) {
    let data = graph.data();
    if data.schema_version > STATECHART_SCHEMA_VERSION {
        diagnostics.push(GraphDiagnostic::error(
            "unsupported_statechart_schema",
            format!(
                "statechart schema {} is newer than supported schema {}",
                data.schema_version, STATECHART_SCHEMA_VERSION
            ),
        ));
    }

    match data.regions.get(&data.root_region) {
        Some(region) if region.parent_state.is_none() => {}
        Some(_) => diagnostics.push(GraphDiagnostic::error(
            "root_region_has_parent",
            "the root statechart region cannot have a parent state",
        )),
        None => diagnostics.push(GraphDiagnostic::error(
            "missing_root_region",
            "the root statechart region is missing",
        )),
    }

    let mut memberships = BTreeSet::new();
    for (key, region) in &data.regions {
        if *key != region.id {
            diagnostics.push(GraphDiagnostic::error(
                "region_key_mismatch",
                format!("region map key `{key}` differs from embedded id `{}`", region.id),
            ));
        }
        if let Some(parent) = region.parent_state
            && graph.node(StatechartGraphDomain::node_id(parent)).is_none()
        {
            diagnostics.push(GraphDiagnostic::error(
                "missing_region_parent",
                format!("region `{}` references missing parent state `{parent}`", region.id),
            ));
        }
        if let Some(initial) = region.initial
            && !region.states.contains(&initial)
        {
            diagnostics.push(GraphDiagnostic::error(
                "initial_state_outside_region",
                format!("initial state `{initial}` is not a member of region `{}`", region.id),
            ));
        }
        for state in &region.states {
            if !memberships.insert(*state) {
                diagnostics.push(
                    GraphDiagnostic::error(
                        "duplicate_region_membership",
                        format!("state `{state}` belongs to more than one region"),
                    )
                    .at_node(StatechartGraphDomain::node_id(*state)),
                );
            }
            match graph.node(StatechartGraphDomain::node_id(*state)) {
                Some(node) if node.data.parent_region == region.id => {}
                Some(_) => diagnostics.push(
                    GraphDiagnostic::error(
                        "state_parent_region_mismatch",
                        format!("state `{state}` does not identify region `{}` as its parent", region.id),
                    )
                    .at_node(StatechartGraphDomain::node_id(*state)),
                ),
                None => diagnostics.push(GraphDiagnostic::error(
                    "missing_region_state",
                    format!("region `{}` references missing state `{state}`", region.id),
                )),
            }
        }
    }

    for node in graph.nodes() {
        validate_node(graph, node.id, &node.data, diagnostics);
        let state = StatechartGraphDomain::state_id(node.id);
        if !memberships.contains(&state) {
            diagnostics.push(
                GraphDiagnostic::error(
                    "state_without_region_membership",
                    format!("state `{state}` is not listed by its parent region"),
                )
                .at_node(node.id),
            );
        }
    }
    validate_active_configuration(graph, diagnostics);

    let mut creation_orders = BTreeSet::new();
    for edge in graph.edges() {
        validate_edge(edge, diagnostics);
        if !creation_orders.insert(edge.data.creation_order) {
            diagnostics.push(
                GraphDiagnostic::error(
                    "duplicate_transition_order",
                    format!("transition creation order {} is duplicated", edge.data.creation_order),
                )
                .at_edge(edge.id),
            );
        }
        if edge.data.creation_order >= data.next_transition_order {
            diagnostics.push(
                GraphDiagnostic::error(
                    "transition_order_not_advanced",
                    format!(
                        "next transition order {} does not follow transition order {}",
                        data.next_transition_order, edge.data.creation_order
                    ),
                )
                .at_edge(edge.id),
            );
        }
    }
}

fn validate_node(
    graph: &StatechartGraphDocument,
    id: GraphNodeId,
    node: &StatechartNodeData,
    diagnostics: &mut Vec<GraphDiagnostic>,
) {
    let state_id = StatechartGraphDomain::state_id(id);
    let Some(parent) = graph.data().regions.get(&node.parent_region) else {
        diagnostics.push(
            GraphDiagnostic::error(
                "missing_parent_region",
                format!("state `{state_id}` references missing region `{}`", node.parent_region),
            )
            .at_node(id),
        );
        return;
    };
    if !parent.states.contains(&state_id) {
        diagnostics.push(
            GraphDiagnostic::error(
                "state_missing_from_parent_region",
                format!("state `{state_id}` is not listed by region `{}`", parent.id),
            )
            .at_node(id),
        );
    }

    if let StateKind::Composite { regions, enter, .. } = &node.kind {
        if regions.is_empty() {
            diagnostics.push(
                GraphDiagnostic::error(
                    "composite_without_region",
                    format!("composite state `{state_id}` has no child region"),
                )
                .at_node(id),
            );
        }
        for region in regions {
            match graph.data().regions.get(region) {
                Some(child) if child.parent_state == Some(state_id) => {}
                Some(_) => diagnostics.push(
                    GraphDiagnostic::error(
                        "child_region_parent_mismatch",
                        format!("region `{region}` does not identify state `{state_id}` as its parent"),
                    )
                    .at_node(id),
                ),
                None => diagnostics.push(
                    GraphDiagnostic::error(
                        "missing_child_region",
                        format!("composite state `{state_id}` references missing region `{region}`"),
                    )
                    .at_node(id),
                ),
            }
        }
        if let EnterPolicy::Explicit(target) = enter
            && graph.node(StatechartGraphDomain::node_id(*target)).is_none()
        {
            diagnostics.push(
                GraphDiagnostic::error(
                    "missing_explicit_entry_state",
                    format!("composite state `{state_id}` references missing entry state `{target}`"),
                )
                .at_node(id),
            );
        }
    }
}

fn validate_edge(edge: &GraphEdge<StatechartEdgeData>, diagnostics: &mut Vec<GraphDiagnostic>) {
    if edge.from.port != StatechartGraphDomain::outgoing_port() {
        diagnostics.push(
            GraphDiagnostic::error(
                "invalid_transition_source_port",
                "statechart transition uses an invalid source port",
            )
            .at_edge(edge.id),
        );
    }
    if edge.to.port != StatechartGraphDomain::incoming_port() {
        diagnostics.push(
            GraphDiagnostic::error(
                "invalid_transition_target_port",
                "statechart transition uses an invalid target port",
            )
            .at_edge(edge.id),
        );
    }
}

fn validate_active_configuration(graph: &StatechartGraphDocument, diagnostics: &mut Vec<GraphDiagnostic>) {
    let active = &graph.data().active;
    for state in active
        .active_leaf_paths
        .iter()
        .flatten()
        .chain(&active.active_scopes)
        .chain(active.history.keys())
        .chain(active.history.values().filter_map(|history| history.shallow.as_ref()))
        .chain(
            active
                .history
                .values()
                .filter_map(|history| history.deep.as_ref())
                .flatten(),
        )
    {
        if graph.node(StatechartGraphDomain::node_id(*state)).is_none() {
            diagnostics.push(GraphDiagnostic::error(
                "active_configuration_missing_state",
                format!("active statechart configuration references missing state `{state}`"),
            ));
        }
    }
}
