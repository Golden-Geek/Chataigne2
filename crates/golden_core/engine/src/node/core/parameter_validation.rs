use super::{Node, NodeId, NodeUuid};
use crate::parameter::ParamValue;

/// Borrowed graph access for validating a parameter value before it is committed.
/// Owners can enforce domain invariants without constructing a process-tree snapshot.
pub struct ParameterValidationContext<'a> {
    /// Parameter being edited.
    pub parameter: NodeId,
    /// Value after the parameter's own type/constraint preparation.
    pub value: &'a ParamValue,
    lookup: &'a dyn Fn(NodeId) -> Option<&'a dyn Node>,
    resolve: &'a dyn Fn(NodeUuid) -> Option<NodeId>,
}

impl<'a> ParameterValidationContext<'a> {
    pub(crate) fn new(
        parameter: NodeId,
        value: &'a ParamValue,
        lookup: &'a dyn Fn(NodeId) -> Option<&'a dyn Node>,
        resolve: &'a dyn Fn(NodeUuid) -> Option<NodeId>,
    ) -> Self {
        Self {
            parameter,
            value,
            lookup,
            resolve,
        }
    }
    /// Reads an existing node without copying the graph.
    pub fn node(&self, id: NodeId) -> Option<&'a dyn Node> {
        (self.lookup)(id)
    }
    /// Resolves a persistent identity in the current graph.
    pub fn resolve(&self, uuid: NodeUuid) -> Option<NodeId> {
        (self.resolve)(uuid)
    }
}
