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
    proposed: Option<&'a std::collections::HashMap<NodeId, ParamValue>>,
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
            proposed: None,
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
    pub(crate) fn with_proposed(mut self, values: &'a std::collections::HashMap<NodeId, ParamValue>) -> Self {
        self.proposed = Some(values);
        self
    }
    /// Reads the final normalized value in this write, or its current committed value.
    pub fn parameter_value(&self, id: NodeId) -> Option<ParamValue> {
        if let Some(value) = self.proposed.and_then(|values| values.get(&id)) {
            return Some(value.clone());
        }
        if id == self.parameter {
            return Some(self.value.clone());
        }
        self.node(id)?.engine_param_snapshot().map(|snapshot| snapshot.value)
    }
}
