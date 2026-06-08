use smol_str::SmolStr;

use crate::{ANodeId, ExposedDeclId, FacetId, ValueTypeId};

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ANodeFieldPath {
    pub node: ANodeId,
    pub field: SmolStr,
}

impl ANodeFieldPath {
    #[must_use]
    pub fn new(node: ANodeId, field: impl Into<SmolStr>) -> Self {
        Self {
            node,
            field: field.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ValueTypeSpec {
    Exact(ValueTypeId),
    Facet(FacetId),
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ParamUiHints {
    pub editor: Option<String>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub step: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExposedParam {
    pub decl_id: ExposedDeclId,
    pub label: String,
    pub description: Option<String>,
    pub target: ANodeFieldPath,
    pub value_type: ValueTypeSpec,
    pub ui: ParamUiHints,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExposedInput {
    pub decl_id: ExposedDeclId,
    pub label: String,
    pub target: ANodeFieldPath,
    pub value_type: ValueTypeSpec,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExposedOutput {
    pub decl_id: ExposedDeclId,
    pub label: String,
    pub source: ANodeFieldPath,
    pub value_type: ValueTypeSpec,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExposedAction {
    pub decl_id: ExposedDeclId,
    pub label: String,
    pub target: ANodeFieldPath,
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ExposedSurface {
    pub params: Vec<ExposedParam>,
    pub inputs: Vec<ExposedInput>,
    pub outputs: Vec<ExposedOutput>,
    pub actions: Vec<ExposedAction>,
}
