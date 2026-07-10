use std::{collections::BTreeMap, ops::Range};

use golden_model::EntityId;
use golden_values::Value;
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenerationId(pub EntityId);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InputId(pub SmolStr);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectSinkId(pub SmolStr);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StableStateKey(pub SmolStr);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarType {
    Bool,
    Integer,
    Float,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScalarValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
}

impl ScalarValue {
    pub const fn value_type(self) -> ScalarType {
        match self {
            Self::Bool(_) => ScalarType::Bool,
            Self::Integer(_) => ScalarType::Integer,
            Self::Float(_) => ScalarType::Float,
        }
    }
}

impl TryFrom<&Value> for ScalarValue {
    type Error = ScalarConversionError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value {
            Value::Bool(value) => Ok(Self::Bool(*value)),
            Value::Integer(value) => Ok(Self::Integer(*value)),
            Value::Float(value) => Ok(Self::Float(value.get())),
            other => Err(ScalarConversionError::Unsupported(other.type_id())),
        }
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ScalarConversionError {
    #[error("value type is not supported by dense scalar arenas: {0}")]
    Unsupported(&'static str),
}

#[derive(Clone, Debug)]
pub struct SlotSpec {
    pub key: SmolStr,
    pub initial: ScalarValue,
}

#[derive(Clone, Debug)]
pub struct InputBindingSpec {
    pub id: InputId,
    pub slot: u32,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EffectOrder {
    pub processor: u32,
    pub lane: u32,
    pub operation: u32,
}

#[derive(Clone, Debug)]
pub enum OperationSpec {
    Copy {
        input: u32,
        output: u32,
        batch: u32,
    },
    AddFloat {
        left: u32,
        right: u32,
        output: u32,
        batch: u32,
    },
    MultiplyFloat {
        left: u32,
        right: u32,
        output: u32,
        batch: u32,
    },
    ConditionGateFloat {
        condition: u32,
        value: u32,
        output: u32,
        batch: u32,
    },
    Emit {
        input: u32,
        sink: EffectSinkId,
        order: EffectOrder,
        batch: u32,
    },
}

impl OperationSpec {
    pub const fn batch(&self) -> u32 {
        match self {
            Self::Copy { batch, .. }
            | Self::AddFloat { batch, .. }
            | Self::MultiplyFloat { batch, .. }
            | Self::ConditionGateFloat { batch, .. }
            | Self::Emit { batch, .. } => *batch,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StateSpec {
    pub key: StableStateKey,
    pub initial: f64,
}

#[derive(Clone, Debug)]
pub struct GenerationSpec {
    pub id: GenerationId,
    pub slots: Vec<SlotSpec>,
    pub inputs: Vec<InputBindingSpec>,
    pub operations: Vec<OperationSpec>,
    pub state: Vec<StateSpec>,
    pub sparse_threshold_percent: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DenseSlot {
    pub value_type: ScalarType,
    pub index: u32,
    pub route: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInputSlot {
    pub generation: GenerationId,
    pub slot: DenseSlot,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputUpdate {
    pub slot: DirectInputSlot,
    pub value: ScalarValue,
}

#[derive(Clone, Debug)]
pub enum RuntimeOperation {
    Copy {
        input: DenseSlot,
        output: DenseSlot,
    },
    AddFloat {
        left: DenseSlot,
        right: DenseSlot,
        output: DenseSlot,
    },
    MultiplyFloat {
        left: DenseSlot,
        right: DenseSlot,
        output: DenseSlot,
    },
    ConditionGateFloat {
        condition: DenseSlot,
        value: DenseSlot,
        output: DenseSlot,
    },
    Emit {
        input: DenseSlot,
        sink: EffectSinkId,
        order: EffectOrder,
    },
}

impl RuntimeOperation {
    pub fn inputs(&self) -> impl Iterator<Item = DenseSlot> {
        let mut inputs = [None, None];
        match self {
            Self::Copy { input, .. } | Self::Emit { input, .. } => inputs[0] = Some(*input),
            Self::AddFloat { left, right, .. } | Self::MultiplyFloat { left, right, .. } => {
                inputs = [Some(*left), Some(*right)];
            }
            Self::ConditionGateFloat { condition, value, .. } => {
                inputs = [Some(*condition), Some(*value)];
            }
        }
        inputs.into_iter().flatten()
    }

    pub const fn output(&self) -> Option<DenseSlot> {
        match self {
            Self::Copy { output, .. }
            | Self::AddFloat { output, .. }
            | Self::MultiplyFloat { output, .. }
            | Self::ConditionGateFloat { output, .. } => Some(*output),
            Self::Emit { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RuntimeBatch {
    pub id: u32,
    pub operations: Range<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct DenseArenaDefaults {
    pub bools: Vec<bool>,
    pub integers: Vec<i64>,
    pub floats: Vec<f64>,
}

#[derive(Clone, Debug)]
pub struct RuntimeGeneration {
    pub id: GenerationId,
    pub slots: Vec<DenseSlot>,
    pub slot_keys: Vec<SmolStr>,
    pub inputs: BTreeMap<InputId, DirectInputSlot>,
    pub operations: Vec<RuntimeOperation>,
    pub batches: Vec<RuntimeBatch>,
    pub route_offsets: Vec<u32>,
    pub route_operations: Vec<u32>,
    pub defaults: DenseArenaDefaults,
    pub state: Vec<StateSpec>,
    pub sparse_threshold_percent: u8,
    pub effect_capacity: usize,
}

impl RuntimeGeneration {
    pub fn direct_input(&self, input: &InputId) -> Option<DirectInputSlot> {
        self.inputs.get(input).copied()
    }

    pub fn consumers(&self, route: u32) -> &[u32] {
        let start = self.route_offsets[route as usize] as usize;
        let end = self.route_offsets[route as usize + 1] as usize;
        &self.route_operations[start..end]
    }
}
