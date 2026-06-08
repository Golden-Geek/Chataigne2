use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::{FacetId, ValueTypeId};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct TypeVar(SmolStr);

impl TypeVar {
    #[must_use]
    pub fn new(value: impl Into<SmolStr>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for TypeVar {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TypeBindingSource {
    Default,
    InferredFromConnection,
    ForcedByModel,
    ForcedByUser,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeBinding {
    pub value_type: ValueTypeId,
    pub source: TypeBindingSource,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeBindings {
    values: IndexMap<TypeVar, TypeBinding>,
}

impl TypeBindings {
    pub fn insert(
        &mut self,
        variable: impl Into<TypeVar>,
        value_type: ValueTypeId,
        source: TypeBindingSource,
    ) -> Option<TypeBinding> {
        self.values.insert(variable.into(), TypeBinding { value_type, source })
    }

    #[must_use]
    pub fn get(&self, variable: &TypeVar) -> Option<&TypeBinding> {
        self.values.get(variable)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&TypeVar, &TypeBinding)> {
        self.values.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TypeConstraint {
    Any,
    Exact(ValueTypeId),
    Facet(FacetId),
    Primitive,
    NumericLike,
    Generic(TypeVar),
    OneOf(Vec<TypeConstraint>),
}
