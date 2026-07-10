use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{
    DenseSlot, DirectInputSlot, GenerationSpec, OperationSpec, RuntimeBatch, RuntimeGeneration, RuntimeOperation,
    ScalarType, ScalarValue,
};

#[derive(Default)]
pub struct GenerationCompiler;

impl GenerationCompiler {
    pub fn compile(&self, spec: GenerationSpec) -> Result<RuntimeGeneration, RuntimeCompileError> {
        if !(1..=100).contains(&spec.sparse_threshold_percent) {
            return Err(RuntimeCompileError::InvalidSparseThreshold(
                spec.sparse_threshold_percent,
            ));
        }
        let mut defaults = crate::model::DenseArenaDefaults::default();
        let mut slots = Vec::with_capacity(spec.slots.len());
        let mut slot_keys = Vec::with_capacity(spec.slots.len());
        let mut unique_slot_keys = BTreeSet::new();
        for (route, slot) in spec.slots.into_iter().enumerate() {
            if !unique_slot_keys.insert(slot.key.clone()) {
                return Err(RuntimeCompileError::DuplicateSlotKey(slot.key));
            }
            let (value_type, index) = match slot.initial {
                ScalarValue::Bool(value) => {
                    let index = push_index(&mut defaults.bools, value)?;
                    (ScalarType::Bool, index)
                }
                ScalarValue::Integer(value) => {
                    let index = push_index(&mut defaults.integers, value)?;
                    (ScalarType::Integer, index)
                }
                ScalarValue::Float(value) if value.is_finite() => {
                    let index = push_index(&mut defaults.floats, value)?;
                    (ScalarType::Float, index)
                }
                ScalarValue::Float(_) => return Err(RuntimeCompileError::NonFiniteFloat),
            };
            slots.push(DenseSlot {
                value_type,
                index,
                route: u32::try_from(route).map_err(|_| RuntimeCompileError::CapacityOverflow)?,
            });
            slot_keys.push(slot.key);
        }

        let generation_id = spec.id;
        let mut inputs = BTreeMap::new();
        for binding in spec.inputs {
            let slot = slot(&slots, binding.slot)?;
            if inputs
                .insert(
                    binding.id.clone(),
                    DirectInputSlot {
                        generation: generation_id,
                        slot,
                    },
                )
                .is_some()
            {
                return Err(RuntimeCompileError::DuplicateInput(binding.id));
            }
        }

        let mut operations = Vec::with_capacity(spec.operations.len());
        let mut batches = Vec::new();
        let mut current_batch = None::<(u32, u32)>;
        let mut previous_batch = None;
        let mut effect_orders = BTreeSet::new();
        let mut effect_capacity = 0_usize;
        for authored in spec.operations {
            let batch = authored.batch();
            if previous_batch.is_some_and(|previous| batch < previous) {
                return Err(RuntimeCompileError::BatchOrder);
            }
            if previous_batch != Some(batch) {
                if let Some((id, start)) = current_batch.take() {
                    batches.push(RuntimeBatch {
                        id,
                        operations: start
                            ..u32::try_from(operations.len()).map_err(|_| RuntimeCompileError::CapacityOverflow)?,
                    });
                }
                current_batch = Some((
                    batch,
                    u32::try_from(operations.len()).map_err(|_| RuntimeCompileError::CapacityOverflow)?,
                ));
                previous_batch = Some(batch);
            }
            let operation = compile_operation(&slots, authored)?;
            if let RuntimeOperation::Emit { order, .. } = &operation {
                if !effect_orders.insert(*order) {
                    return Err(RuntimeCompileError::DuplicateEffectOrder(*order));
                }
                effect_capacity += 1;
            }
            operations.push(operation);
        }
        if let Some((id, start)) = current_batch {
            batches.push(RuntimeBatch {
                id,
                operations: start
                    ..u32::try_from(operations.len()).map_err(|_| RuntimeCompileError::CapacityOverflow)?,
            });
        }
        validate_topology(&operations)?;
        let (route_offsets, route_operations) = compile_routes(slots.len(), &operations)?;
        let mut state_keys = BTreeSet::new();
        for state in &spec.state {
            if !state.initial.is_finite() {
                return Err(RuntimeCompileError::NonFiniteFloat);
            }
            if !state_keys.insert(state.key.clone()) {
                return Err(RuntimeCompileError::DuplicateStateKey(state.key.clone()));
            }
        }
        Ok(RuntimeGeneration {
            id: generation_id,
            slots,
            slot_keys,
            inputs,
            operations,
            batches,
            route_offsets,
            route_operations,
            defaults,
            state: spec.state,
            sparse_threshold_percent: spec.sparse_threshold_percent,
            effect_capacity,
        })
    }
}

fn push_index<T>(values: &mut Vec<T>, value: T) -> Result<u32, RuntimeCompileError> {
    let index = u32::try_from(values.len()).map_err(|_| RuntimeCompileError::CapacityOverflow)?;
    values.push(value);
    Ok(index)
}

fn slot(slots: &[DenseSlot], index: u32) -> Result<DenseSlot, RuntimeCompileError> {
    slots
        .get(index as usize)
        .copied()
        .ok_or(RuntimeCompileError::MissingSlot(index))
}

fn typed_slot(slots: &[DenseSlot], index: u32, expected: ScalarType) -> Result<DenseSlot, RuntimeCompileError> {
    let slot = slot(slots, index)?;
    if slot.value_type != expected {
        return Err(RuntimeCompileError::TypeMismatch {
            slot: index,
            expected,
            actual: slot.value_type,
        });
    }
    Ok(slot)
}

fn compile_operation(slots: &[DenseSlot], operation: OperationSpec) -> Result<RuntimeOperation, RuntimeCompileError> {
    Ok(match operation {
        OperationSpec::Copy { input, output, .. } => {
            let input = slot(slots, input)?;
            let output = slot(slots, output)?;
            if input.value_type != output.value_type {
                return Err(RuntimeCompileError::TypeMismatch {
                    slot: output.route,
                    expected: input.value_type,
                    actual: output.value_type,
                });
            }
            RuntimeOperation::Copy { input, output }
        }
        OperationSpec::AddFloat {
            left, right, output, ..
        } => RuntimeOperation::AddFloat {
            left: typed_slot(slots, left, ScalarType::Float)?,
            right: typed_slot(slots, right, ScalarType::Float)?,
            output: typed_slot(slots, output, ScalarType::Float)?,
        },
        OperationSpec::MultiplyFloat {
            left, right, output, ..
        } => RuntimeOperation::MultiplyFloat {
            left: typed_slot(slots, left, ScalarType::Float)?,
            right: typed_slot(slots, right, ScalarType::Float)?,
            output: typed_slot(slots, output, ScalarType::Float)?,
        },
        OperationSpec::ConditionGateFloat {
            condition,
            value,
            output,
            ..
        } => RuntimeOperation::ConditionGateFloat {
            condition: typed_slot(slots, condition, ScalarType::Bool)?,
            value: typed_slot(slots, value, ScalarType::Float)?,
            output: typed_slot(slots, output, ScalarType::Float)?,
        },
        OperationSpec::Emit { input, sink, order, .. } => RuntimeOperation::Emit {
            input: slot(slots, input)?,
            sink,
            order,
        },
    })
}

fn validate_topology(operations: &[RuntimeOperation]) -> Result<(), RuntimeCompileError> {
    let mut producer = BTreeMap::<u32, usize>::new();
    for (index, operation) in operations.iter().enumerate() {
        if let Some(output) = operation.output()
            && producer.insert(output.route, index).is_some()
        {
            return Err(RuntimeCompileError::DuplicateProducer(output.route));
        }
    }
    for (index, operation) in operations.iter().enumerate() {
        for input in operation.inputs() {
            if producer.get(&input.route).is_some_and(|producer| *producer >= index) {
                return Err(RuntimeCompileError::OperationOrder);
            }
        }
    }
    Ok(())
}

fn compile_routes(
    slot_count: usize,
    operations: &[RuntimeOperation],
) -> Result<(Vec<u32>, Vec<u32>), RuntimeCompileError> {
    let mut routes = vec![Vec::<u32>::new(); slot_count];
    for (operation_index, operation) in operations.iter().enumerate() {
        let operation_index = u32::try_from(operation_index).map_err(|_| RuntimeCompileError::CapacityOverflow)?;
        for input in operation.inputs() {
            routes[input.route as usize].push(operation_index);
        }
    }
    let mut offsets = Vec::with_capacity(slot_count + 1);
    let mut flattened = Vec::new();
    offsets.push(0);
    for route in routes {
        flattened.extend(route);
        offsets.push(u32::try_from(flattened.len()).map_err(|_| RuntimeCompileError::CapacityOverflow)?);
    }
    Ok((offsets, flattened))
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeCompileError {
    #[error("runtime slot does not exist: {0}")]
    MissingSlot(u32),
    #[error("runtime slot {slot} expected {expected:?}, received {actual:?}")]
    TypeMismatch {
        slot: u32,
        expected: ScalarType,
        actual: ScalarType,
    },
    #[error("runtime float values must be finite")]
    NonFiniteFloat,
    #[error("runtime slot key is duplicated: {0}")]
    DuplicateSlotKey(smol_str::SmolStr),
    #[error("runtime input is duplicated: {0:?}")]
    DuplicateInput(crate::InputId),
    #[error("runtime state key is duplicated: {0:?}")]
    DuplicateStateKey(crate::StableStateKey),
    #[error("effect commit order is duplicated: {0:?}")]
    DuplicateEffectOrder(crate::EffectOrder),
    #[error("operation batches must be authored in ascending order")]
    BatchOrder,
    #[error("operations must be authored in dependency order")]
    OperationOrder,
    #[error("runtime slot {0} has more than one producer")]
    DuplicateProducer(u32),
    #[error("sparse threshold must be between 1 and 100, received {0}")]
    InvalidSparseThreshold(u8),
    #[error("runtime generation exceeds addressable capacity")]
    CapacityOverflow,
}
