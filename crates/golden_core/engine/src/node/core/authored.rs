use std::sync::Arc;

use crate::parameter::ParameterAuthoredState;

/// Complete in-memory authored state for history and callback restoration.
/// This is deliberately separate from project JSON: cloning a snapshot retains
/// native storage instead of serializing it or rebuilding derived indexes.
#[derive(Clone)]
pub struct NodeAuthoredData(Payload);

#[derive(Clone)]
enum Payload {
    Json(Arc<serde_json::Value>),
    Parameter(Arc<ParameterAuthoredState>),
    Inherited(Arc<InheritedData>),
}

#[derive(PartialEq)]
struct InheritedData {
    base: NodeAuthoredData,
    own: serde_json::Value,
}

impl NodeAuthoredData {
    /// Captures a node's complete JSON-authored fields. Project codecs may still
    /// use sparse records; history captures must include default-valued fields.
    pub fn json(data: serde_json::Value) -> Self {
        Self(Payload::Json(Arc::new(data)))
    }

    /// Composes an inherited native snapshot with this node's complete fields.
    pub fn inherited(base: Self, own: serde_json::Value) -> Self {
        Self(Payload::Inherited(Arc::new(InheritedData { base, own })))
    }

    /// Borrows a JSON snapshot without converting native payloads to JSON.
    pub fn as_json(&self) -> Result<&serde_json::Value, String> {
        match &self.0 {
            Payload::Json(data) => Ok(data),
            _ => Err("expected a JSON authored snapshot".into()),
        }
    }

    /// Borrows this node's fields and shares its inherited snapshot. Neither
    /// operation clones JSON trees or native enum entries.
    pub fn inherited_parts(&self) -> Result<(Self, &serde_json::Value), String> {
        match &self.0 {
            Payload::Inherited(data) => Ok((data.base.clone(), &data.own)),
            _ => Err("expected an inherited authored snapshot".into()),
        }
    }

    pub(crate) fn parameter(state: ParameterAuthoredState) -> Self {
        Self(Payload::Parameter(Arc::new(state)))
    }

    pub(crate) fn into_parameter(self) -> Result<ParameterAuthoredState, String> {
        match self.0 {
            Payload::Parameter(state) => Ok(Arc::unwrap_or_clone(state)),
            _ => Err("expected a parameter authored snapshot".into()),
        }
    }
}

impl PartialEq for NodeAuthoredData {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Payload::Json(left), Payload::Json(right)) => Arc::ptr_eq(left, right) || left == right,
            (Payload::Parameter(left), Payload::Parameter(right)) => Arc::ptr_eq(left, right) || left == right,
            (Payload::Inherited(left), Payload::Inherited(right)) => Arc::ptr_eq(left, right) || left == right,
            _ => false,
        }
    }
}

impl std::fmt::Debug for NodeAuthoredData {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = match self.0 {
            Payload::Json(_) => "JSON",
            Payload::Parameter(_) => "Parameter",
            Payload::Inherited(_) => "Inherited",
        };
        formatter.debug_tuple("NodeAuthoredData").field(&kind).finish()
    }
}
