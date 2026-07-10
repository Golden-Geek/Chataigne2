use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use golden_graph::{GraphNodeId, PortRef, stable_topological_order};
use golden_model::Revision;
use golden_values::{FiniteF64, Value};
use thiserror::Error;

use crate::{ANodeRegistry, AlchemistFormula, BuiltinOperation, FormulaId, SurfaceItemId};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExecNodeId(pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ValueSlot(pub u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CompileKey {
    pub formula: FormulaId,
    pub graph_revision: Revision,
    pub registry_revision: Revision,
}

#[derive(Clone, Debug)]
pub enum CompiledOp {
    Constant {
        node: ExecNodeId,
        value: Value,
        output: ValueSlot,
    },
    AddFloat {
        node: ExecNodeId,
        left: ValueSlot,
        right: ValueSlot,
        output: ValueSlot,
    },
    MultiplyFloat {
        node: ExecNodeId,
        left: ValueSlot,
        right: ValueSlot,
        output: ValueSlot,
    },
    PassThrough {
        node: ExecNodeId,
        input: ValueSlot,
        output: ValueSlot,
    },
    ConditionGate {
        node: ExecNodeId,
        condition: ValueSlot,
        value: ValueSlot,
        output: ValueSlot,
    },
}

impl CompiledOp {
    pub const fn output(&self) -> ValueSlot {
        match self {
            Self::Constant { output, .. }
            | Self::AddFloat { output, .. }
            | Self::MultiplyFloat { output, .. }
            | Self::PassThrough { output, .. }
            | Self::ConditionGate { output, .. } => *output,
        }
    }

    pub const fn node(&self) -> ExecNodeId {
        match self {
            Self::Constant { node, .. }
            | Self::AddFloat { node, .. }
            | Self::MultiplyFloat { node, .. }
            | Self::PassThrough { node, .. }
            | Self::ConditionGate { node, .. } => *node,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StateLayout {
    pub slots: u32,
}

#[derive(Clone, Debug)]
pub struct CompiledFormulaKernel {
    pub key: CompileKey,
    pub operations: Vec<CompiledOp>,
    pub slot_defaults: Vec<Value>,
    pub surface_inputs: BTreeMap<SurfaceItemId, ValueSlot>,
    pub surface_outputs: BTreeMap<SurfaceItemId, ValueSlot>,
    pub output_slots: BTreeSet<ValueSlot>,
    pub state_layout: StateLayout,
    pub time_dependent: bool,
}

pub struct FormulaCompiler<'a> {
    registry: &'a ANodeRegistry,
}

impl<'a> FormulaCompiler<'a> {
    pub const fn new(registry: &'a ANodeRegistry) -> Self {
        Self { registry }
    }

    pub fn compile(&self, formula: &AlchemistFormula) -> Result<CompiledFormulaKernel, CompileError> {
        let order = stable_topological_order(&formula.graph).map_err(|cycle| CompileError::Cycle(cycle.remaining))?;
        let key = CompileKey {
            formula: formula.id,
            graph_revision: formula.graph.revision(),
            registry_revision: self.registry.revision(),
        };
        let mut next_slot = 0_u32;
        let mut slots = BTreeMap::<PortRef, ValueSlot>::new();
        let mut slot_defaults = Vec::new();

        for node in formula.graph.nodes() {
            for port in &node.data.outputs {
                slots.insert(
                    PortRef {
                        node: node.id,
                        port: port.id,
                    },
                    allocate_slot(&mut next_slot, &mut slot_defaults, &port.value_type)?,
                );
            }
        }
        for node in formula.graph.nodes() {
            for port in &node.data.inputs {
                let reference = PortRef {
                    node: node.id,
                    port: port.id,
                };
                let source = formula
                    .graph
                    .incoming_edges(node.id)
                    .find(|edge| edge.to == reference)
                    .map(|edge| edge.from);
                let slot = match source {
                    Some(source) => *slots.get(&source).ok_or(CompileError::MissingPort(source))?,
                    None => allocate_slot(&mut next_slot, &mut slot_defaults, &port.value_type)?,
                };
                slots.insert(reference, slot);
            }
        }

        let surface_inputs = formula
            .surface
            .inputs
            .iter()
            .map(|input| {
                let slot = *slots
                    .get(&input.target)
                    .ok_or(CompileError::MissingPort(input.target))?;
                if input.default.type_id() != input.value_type.as_str() {
                    return Err(CompileError::SurfaceTypeMismatch(input.id));
                }
                slot_defaults[slot.0 as usize] = input.default.clone();
                Ok((input.id, slot))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let surface_outputs = formula
            .surface
            .outputs
            .iter()
            .map(|output| {
                slots
                    .get(&output.source)
                    .copied()
                    .map(|slot| (output.id, slot))
                    .ok_or(CompileError::MissingPort(output.source))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;

        let mut operations = Vec::with_capacity(order.len());
        let mut state_slots = 0_u32;
        let mut time_dependent = false;
        for (index, node_id) in order.into_iter().enumerate() {
            let node = formula.graph.node(node_id).ok_or(CompileError::MissingNode(node_id))?;
            let definition = self
                .registry
                .get(&node.data.node_type)
                .ok_or_else(|| CompileError::UnknownNodeType(node.data.node_type.0.clone()))?;
            state_slots = state_slots
                .checked_add(u32::from(definition.capabilities.state_slots))
                .ok_or(CompileError::StateLayoutOverflow)?;
            time_dependent |= definition.capabilities.time_dependent;
            let exec = ExecNodeId(u32::try_from(index).map_err(|_| CompileError::TooManyNodes)?);
            operations.push(compile_operation(
                exec,
                node_id,
                &node.data,
                definition.operation,
                &slots,
            )?);
        }
        let output_slots = surface_outputs.values().copied().collect();
        Ok(CompiledFormulaKernel {
            key,
            operations,
            slot_defaults,
            surface_inputs,
            surface_outputs,
            output_slots,
            state_layout: StateLayout { slots: state_slots },
            time_dependent,
        })
    }
}

fn compile_operation(
    node: ExecNodeId,
    node_id: GraphNodeId,
    authored: &crate::AlchemistNode,
    operation: BuiltinOperation,
    slots: &BTreeMap<PortRef, ValueSlot>,
) -> Result<CompiledOp, CompileError> {
    let input = |index: usize| {
        authored
            .inputs
            .get(index)
            .and_then(|port| {
                slots.get(&PortRef {
                    node: node_id,
                    port: port.id,
                })
            })
            .copied()
            .ok_or(CompileError::InvalidArity(node_id))
    };
    let output = |index: usize| {
        authored
            .outputs
            .get(index)
            .and_then(|port| {
                slots.get(&PortRef {
                    node: node_id,
                    port: port.id,
                })
            })
            .copied()
            .ok_or(CompileError::InvalidArity(node_id))
    };
    match operation {
        BuiltinOperation::Constant => Ok(CompiledOp::Constant {
            node,
            value: authored
                .config
                .get("value")
                .cloned()
                .ok_or(CompileError::MissingConstant(node_id))?,
            output: output(0)?,
        }),
        BuiltinOperation::AddFloat => Ok(CompiledOp::AddFloat {
            node,
            left: input(0)?,
            right: input(1)?,
            output: output(0)?,
        }),
        BuiltinOperation::MultiplyFloat => Ok(CompiledOp::MultiplyFloat {
            node,
            left: input(0)?,
            right: input(1)?,
            output: output(0)?,
        }),
        BuiltinOperation::PassThrough => Ok(CompiledOp::PassThrough {
            node,
            input: input(0)?,
            output: output(0)?,
        }),
        BuiltinOperation::ConditionGate => Ok(CompiledOp::ConditionGate {
            node,
            condition: input(0)?,
            value: input(1)?,
            output: output(0)?,
        }),
    }
}

fn allocate_slot(
    next: &mut u32,
    defaults: &mut Vec<Value>,
    value_type: &golden_values::ValueTypeId,
) -> Result<ValueSlot, CompileError> {
    let slot = ValueSlot(*next);
    *next = next.checked_add(1).ok_or(CompileError::TooManySlots)?;
    defaults.push(default_value(value_type)?);
    Ok(slot)
}

fn default_value(value_type: &golden_values::ValueTypeId) -> Result<Value, CompileError> {
    match value_type.as_str() {
        "bool" => Ok(Value::Bool(false)),
        "integer" => Ok(Value::Integer(0)),
        "float" => Ok(Value::Float(FiniteF64::new(0.0).expect("zero is finite"))),
        "string" => Ok(Value::String(String::new())),
        other => Err(CompileError::UnsupportedValueType(other.to_owned())),
    }
}

#[derive(Default)]
pub struct FormulaCompileCache {
    kernels: Mutex<BTreeMap<CompileKey, Arc<CompiledFormulaKernel>>>,
    compilations: AtomicUsize,
}

impl FormulaCompileCache {
    pub fn compile(
        &self,
        formula: &AlchemistFormula,
        registry: &ANodeRegistry,
    ) -> Result<Arc<CompiledFormulaKernel>, CompileError> {
        let key = CompileKey {
            formula: formula.id,
            graph_revision: formula.graph.revision(),
            registry_revision: registry.revision(),
        };
        let mut kernels = self.kernels.lock().expect("compile cache lock poisoned");
        if let Some(kernel) = kernels.get(&key) {
            return Ok(Arc::clone(kernel));
        }
        let kernel = Arc::new(FormulaCompiler::new(registry).compile(formula)?);
        kernels.insert(key, Arc::clone(&kernel));
        self.compilations.fetch_add(1, Ordering::Relaxed);
        Ok(kernel)
    }

    pub fn compilation_count(&self) -> usize {
        self.compilations.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CompileError {
    #[error("formula graph contains a cycle")]
    Cycle(Vec<GraphNodeId>),
    #[error("formula references missing node {0:?}")]
    MissingNode(GraphNodeId),
    #[error("formula references missing port {0:?}")]
    MissingPort(PortRef),
    #[error("unknown ANode type {0}")]
    UnknownNodeType(smol_str::SmolStr),
    #[error("ANode has invalid input/output arity: {0:?}")]
    InvalidArity(GraphNodeId),
    #[error("constant ANode has no value: {0:?}")]
    MissingConstant(GraphNodeId),
    #[error("surface type does not match its default or port: {0:?}")]
    SurfaceTypeMismatch(SurfaceItemId),
    #[error("value type is not supported by the Phase 3 evaluator: {0}")]
    UnsupportedValueType(String),
    #[error("formula has too many executable nodes")]
    TooManyNodes,
    #[error("formula has too many value slots")]
    TooManySlots,
    #[error("formula state layout overflowed")]
    StateLayoutOverflow,
}
