use std::{collections::HashMap, fmt::Debug, sync::Arc, time::Duration};

use crate::{
    CompiledAlchemistGraph, CompiledNodeOperation, ExecNodeId, InputValueSource, RuntimeValue, StableRef, TriggerValue,
    ValueSlotId, ValueTypeRegistry,
};

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeEvent {
    pub topic: Arc<str>,
    pub value: RuntimeValue,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeInputSnapshot {
    values: HashMap<StableRef, RuntimeValue>,
}

impl RuntimeInputSnapshot {
    pub fn insert(&mut self, reference: StableRef, value: RuntimeValue) -> Option<RuntimeValue> {
        self.values.insert(reference, value)
    }

    #[must_use]
    pub fn get(&self, reference: &StableRef) -> Option<&RuntimeValue> {
        self.values.get(reference)
    }
}

pub struct RuntimeRegistries<'a> {
    pub value_types: &'a ValueTypeRegistry,
}

pub struct EvaluationCtx<'a> {
    pub logical_tick: u64,
    pub delta_time: Duration,
    pub events: &'a [RuntimeEvent],
    pub inputs: &'a RuntimeInputSnapshot,
    pub registries: &'a RuntimeRegistries<'a>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeIntent {
    pub kind: Arc<str>,
    pub target: Option<StableRef>,
    pub payload: RuntimeValue,
    pub logical_tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeDiagnostic {
    pub exec_node: ExecNodeId,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DebugValueSample {
    pub exec_node: ExecNodeId,
    pub output_slot: ValueSlotId,
    pub value: RuntimeValue,
    pub logical_tick: u64,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeOutput {
    pub intents: Vec<RuntimeIntent>,
    pub diagnostics: Vec<RuntimeDiagnostic>,
    pub debug_samples: Vec<DebugValueSample>,
}

#[derive(Clone, Debug)]
pub struct AlchemistMemory {
    values: Vec<RuntimeValue>,
    states: Vec<RuntimeValue>,
}

impl AlchemistMemory {
    #[must_use]
    pub fn for_graph(compiled: &CompiledAlchemistGraph) -> Self {
        Self {
            values: vec![RuntimeValue::Unit; compiled.state_layout.value_slot_count],
            states: vec![RuntimeValue::Unit; compiled.state_layout.state_slot_count],
        }
    }

    #[must_use]
    pub fn value(&self, slot: ValueSlotId) -> Option<&RuntimeValue> {
        self.values.get(slot.index())
    }
}

pub struct NodeEvaluation<'a, 'ctx> {
    pub exec_node: ExecNodeId,
    pub ctx: &'a EvaluationCtx<'ctx>,
    pub inputs: &'a [RuntimeValue],
    pub state: &'a mut [RuntimeValue],
    pub intents: &'a mut Vec<RuntimeIntent>,
}

pub trait CompiledNodeEvaluator: Send + Sync + Debug {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String>;
}

pub struct AlchemistRuntime {
    pub compiled: Arc<CompiledAlchemistGraph>,
    pub memory: AlchemistMemory,
    execution_counts: Vec<u64>,
    evaluating: bool,
}

impl AlchemistRuntime {
    #[must_use]
    pub fn new(compiled: Arc<CompiledAlchemistGraph>) -> Self {
        let memory = AlchemistMemory::for_graph(&compiled);
        let execution_counts = vec![0; compiled.exec_nodes.len()];
        Self {
            compiled,
            memory,
            execution_counts,
            evaluating: false,
        }
    }

    pub fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        if self.evaluating {
            return RuntimeOutput {
                diagnostics: vec![RuntimeDiagnostic {
                    exec_node: ExecNodeId::new(0),
                    message: "Alchemist runtime evaluation is non-reentrant".into(),
                }],
                ..RuntimeOutput::default()
            };
        }
        self.evaluating = true;
        let mut output = RuntimeOutput::default();
        for exec_id in &self.compiled.topo_order {
            let node = &self.compiled.exec_nodes[exec_id.index()];
            let inputs: Vec<RuntimeValue> = node
                .inputs
                .iter()
                .map(|source| match source {
                    InputValueSource::Slot(slot) => self.memory.values[slot.index()].clone(),
                    InputValueSource::Constant(value) => value.clone(),
                    InputValueSource::Unset => RuntimeValue::Unit,
                })
                .collect();
            let state = &mut self.memory.states[node.state_range.clone()];
            let result = evaluate_operation(
                &node.operation,
                NodeEvaluation {
                    exec_node: *exec_id,
                    ctx,
                    inputs: &inputs,
                    state,
                    intents: &mut output.intents,
                },
            );
            match result {
                Ok(values) if values.len() == node.outputs.len() => {
                    for (slot, value) in node.outputs.iter().zip(values) {
                        self.memory.values[slot.index()] = value.clone();
                        output.debug_samples.push(DebugValueSample {
                            exec_node: *exec_id,
                            output_slot: *slot,
                            value,
                            logical_tick: ctx.logical_tick,
                        });
                    }
                }
                Ok(values) => output.diagnostics.push(RuntimeDiagnostic {
                    exec_node: *exec_id,
                    message: format!(
                        "node produced {} output(s), expected {}",
                        values.len(),
                        node.outputs.len()
                    ),
                }),
                Err(message) => output.diagnostics.push(RuntimeDiagnostic {
                    exec_node: *exec_id,
                    message,
                }),
            }
            self.execution_counts[exec_id.index()] += 1;
        }
        self.evaluating = false;
        output
    }

    #[must_use]
    pub fn execution_count(&self, exec_node: ExecNodeId) -> u64 {
        self.execution_counts[exec_node.index()]
    }
}

fn evaluate_operation(
    operation: &CompiledNodeOperation,
    mut evaluation: NodeEvaluation<'_, '_>,
) -> Result<Vec<RuntimeValue>, String> {
    match operation {
        CompiledNodeOperation::Constant(value) => Ok(vec![value.clone()]),
        CompiledNodeOperation::Add => {
            let [left, right] = require_inputs::<2>(evaluation.inputs)?;
            Ok(vec![add_values(left, right)?])
        }
        CompiledNodeOperation::Compare => {
            let [left, right] = require_inputs::<2>(evaluation.inputs)?;
            Ok(vec![RuntimeValue::Bool(left == right)])
        }
        CompiledNodeOperation::BoolAnd => {
            let [left, right] = bool_inputs::<2>(evaluation.inputs)?;
            Ok(vec![RuntimeValue::Bool(left && right)])
        }
        CompiledNodeOperation::BoolOr => {
            let [left, right] = bool_inputs::<2>(evaluation.inputs)?;
            Ok(vec![RuntimeValue::Bool(left || right)])
        }
        CompiledNodeOperation::BoolNot => {
            let [value] = bool_inputs::<1>(evaluation.inputs)?;
            Ok(vec![RuntimeValue::Bool(!value)])
        }
        CompiledNodeOperation::Edge => {
            let [value] = bool_inputs::<1>(evaluation.inputs)?;
            let previous = matches!(evaluation.state.first(), Some(RuntimeValue::Bool(true)));
            if let Some(state) = evaluation.state.first_mut() {
                *state = RuntimeValue::Bool(value);
            }
            Ok(vec![RuntimeValue::Trigger(TriggerValue {
                fired: value && !previous,
                edge_id: u64::from(evaluation.exec_node.index() as u32),
                logical_tick: evaluation.ctx.logical_tick,
            })])
        }
        CompiledNodeOperation::Gate => {
            let [trigger, open] = require_inputs::<2>(evaluation.inputs)?;
            let RuntimeValue::Trigger(trigger) = trigger else {
                return Err("Gate expects a trigger input".into());
            };
            let RuntimeValue::Bool(open) = open else {
                return Err("Gate expects a boolean open input".into());
            };
            Ok(vec![RuntimeValue::Trigger(TriggerValue {
                fired: trigger.fired && *open,
                ..*trigger
            })])
        }
        CompiledNodeOperation::MapRange => {
            let values = float_inputs::<5>(evaluation.inputs)?;
            let [value, in_min, in_max, out_min, out_max] = values;
            if (in_max - in_min).abs() <= f64::EPSILON {
                return Err("Map Range input range cannot be zero".into());
            }
            let normalized = (value - in_min) / (in_max - in_min);
            Ok(vec![RuntimeValue::Float(out_min + normalized * (out_max - out_min))])
        }
        CompiledNodeOperation::Clamp => {
            let [value, minimum, maximum] = float_inputs::<3>(evaluation.inputs)?;
            Ok(vec![RuntimeValue::Float(value.clamp(minimum, maximum))])
        }
        CompiledNodeOperation::DelayOneTick => {
            let [value] = require_inputs::<1>(evaluation.inputs)?;
            if let Some(state) = evaluation.state.first_mut() {
                *state = value.clone();
            }
            Ok(vec![value.clone()])
        }
        CompiledNodeOperation::DebugLog => {
            let [value] = require_inputs::<1>(evaluation.inputs)?;
            evaluation.intents.push(RuntimeIntent {
                kind: Arc::from("debug.log"),
                target: None,
                payload: value.clone(),
                logical_tick: evaluation.ctx.logical_tick,
            });
            Ok(Vec::new())
        }
        CompiledNodeOperation::Custom(evaluator) => evaluator.evaluate(&mut evaluation),
    }
}

fn require_inputs<const N: usize>(inputs: &[RuntimeValue]) -> Result<[&RuntimeValue; N], String> {
    inputs
        .iter()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| format!("node expects {N} input(s)"))
}

fn bool_inputs<const N: usize>(inputs: &[RuntimeValue]) -> Result<[bool; N], String> {
    require_inputs::<N>(inputs)?
        .map(|value| match value {
            RuntimeValue::Bool(value) => Ok(*value),
            _ => Err("node expects boolean inputs".into()),
        })
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?
        .try_into()
        .map_err(|_| "invalid boolean input count".into())
}

fn float_inputs<const N: usize>(inputs: &[RuntimeValue]) -> Result<[f64; N], String> {
    require_inputs::<N>(inputs)?
        .map(|value| match value {
            RuntimeValue::Float(value) => Ok(*value),
            RuntimeValue::Int(value) => Ok(*value as f64),
            _ => Err("node expects numeric scalar inputs".into()),
        })
        .into_iter()
        .collect::<Result<Vec<_>, String>>()?
        .try_into()
        .map_err(|_| "invalid numeric input count".into())
}

fn add_values(left: &RuntimeValue, right: &RuntimeValue) -> Result<RuntimeValue, String> {
    match (left, right) {
        (RuntimeValue::Int(left), RuntimeValue::Int(right)) => Ok(RuntimeValue::Int(left + right)),
        (RuntimeValue::Float(left), RuntimeValue::Float(right)) => Ok(RuntimeValue::Float(left + right)),
        (RuntimeValue::Int(left), RuntimeValue::Float(right)) => Ok(RuntimeValue::Float(*left as f64 + right)),
        (RuntimeValue::Float(left), RuntimeValue::Int(right)) => Ok(RuntimeValue::Float(left + *right as f64)),
        (RuntimeValue::Vec2(left), RuntimeValue::Vec2(right)) => {
            Ok(RuntimeValue::Vec2([left[0] + right[0], left[1] + right[1]]))
        }
        (RuntimeValue::Vec3(left), RuntimeValue::Vec3(right)) => Ok(RuntimeValue::Vec3([
            left[0] + right[0],
            left[1] + right[1],
            left[2] + right[2],
        ])),
        _ => Err("Add received incompatible runtime values".into()),
    }
}
