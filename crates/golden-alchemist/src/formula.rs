use std::collections::BTreeMap;

use golden_graph::{GraphDocument, PortRef};
use golden_model::EntityId;
use golden_values::{Value, ValueTypeId};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::AlchemistGraphDomain;

macro_rules! formula_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub EntityId);

        impl $name {
            pub fn new() -> Self {
                Self(EntityId::new())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

formula_id!(FormulaId);
formula_id!(FormulaPropertyId);
formula_id!(SurfaceItemId);
formula_id!(ManagedRegionId);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormulaSchema {
    pub version: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FormulaProperty {
    pub id: FormulaPropertyId,
    pub name: SmolStr,
    pub value_type: ValueTypeId,
    pub default: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceInput {
    pub id: SurfaceItemId,
    pub label: SmolStr,
    pub target: PortRef,
    pub value_type: ValueTypeId,
    pub default: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceOutput {
    pub id: SurfaceItemId,
    pub label: SmolStr,
    pub source: PortRef,
    pub value_type: ValueTypeId,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FormulaSurface {
    pub inputs: Vec<SurfaceInput>,
    pub outputs: Vec<SurfaceOutput>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManagedRegionDefinition {
    pub id: ManagedRegionId,
    pub label: SmolStr,
    pub item_type: SmolStr,
    pub minimum_items: usize,
    pub maximum_items: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FormulaMetadata {
    pub name: SmolStr,
    pub description: String,
    pub tags: Vec<SmolStr>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FormulaDefaults {
    pub properties: BTreeMap<FormulaPropertyId, Value>,
}

pub struct AlchemistFormula {
    pub id: FormulaId,
    pub schema: FormulaSchema,
    pub graph: GraphDocument<AlchemistGraphDomain>,
    pub properties: Vec<FormulaProperty>,
    pub surface: FormulaSurface,
    pub managed_regions: Vec<ManagedRegionDefinition>,
    pub metadata: FormulaMetadata,
    pub defaults: FormulaDefaults,
}
