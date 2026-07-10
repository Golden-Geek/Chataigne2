use std::{
    cmp::Reverse,
    collections::{BTreeMap, BinaryHeap},
    sync::Arc,
};

use arc_swap::ArcSwap;
use thiserror::Error;

use crate::{
    DenseSlot, DirectInputSlot, EffectOrder, EffectSinkId, GenerationId, InputUpdate, RuntimeGeneration,
    RuntimeOperation, ScalarType, ScalarValue, StableStateKey,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionMode {
    Idle,
    Sparse,
    Dense,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommittedEffect {
    pub sink: EffectSinkId,
    pub value: ScalarValue,
    pub order: EffectOrder,
}

pub trait EffectCommitter {
    fn commit(&mut self, effect: &CommittedEffect);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TickMetrics {
    pub generation: GenerationId,
    pub execution_mode: ExecutionMode,
    pub changed_inputs: usize,
    pub executed_operations: usize,
    pub committed_effects: usize,
    pub project_snapshots: usize,
    pub topology_traversals: usize,
    pub binding_rebuilds: usize,
    pub semantic_allocations: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationSwapReport {
    pub previous: GenerationId,
    pub current: GenerationId,
    pub migrated_slots: usize,
    pub migrated_state: usize,
}

#[derive(Clone)]
struct DenseArena {
    bools: Vec<bool>,
    integers: Vec<i64>,
    floats: Vec<f64>,
}

impl DenseArena {
    fn from_generation(generation: &RuntimeGeneration) -> Self {
        Self {
            bools: generation.defaults.bools.clone(),
            integers: generation.defaults.integers.clone(),
            floats: generation.defaults.floats.clone(),
        }
    }

    fn read(&self, slot: DenseSlot) -> ScalarValue {
        match slot.value_type {
            ScalarType::Bool => ScalarValue::Bool(self.bools[slot.index as usize]),
            ScalarType::Integer => ScalarValue::Integer(self.integers[slot.index as usize]),
            ScalarType::Float => ScalarValue::Float(self.floats[slot.index as usize]),
        }
    }

    fn write(&mut self, slot: DenseSlot, value: ScalarValue) -> Result<bool, SemanticRuntimeError> {
        if slot.value_type != value.value_type() {
            return Err(SemanticRuntimeError::InputTypeMismatch {
                expected: slot.value_type,
                actual: value.value_type(),
            });
        }
        let changed = match (slot.value_type, value) {
            (ScalarType::Bool, ScalarValue::Bool(value)) => replace(&mut self.bools[slot.index as usize], value),
            (ScalarType::Integer, ScalarValue::Integer(value)) => {
                replace(&mut self.integers[slot.index as usize], value)
            }
            (ScalarType::Float, ScalarValue::Float(value)) if value.is_finite() => {
                replace(&mut self.floats[slot.index as usize], value)
            }
            (ScalarType::Float, ScalarValue::Float(_)) => return Err(SemanticRuntimeError::NonFiniteFloat),
            _ => unreachable!("type equality was checked"),
        };
        Ok(changed)
    }
}

fn replace<T: PartialEq>(target: &mut T, value: T) -> bool {
    if *target == value {
        false
    } else {
        *target = value;
        true
    }
}

pub struct SemanticRuntime {
    generation: ArcSwap<RuntimeGeneration>,
    arena: DenseArena,
    state: Vec<f64>,
    dirty_operations: Vec<bool>,
    pending: BinaryHeap<Reverse<u32>>,
    effects: Vec<CommittedEffect>,
    needs_full_evaluation: bool,
}

impl SemanticRuntime {
    pub fn new(generation: Arc<RuntimeGeneration>) -> Self {
        let arena = DenseArena::from_generation(&generation);
        let state = generation.state.iter().map(|state| state.initial).collect();
        let operation_count = generation.operations.len();
        let effect_capacity = generation.effect_capacity;
        Self {
            generation: ArcSwap::from(generation),
            arena,
            state,
            dirty_operations: vec![false; operation_count],
            pending: BinaryHeap::with_capacity(operation_count),
            effects: Vec::with_capacity(effect_capacity),
            needs_full_evaluation: true,
        }
    }

    pub fn generation(&self) -> Arc<RuntimeGeneration> {
        self.generation.load_full()
    }

    pub fn state(&self, key: &StableStateKey) -> Option<f64> {
        let generation = self.generation.load();
        generation
            .state
            .iter()
            .position(|state| &state.key == key)
            .map(|index| self.state[index])
    }

    pub fn set_state(&mut self, key: &StableStateKey, value: f64) -> Result<(), SemanticRuntimeError> {
        if !value.is_finite() {
            return Err(SemanticRuntimeError::NonFiniteFloat);
        }
        let generation = self.generation.load();
        let index = generation
            .state
            .iter()
            .position(|state| &state.key == key)
            .ok_or_else(|| SemanticRuntimeError::MissingState(key.clone()))?;
        self.state[index] = value;
        Ok(())
    }

    pub fn tick(
        &mut self,
        updates: &[InputUpdate],
        committer: &mut impl EffectCommitter,
    ) -> Result<TickMetrics, SemanticRuntimeError> {
        let generation = self.generation.load_full();
        let pending_capacity = self.pending.capacity();
        let effect_capacity = self.effects.capacity();
        self.effects.clear();
        let mut changed_inputs = 0;
        for update in updates {
            validate_direct_slot(&generation, update.slot)?;
            if self.arena.write(update.slot.slot, update.value)? {
                changed_inputs += 1;
                mark_consumers(
                    &generation,
                    update.slot.slot,
                    &mut self.dirty_operations,
                    &mut self.pending,
                );
            }
        }

        let (execution_mode, executed_operations) = if self.needs_full_evaluation {
            self.pending.clear();
            self.dirty_operations.fill(false);
            self.needs_full_evaluation = false;
            let count = execute_dense(&generation, &mut self.arena, &mut self.effects)?;
            (ExecutionMode::Dense, count)
        } else if self.pending.is_empty() {
            (ExecutionMode::Idle, 0)
        } else if self.pending.len() * 100
            >= generation.operations.len() * usize::from(generation.sparse_threshold_percent)
        {
            self.pending.clear();
            self.dirty_operations.fill(false);
            let count = execute_dense(&generation, &mut self.arena, &mut self.effects)?;
            (ExecutionMode::Dense, count)
        } else {
            let count = execute_sparse(
                &generation,
                &mut self.arena,
                &mut self.effects,
                &mut self.dirty_operations,
                &mut self.pending,
            )?;
            (ExecutionMode::Sparse, count)
        };

        self.effects.sort_unstable_by_key(|effect| effect.order);
        for effect in &self.effects {
            committer.commit(effect);
        }
        Ok(TickMetrics {
            generation: generation.id,
            execution_mode,
            changed_inputs,
            executed_operations,
            committed_effects: self.effects.len(),
            project_snapshots: 0,
            topology_traversals: 0,
            binding_rebuilds: 0,
            semantic_allocations: usize::from(self.pending.capacity() != pending_capacity)
                + usize::from(self.effects.capacity() != effect_capacity),
        })
    }

    pub fn swap_generation(&mut self, next: Arc<RuntimeGeneration>) -> GenerationSwapReport {
        let previous = self.generation.load_full();
        let mut next_arena = DenseArena::from_generation(&next);
        let previous_slots = previous
            .slot_keys
            .iter()
            .enumerate()
            .map(|(index, key)| (key, previous.slots[index]))
            .collect::<BTreeMap<_, _>>();
        let mut migrated_slots = 0;
        for (index, key) in next.slot_keys.iter().enumerate() {
            let next_slot = next.slots[index];
            if let Some(previous_slot) = previous_slots.get(key)
                && previous_slot.value_type == next_slot.value_type
            {
                let value = self.arena.read(*previous_slot);
                let _ = next_arena.write(next_slot, value);
                migrated_slots += 1;
            }
        }
        let previous_state = previous
            .state
            .iter()
            .enumerate()
            .map(|(index, state)| (&state.key, self.state[index]))
            .collect::<BTreeMap<_, _>>();
        let mut migrated_state = 0;
        let next_state = next
            .state
            .iter()
            .map(|state| {
                previous_state.get(&state.key).copied().map_or(state.initial, |value| {
                    migrated_state += 1;
                    value
                })
            })
            .collect();
        let report = GenerationSwapReport {
            previous: previous.id,
            current: next.id,
            migrated_slots,
            migrated_state,
        };
        self.arena = next_arena;
        self.state = next_state;
        self.dirty_operations = vec![false; next.operations.len()];
        self.pending = BinaryHeap::with_capacity(next.operations.len());
        self.effects = Vec::with_capacity(next.effect_capacity);
        self.needs_full_evaluation = true;
        self.generation.store(next);
        report
    }
}

fn validate_direct_slot(generation: &RuntimeGeneration, slot: DirectInputSlot) -> Result<(), SemanticRuntimeError> {
    let actual = generation.slots.get(slot.slot.route as usize).copied();
    if slot.generation == generation.id && actual == Some(slot.slot) {
        Ok(())
    } else {
        Err(SemanticRuntimeError::StaleInputSlot)
    }
}

fn mark_consumers(
    generation: &RuntimeGeneration,
    slot: DenseSlot,
    dirty: &mut [bool],
    pending: &mut BinaryHeap<Reverse<u32>>,
) {
    for operation in generation.consumers(slot.route) {
        let index = *operation as usize;
        if !dirty[index] {
            dirty[index] = true;
            pending.push(Reverse(*operation));
        }
    }
}

fn execute_dense(
    generation: &RuntimeGeneration,
    arena: &mut DenseArena,
    effects: &mut Vec<CommittedEffect>,
) -> Result<usize, SemanticRuntimeError> {
    let mut executed = 0;
    for batch in &generation.batches {
        for index in batch.operations.clone() {
            execute_operation(&generation.operations[index as usize], arena, effects)?;
            executed += 1;
        }
    }
    Ok(executed)
}

fn execute_sparse(
    generation: &RuntimeGeneration,
    arena: &mut DenseArena,
    effects: &mut Vec<CommittedEffect>,
    dirty: &mut [bool],
    pending: &mut BinaryHeap<Reverse<u32>>,
) -> Result<usize, SemanticRuntimeError> {
    let mut executed = 0;
    while let Some(Reverse(index)) = pending.pop() {
        dirty[index as usize] = false;
        let output = execute_operation(&generation.operations[index as usize], arena, effects)?;
        if let Some(output) = output {
            mark_consumers(generation, output, dirty, pending);
        }
        executed += 1;
    }
    Ok(executed)
}

fn execute_operation(
    operation: &RuntimeOperation,
    arena: &mut DenseArena,
    effects: &mut Vec<CommittedEffect>,
) -> Result<Option<DenseSlot>, SemanticRuntimeError> {
    let output = match operation {
        RuntimeOperation::Copy { input, output } => {
            let value = arena.read(*input);
            arena.write(*output, value)?.then_some(*output)
        }
        RuntimeOperation::AddFloat { left, right, output } => {
            let value = float(arena, *left) + float(arena, *right);
            arena.write(*output, finite(value)?)?.then_some(*output)
        }
        RuntimeOperation::MultiplyFloat { left, right, output } => {
            let value = float(arena, *left) * float(arena, *right);
            arena.write(*output, finite(value)?)?.then_some(*output)
        }
        RuntimeOperation::ConditionGateFloat {
            condition,
            value,
            output,
        } => {
            let value = if bool_value(arena, *condition) {
                float(arena, *value)
            } else {
                0.0
            };
            arena.write(*output, ScalarValue::Float(value))?.then_some(*output)
        }
        RuntimeOperation::Emit { input, sink, order } => {
            effects.push(CommittedEffect {
                sink: sink.clone(),
                value: arena.read(*input),
                order: *order,
            });
            None
        }
    };
    Ok(output)
}

fn float(arena: &DenseArena, slot: DenseSlot) -> f64 {
    match arena.read(slot) {
        ScalarValue::Float(value) => value,
        _ => unreachable!("operation slots are compile-time checked"),
    }
}

fn bool_value(arena: &DenseArena, slot: DenseSlot) -> bool {
    match arena.read(slot) {
        ScalarValue::Bool(value) => value,
        _ => unreachable!("operation slots are compile-time checked"),
    }
}

fn finite(value: f64) -> Result<ScalarValue, SemanticRuntimeError> {
    value
        .is_finite()
        .then_some(ScalarValue::Float(value))
        .ok_or(SemanticRuntimeError::NonFiniteFloat)
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum SemanticRuntimeError {
    #[error("direct input slot belongs to a stale runtime generation")]
    StaleInputSlot,
    #[error("input expected {expected:?}, received {actual:?}")]
    InputTypeMismatch { expected: ScalarType, actual: ScalarType },
    #[error("runtime float values must be finite")]
    NonFiniteFloat,
    #[error("runtime state does not exist: {0:?}")]
    MissingState(StableStateKey),
}
