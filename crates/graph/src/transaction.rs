use std::collections::BTreeSet;

use crate::document::{RemovedEdge, RemovedNode};
use crate::{
    DiagnosticSeverity, GraphChangeSet, GraphDelta, GraphDiagnostic, GraphDocument, GraphDomain, GraphEdge,
    GraphEdgeId, GraphEditError, GraphNode, GraphNodeId, GraphRevision, GraphTransactionError,
    IncomingConnectionPolicy, NodePresentation, PortDirection,
};

/// One mutation in an atomic graph transaction.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GraphOperation<G, N, E> {
    InsertNode {
        node: GraphNode<N>,
        presentation: Option<NodePresentation>,
    },
    RemoveNode {
        node: GraphNodeId,
    },
    ReplaceNodeData {
        node: GraphNodeId,
        data: N,
    },
    ReplaceGraphData {
        data: G,
    },
    Connect {
        edge: GraphEdge<E>,
    },
    Disconnect {
        edge: GraphEdgeId,
    },
    ReplaceEdgeData {
        edge: GraphEdgeId,
        data: E,
    },
    SetNodePresentation {
        node: GraphNodeId,
        presentation: Option<NodePresentation>,
    },
}

/// A revision-checked batch of graph operations.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphTransaction<G, N, E> {
    pub base_revision: GraphRevision,
    pub operations: Vec<GraphOperation<G, N, E>>,
}

impl<G, N, E> GraphTransaction<G, N, E> {
    #[must_use]
    pub fn new(base_revision: GraphRevision) -> Self {
        Self {
            base_revision,
            operations: Vec::new(),
        }
    }

    #[must_use]
    pub fn for_document(document: &GraphDocument<G, N, E>) -> Self {
        Self::new(document.revision())
    }

    pub fn push(&mut self, operation: GraphOperation<G, N, E>) {
        self.operations.push(operation);
    }

    pub fn insert_node(&mut self, node: GraphNode<N>, presentation: Option<NodePresentation>) {
        self.push(GraphOperation::InsertNode { node, presentation });
    }

    pub fn remove_node(&mut self, node: GraphNodeId) {
        self.push(GraphOperation::RemoveNode { node });
    }

    pub fn replace_node_data(&mut self, node: GraphNodeId, data: N) {
        self.push(GraphOperation::ReplaceNodeData { node, data });
    }

    pub fn replace_graph_data(&mut self, data: G) {
        self.push(GraphOperation::ReplaceGraphData { data });
    }

    pub fn connect(&mut self, edge: GraphEdge<E>) {
        self.push(GraphOperation::Connect { edge });
    }

    pub fn disconnect(&mut self, edge: GraphEdgeId) {
        self.push(GraphOperation::Disconnect { edge });
    }

    pub fn replace_edge_data(&mut self, edge: GraphEdgeId, data: E) {
        self.push(GraphOperation::ReplaceEdgeData { edge, data });
    }

    pub fn set_node_presentation(&mut self, node: GraphNodeId, presentation: Option<NodePresentation>) {
        self.push(GraphOperation::SetNodePresentation { node, presentation });
    }

    pub fn commit<D>(
        self,
        document: &mut GraphDocument<G, N, E>,
        domain: &D,
    ) -> Result<GraphCommit, GraphTransactionError>
    where
        G: Clone,
        N: Clone,
        E: Clone,
        D: GraphDomain<GraphData = G, NodeData = N, EdgeData = E>,
    {
        if document.revision().sequence != self.base_revision.sequence {
            return Err(GraphTransactionError::RevisionConflict {
                expected: self.base_revision.sequence,
                actual: document.revision().sequence,
            });
        }

        let from = document.revision();
        let mut changes = GraphChangeSet::default();
        let mut undo = Vec::with_capacity(self.operations.len());

        for operation in self.operations {
            match apply_operation(document, domain, operation, &mut changes) {
                Ok(applied) => undo.push(applied),
                Err(error) => {
                    rollback(document, undo);
                    return Err(error.into());
                }
            }
        }

        if let Err(error) = validate_changed_ports(document, domain, &changes) {
            rollback(document, undo);
            return Err(error.into());
        }

        let diagnostics = domain.validate_graph(document, &changes);
        let errors = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .cloned()
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            rollback(document, undo);
            return Err(GraphTransactionError::Validation(errors));
        }

        let to = from.advanced(&changes);
        document.revision = to;
        Ok(GraphCommit {
            delta: GraphDelta { from, to, changes },
            diagnostics,
        })
    }
}

/// Successful transaction result with coherent delta and non-fatal diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphCommit {
    pub delta: GraphDelta,
    pub diagnostics: Vec<GraphDiagnostic>,
}

enum Undo<G, N, E> {
    InsertedNode(GraphNodeId),
    RemovedNode(RemovedNode<N, E>),
    ReplacedNodeData {
        node: GraphNodeId,
        data: N,
    },
    ReplacedGraphData(G),
    InsertedEdge(GraphEdgeId),
    RemovedEdge(RemovedEdge<E>),
    ReplacedEdgeData {
        edge: GraphEdgeId,
        data: E,
    },
    NodePresentation {
        node: GraphNodeId,
        presentation: Option<NodePresentation>,
    },
}

fn apply_operation<G, N, E, D>(
    document: &mut GraphDocument<G, N, E>,
    domain: &D,
    operation: GraphOperation<G, N, E>,
    changes: &mut GraphChangeSet,
) -> Result<Undo<G, N, E>, GraphEditError>
where
    D: GraphDomain<GraphData = G, NodeData = N, EdgeData = E>,
{
    match operation {
        GraphOperation::InsertNode { node, presentation } => {
            let id = node.id;
            document.insert_node_at(None, node)?;
            if let Some(presentation) = presentation {
                document.set_node_presentation(id, Some(presentation))?;
                changes.presentation_nodes.insert(id);
            }
            changes.node_inserted(id);
            Ok(Undo::InsertedNode(id))
        }
        GraphOperation::RemoveNode { node } => {
            let removed = document.remove_node(node)?;
            for edge in &removed.edges {
                changes.edge_removed(edge.edge.id);
            }
            let had_presentation = removed.presentation.is_some();
            let groups = removed.group_memberships.clone();
            if changes.node_removed(node) {
                if had_presentation {
                    changes.presentation_nodes.insert(node);
                }
                changes.presentation_groups.extend(groups);
            }
            Ok(Undo::RemovedNode(removed))
        }
        GraphOperation::ReplaceNodeData { node, data } => {
            let record = document.nodes.get_mut(&node).ok_or(GraphEditError::MissingNode(node))?;
            let old = std::mem::replace(&mut record.data, data);
            changes.updated_nodes.insert(node);
            Ok(Undo::ReplacedNodeData { node, data: old })
        }
        GraphOperation::ReplaceGraphData { data } => {
            let old = std::mem::replace(&mut document.data, data);
            changes.graph_data_changed = true;
            Ok(Undo::ReplacedGraphData(old))
        }
        GraphOperation::Connect { edge } => {
            validate_edge(document, domain, &edge)?;
            let id = edge.id;
            document.insert_edge_at(None, edge)?;
            changes.edge_inserted(id);
            Ok(Undo::InsertedEdge(id))
        }
        GraphOperation::Disconnect { edge } => {
            let removed = document.remove_edge(edge)?;
            changes.edge_removed(edge);
            Ok(Undo::RemovedEdge(removed))
        }
        GraphOperation::ReplaceEdgeData { edge, data } => {
            let record = document.edges.get_mut(&edge).ok_or(GraphEditError::MissingEdge(edge))?;
            let old = std::mem::replace(&mut record.data, data);
            changes.updated_edges.insert(edge);
            Ok(Undo::ReplacedEdgeData { edge, data: old })
        }
        GraphOperation::SetNodePresentation { node, presentation } => {
            let old = document.set_node_presentation(node, presentation)?;
            changes.presentation_nodes.insert(node);
            Ok(Undo::NodePresentation {
                node,
                presentation: old,
            })
        }
    }
}

fn validate_edge<G, N, E, D>(
    document: &GraphDocument<G, N, E>,
    domain: &D,
    edge: &GraphEdge<E>,
) -> Result<(), GraphEditError>
where
    D: GraphDomain<GraphData = G, NodeData = N, EdgeData = E>,
{
    validate_edge_ports(document, domain, edge)?;
    let policy = domain.connection_policy(document, edge.from, edge.to);
    if policy.incoming == IncomingConnectionPolicy::Single && document.incoming_edges_for_port(edge.to).next().is_some()
    {
        return Err(GraphEditError::InputAlreadyConnected(edge.to));
    }
    if !policy.allow_parallel && document.has_connection(edge.from, edge.to) {
        return Err(GraphEditError::DuplicateConnection {
            from: edge.from,
            to: edge.to,
        });
    }
    domain
        .validate_connection(document, edge.from, edge.to)
        .map_err(GraphEditError::DomainConnection)
}

fn validate_changed_ports<G, N, E, D>(
    document: &GraphDocument<G, N, E>,
    domain: &D,
    changes: &GraphChangeSet,
) -> Result<(), GraphEditError>
where
    D: GraphDomain<GraphData = G, NodeData = N, EdgeData = E>,
{
    let affected = if changes.graph_data_changed {
        document.edges().map(|edge| edge.id).collect::<BTreeSet<_>>()
    } else {
        changes
            .updated_nodes
            .iter()
            .flat_map(|node| document.incoming_edges(*node).chain(document.outgoing_edges(*node)))
            .collect()
    };
    for edge in affected {
        validate_edge_ports(document, domain, document.edge(edge).expect("topology edge is indexed"))?;
    }
    Ok(())
}

fn validate_edge_ports<G, N, E, D>(
    document: &GraphDocument<G, N, E>,
    domain: &D,
    edge: &GraphEdge<E>,
) -> Result<(), GraphEditError>
where
    D: GraphDomain<GraphData = G, NodeData = N, EdgeData = E>,
{
    let from_node = document
        .node(edge.from.node)
        .ok_or(GraphEditError::MissingNode(edge.from.node))?;
    let to_node = document
        .node(edge.to.node)
        .ok_or(GraphEditError::MissingNode(edge.to.node))?;
    let from_ports = domain.node_ports(&from_node.data, document);
    let to_ports = domain.node_ports(&to_node.data, document);
    let from = from_ports
        .get(edge.from.port)
        .ok_or(GraphEditError::MissingPort(edge.from))?;
    let to = to_ports.get(edge.to.port).ok_or(GraphEditError::MissingPort(edge.to))?;
    if from.direction != PortDirection::Output {
        return Err(GraphEditError::WrongPortDirection {
            port: edge.from,
            expected: "output",
        });
    }
    if to.direction != PortDirection::Input {
        return Err(GraphEditError::WrongPortDirection {
            port: edge.to,
            expected: "input",
        });
    }
    Ok(())
}

fn rollback<G, N, E>(document: &mut GraphDocument<G, N, E>, undo: Vec<Undo<G, N, E>>) {
    for operation in undo.into_iter().rev() {
        match operation {
            Undo::InsertedNode(node) => {
                document
                    .remove_node(node)
                    .expect("transaction rollback must remove inserted node");
            }
            Undo::RemovedNode(removed) => document.restore_node(removed),
            Undo::ReplacedNodeData { node, data } => {
                document.nodes.get_mut(&node).expect("rollback node must exist").data = data;
            }
            Undo::ReplacedGraphData(data) => document.data = data,
            Undo::InsertedEdge(edge) => {
                document
                    .remove_edge(edge)
                    .expect("transaction rollback must remove inserted edge");
            }
            Undo::RemovedEdge(removed) => document.restore_edge(removed),
            Undo::ReplacedEdgeData { edge, data } => {
                document.edges.get_mut(&edge).expect("rollback edge must exist").data = data;
            }
            Undo::NodePresentation { node, presentation } => {
                document
                    .set_node_presentation(node, presentation)
                    .expect("rollback presentation node must exist");
            }
        }
    }
}
