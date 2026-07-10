use golden_model::{EntityId, Revision};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    DiagnosticSeverity, GraphComment, GraphDiagnostic, GraphDocument, GraphDomain, GraphEdge, GraphEdgeId, GraphGroup,
    GraphNode, GraphNodeId, NodePresentation, PortDirection, ViewportBookmark,
};

pub enum GraphOperation<D: GraphDomain> {
    InsertNode {
        node: GraphNode<D>,
        presentation: Option<NodePresentation>,
    },
    RemoveNode {
        node: GraphNodeId,
    },
    ReplaceNode {
        node: GraphNodeId,
        data: D::NodeData,
    },
    Connect {
        edge: GraphEdge<D>,
    },
    Disconnect {
        edge: GraphEdgeId,
    },
    SetNodePresentation {
        node: GraphNodeId,
        presentation: NodePresentation,
    },
    UpsertComment(GraphComment),
    RemoveComment(EntityId),
    UpsertGroup(GraphGroup),
    RemoveGroup(EntityId),
    UpsertBookmark(ViewportBookmark),
    RemoveBookmark(EntityId),
}

pub struct GraphTransaction<D: GraphDomain> {
    pub expected_revision: Revision,
    pub operations: Vec<GraphOperation<D>>,
}

impl<D: GraphDomain> GraphTransaction<D> {
    pub fn new(expected_revision: Revision) -> Self {
        Self {
            expected_revision,
            operations: Vec::new(),
        }
    }

    pub fn push(&mut self, operation: GraphOperation<D>) {
        self.operations.push(operation);
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum GraphChange {
    NodeInserted(GraphNodeId),
    NodeRemoved(GraphNodeId),
    NodeReplaced(GraphNodeId),
    EdgeInserted(GraphEdgeId),
    EdgeRemoved(GraphEdgeId),
    NodePresentationChanged(GraphNodeId),
    CommentChanged(EntityId),
    CommentRemoved(EntityId),
    GroupChanged(EntityId),
    GroupRemoved(EntityId),
    BookmarkChanged(EntityId),
    BookmarkRemoved(EntityId),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GraphChangeSet {
    pub before: Revision,
    pub after: Revision,
    pub changes: Vec<GraphChange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphCommit {
    pub change_set: GraphChangeSet,
    pub diagnostics: Vec<GraphDiagnostic>,
}

enum Undo<D: GraphDomain> {
    RemoveNode(GraphNodeId),
    RestoreNode {
        node: GraphNode<D>,
        presentation: Option<NodePresentation>,
        edges: Vec<GraphEdge<D>>,
    },
    RestoreNodeData {
        node: GraphNodeId,
        data: D::NodeData,
    },
    RemoveEdge(GraphEdgeId),
    RestoreEdge(GraphEdge<D>),
    RestorePresentation {
        node: GraphNodeId,
        presentation: Option<NodePresentation>,
    },
    RestoreComment {
        id: EntityId,
        comment: Option<GraphComment>,
    },
    RestoreGroup {
        id: EntityId,
        group: Option<GraphGroup>,
    },
    RestoreBookmark {
        id: EntityId,
        bookmark: Option<ViewportBookmark>,
    },
}

impl<D: GraphDomain> GraphDocument<D> {
    pub fn apply(&mut self, transaction: GraphTransaction<D>) -> Result<GraphCommit, GraphTransactionError> {
        if transaction.expected_revision != self.revision() {
            return Err(GraphTransactionError::RevisionConflict {
                expected: transaction.expected_revision,
                actual: self.revision(),
            });
        }
        if transaction.operations.is_empty() {
            return Err(GraphTransactionError::EmptyTransaction);
        }
        let after = self.revision().next().ok_or(GraphTransactionError::RevisionExhausted)?;
        let mut undo = Vec::with_capacity(transaction.operations.len());
        let mut changes = Vec::with_capacity(transaction.operations.len());

        for operation in transaction.operations {
            if let Err(error) = self.apply_operation(operation, &mut undo, &mut changes) {
                self.rollback(undo);
                return Err(error);
            }
        }

        let change_set = GraphChangeSet {
            before: self.revision(),
            after,
            changes,
        };
        let diagnostics = self.domain().validate_graph(self, &change_set);
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        {
            self.rollback(undo);
            return Err(GraphTransactionError::Validation(diagnostics));
        }
        self.set_revision(after);
        debug_assert!(self.assert_invariants().is_ok());
        Ok(GraphCommit {
            change_set,
            diagnostics,
        })
    }

    fn apply_operation(
        &mut self,
        operation: GraphOperation<D>,
        undo: &mut Vec<Undo<D>>,
        changes: &mut Vec<GraphChange>,
    ) -> Result<(), GraphTransactionError> {
        match operation {
            GraphOperation::InsertNode { node, presentation } => {
                if self.node(node.id).is_some() {
                    return Err(GraphTransactionError::DuplicateNode(node.id));
                }
                let id = node.id;
                self.insert_node_internal(node, presentation);
                undo.push(Undo::RemoveNode(id));
                changes.push(GraphChange::NodeInserted(id));
            }
            GraphOperation::RemoveNode { node } => {
                let Some(removed) = self.remove_node_internal(node) else {
                    return Err(GraphTransactionError::MissingNode(node));
                };
                changes.extend(removed.edges.iter().map(|edge| GraphChange::EdgeRemoved(edge.id)));
                changes.push(GraphChange::NodeRemoved(node));
                undo.push(Undo::RestoreNode {
                    node: removed.node,
                    presentation: removed.presentation,
                    edges: removed.edges,
                });
            }
            GraphOperation::ReplaceNode { node, data } => {
                let Some(current) = self.node_data_mut(node) else {
                    return Err(GraphTransactionError::MissingNode(node));
                };
                let previous = std::mem::replace(current, data);
                undo.push(Undo::RestoreNodeData { node, data: previous });
                changes.push(GraphChange::NodeReplaced(node));
            }
            GraphOperation::Connect { edge } => {
                if self.edge(edge.id).is_some() {
                    return Err(GraphTransactionError::DuplicateEdge(edge.id));
                }
                if !self.has_port(edge.from, PortDirection::Output) {
                    return Err(GraphTransactionError::MissingPort(edge.from));
                }
                if !self.has_port(edge.to, PortDirection::Input) {
                    return Err(GraphTransactionError::MissingPort(edge.to));
                }
                self.domain()
                    .validate_connection(self, edge.from, edge.to, &edge.data)
                    .map_err(GraphTransactionError::Connection)?;
                let id = edge.id;
                self.insert_edge_internal(edge);
                undo.push(Undo::RemoveEdge(id));
                changes.push(GraphChange::EdgeInserted(id));
            }
            GraphOperation::Disconnect { edge } => {
                let Some(record) = self.remove_edge_internal(edge) else {
                    return Err(GraphTransactionError::MissingEdge(edge));
                };
                undo.push(Undo::RestoreEdge(record));
                changes.push(GraphChange::EdgeRemoved(edge));
            }
            GraphOperation::SetNodePresentation { node, presentation } => {
                if self.node(node).is_none() {
                    return Err(GraphTransactionError::MissingNode(node));
                }
                let previous = self.presentation_mut().nodes.insert(node, presentation);
                undo.push(Undo::RestorePresentation {
                    node,
                    presentation: previous,
                });
                changes.push(GraphChange::NodePresentationChanged(node));
            }
            GraphOperation::UpsertComment(comment) => {
                let id = comment.id;
                let previous = self.presentation_mut().comments.insert(id, comment);
                undo.push(Undo::RestoreComment { id, comment: previous });
                changes.push(GraphChange::CommentChanged(id));
            }
            GraphOperation::RemoveComment(id) => {
                let comment = self
                    .presentation_mut()
                    .comments
                    .remove(&id)
                    .ok_or(GraphTransactionError::MissingComment(id))?;
                undo.push(Undo::RestoreComment {
                    id,
                    comment: Some(comment),
                });
                changes.push(GraphChange::CommentRemoved(id));
            }
            GraphOperation::UpsertGroup(group) => {
                if let Some(missing) = group.nodes.iter().find(|node| self.node(**node).is_none()) {
                    return Err(GraphTransactionError::MissingNode(*missing));
                }
                let id = group.id;
                let previous = self.presentation_mut().groups.insert(id, group);
                undo.push(Undo::RestoreGroup { id, group: previous });
                changes.push(GraphChange::GroupChanged(id));
            }
            GraphOperation::RemoveGroup(id) => {
                let group = self
                    .presentation_mut()
                    .groups
                    .remove(&id)
                    .ok_or(GraphTransactionError::MissingGroup(id))?;
                undo.push(Undo::RestoreGroup { id, group: Some(group) });
                changes.push(GraphChange::GroupRemoved(id));
            }
            GraphOperation::UpsertBookmark(bookmark) => {
                let id = bookmark.id;
                let previous = self.presentation_mut().bookmarks.insert(id, bookmark);
                undo.push(Undo::RestoreBookmark { id, bookmark: previous });
                changes.push(GraphChange::BookmarkChanged(id));
            }
            GraphOperation::RemoveBookmark(id) => {
                let bookmark = self
                    .presentation_mut()
                    .bookmarks
                    .remove(&id)
                    .ok_or(GraphTransactionError::MissingBookmark(id))?;
                undo.push(Undo::RestoreBookmark {
                    id,
                    bookmark: Some(bookmark),
                });
                changes.push(GraphChange::BookmarkRemoved(id));
            }
        }
        Ok(())
    }

    fn rollback(&mut self, undo: Vec<Undo<D>>) {
        for operation in undo.into_iter().rev() {
            match operation {
                Undo::RemoveNode(node) => {
                    self.remove_node_internal(node);
                }
                Undo::RestoreNode {
                    node,
                    presentation,
                    edges,
                } => {
                    self.insert_node_internal(node, presentation);
                    for edge in edges {
                        self.insert_edge_internal(edge);
                    }
                }
                Undo::RestoreNodeData { node, data } => {
                    if let Some(current) = self.node_data_mut(node) {
                        *current = data;
                    }
                }
                Undo::RemoveEdge(edge) => {
                    self.remove_edge_internal(edge);
                }
                Undo::RestoreEdge(edge) => self.insert_edge_internal(edge),
                Undo::RestorePresentation { node, presentation } => match presentation {
                    Some(presentation) => {
                        self.presentation_mut().nodes.insert(node, presentation);
                    }
                    None => {
                        self.presentation_mut().nodes.remove(&node);
                    }
                },
                Undo::RestoreComment { id, comment } => match comment {
                    Some(comment) => {
                        self.presentation_mut().comments.insert(id, comment);
                    }
                    None => {
                        self.presentation_mut().comments.remove(&id);
                    }
                },
                Undo::RestoreGroup { id, group } => match group {
                    Some(group) => {
                        self.presentation_mut().groups.insert(id, group);
                    }
                    None => {
                        self.presentation_mut().groups.remove(&id);
                    }
                },
                Undo::RestoreBookmark { id, bookmark } => match bookmark {
                    Some(bookmark) => {
                        self.presentation_mut().bookmarks.insert(id, bookmark);
                    }
                    None => {
                        self.presentation_mut().bookmarks.remove(&id);
                    }
                },
            }
        }
        debug_assert!(self.assert_invariants().is_ok());
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum GraphTransactionError {
    #[error("graph revision conflict: expected {expected:?}, actual {actual:?}")]
    RevisionConflict { expected: Revision, actual: Revision },
    #[error("empty graph transactions are forbidden")]
    EmptyTransaction,
    #[error("graph revision space is exhausted")]
    RevisionExhausted,
    #[error("node already exists: {0:?}")]
    DuplicateNode(GraphNodeId),
    #[error("node does not exist: {0:?}")]
    MissingNode(GraphNodeId),
    #[error("edge already exists: {0:?}")]
    DuplicateEdge(GraphEdgeId),
    #[error("edge does not exist: {0:?}")]
    MissingEdge(GraphEdgeId),
    #[error("port does not exist with the required direction: {0:?}")]
    MissingPort(crate::PortRef),
    #[error("comment does not exist: {0:?}")]
    MissingComment(EntityId),
    #[error("group does not exist: {0:?}")]
    MissingGroup(EntityId),
    #[error("viewport bookmark does not exist: {0:?}")]
    MissingBookmark(EntityId),
    #[error("connection rejected: {0:?}")]
    Connection(GraphDiagnostic),
    #[error("graph validation rejected the transaction")]
    Validation(Vec<GraphDiagnostic>),
}
