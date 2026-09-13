use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    sync::Arc,
    time::Duration,
};

use indexmap::{IndexMap, IndexSet};
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::{
    ANodeId, ColorValue, CompiledAlchemistGraph, CompiledExecNode, CompiledFormulaPropertySchema,
    CompiledNodeOperation, ExecNodeId, FormulaId, FormulaPropertyId, FormulaPropertySlotId, InputValueSource,
    RuntimeValue, SocketId, StableRef, TriggerValue, ValueSlotId, ValueTypeId, ValueTypeRegistry,
};

mod context;
mod operations;

pub use context::*;
use operations::{change_detection_inputs_into, evaluate_operation, runtime_node_inputs_into};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NodeFlow {
    #[default]
    Deliver,
    Suppress,
}

pub type NodeOutputs = SmallVec<[RuntimeValue; 4]>;

#[derive(Clone, Debug, PartialEq)]
pub struct AlchemistMemory {
    values: Vec<RuntimeValue>,
    value_initialized: Vec<bool>,
    value_flow: Vec<NodeFlow>,
    value_revisions: Vec<u64>,
    states: Vec<RuntimeValue>,
    node_inputs: Vec<Option<Vec<RuntimeValue>>>,
    runtime_inputs: Vec<RuntimeValue>,
    change_inputs: Vec<RuntimeValue>,
    node_initialized: Vec<bool>,
    node_flow: Vec<NodeFlow>,
    dirty_nodes: Vec<bool>,
    last_executed_nodes: Vec<ExecNodeId>,
}

impl AlchemistMemory {
    #[must_use]
    pub fn for_graph(compiled: &CompiledAlchemistGraph) -> Self {
        Self {
            values: vec![RuntimeValue::Unit; compiled.state_layout.value_slot_count],
            value_initialized: vec![false; compiled.state_layout.value_slot_count],
            value_flow: vec![NodeFlow::Deliver; compiled.state_layout.value_slot_count],
            value_revisions: vec![0; compiled.state_layout.value_slot_count],
            states: vec![RuntimeValue::Unit; compiled.state_layout.state_slot_count],
            node_inputs: vec![None; compiled.exec_nodes.len()],
            runtime_inputs: Vec::new(),
            change_inputs: Vec::new(),
            node_initialized: vec![false; compiled.exec_nodes.len()],
            node_flow: vec![NodeFlow::Deliver; compiled.exec_nodes.len()],
            dirty_nodes: vec![false; compiled.exec_nodes.len()],
            last_executed_nodes: Vec::new(),
        }
    }

    #[must_use]
    pub fn value(&self, slot: ValueSlotId) -> Option<&RuntimeValue> {
        self.value_initialized
            .get(slot.index())
            .copied()
            .filter(|initialized| *initialized && self.value_flow[slot.index()] == NodeFlow::Deliver)
            .and_then(|_| self.values.get(slot.index()))
    }

    #[must_use]
    pub fn node_flow(&self, node: ExecNodeId) -> NodeFlow {
        self.node_flow[node.index()]
    }

    #[must_use]
    pub fn slot_flow(&self, slot: ValueSlotId) -> NodeFlow {
        self.value_flow[slot.index()]
    }

    #[must_use]
    pub fn value_len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn state_len(&self) -> usize {
        self.states.len()
    }

    #[must_use]
    fn is_compatible_with_graph(&self, compiled: &CompiledAlchemistGraph) -> bool {
        self.values.len() == compiled.state_layout.value_slot_count
            && self.value_initialized.len() == compiled.state_layout.value_slot_count
            && self.value_flow.len() == compiled.state_layout.value_slot_count
            && self.value_revisions.len() == compiled.state_layout.value_slot_count
            && self.states.len() == compiled.state_layout.state_slot_count
            && self.node_inputs.len() == compiled.exec_nodes.len()
            && self.node_initialized.len() == compiled.exec_nodes.len()
            && self.node_flow.len() == compiled.exec_nodes.len()
            && self.dirty_nodes.len() == compiled.exec_nodes.len()
    }

    #[must_use]
    pub fn is_stateless(&self) -> bool {
        self.states.is_empty()
    }

    /// Restores reusable evaluation scratch to the semantics of newly allocated memory.
    ///
    /// Fresh multiplex lanes execute sequentially, so retaining these buffers avoids rebuilding
    /// seven heap-backed memory tables per lane without sharing values, node state, or change
    /// detection history between evaluations.
    pub fn reset_for_fresh_evaluation(&mut self, compiled: &CompiledAlchemistGraph) {
        if !self.is_compatible_with_graph(compiled) {
            *self = Self::for_graph(compiled);
            return;
        }
        self.values.fill(RuntimeValue::Unit);
        self.value_initialized.fill(false);
        self.value_flow.fill(NodeFlow::Deliver);
        self.value_revisions.fill(0);
        self.states.fill(RuntimeValue::Unit);
        for inputs in self.node_inputs.iter_mut().flatten() {
            inputs.clear();
        }
        self.runtime_inputs.clear();
        self.change_inputs.clear();
        self.node_initialized.fill(false);
        self.node_flow.fill(NodeFlow::Deliver);
        self.dirty_nodes.fill(false);
        self.last_executed_nodes.clear();
    }

    #[must_use]
    pub fn last_executed_nodes(&self) -> &[ExecNodeId] {
        &self.last_executed_nodes
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RuntimePropertyFrame {
    values: Box<[RuntimeValue]>,
}

impl RuntimePropertyFrame {
    #[must_use]
    pub fn from_defaults(schema: &CompiledFormulaPropertySchema) -> Self {
        let mut values = vec![RuntimeValue::Unit; schema.len()];
        for property in schema.properties.values() {
            values[property.slot.index()] = property.default_value.clone();
        }
        Self {
            values: values.into_boxed_slice(),
        }
    }

    pub fn with_overrides(
        schema: &CompiledFormulaPropertySchema,
        overrides: &indexmap::IndexMap<FormulaPropertyId, RuntimeValue>,
    ) -> Result<Self, RuntimePropertyFrameError> {
        let mut frame = Self::from_defaults(schema);
        for (id, value) in overrides {
            frame.set_override(schema, id, value.clone())?;
        }
        Ok(frame)
    }

    pub fn set_override(
        &mut self,
        schema: &CompiledFormulaPropertySchema,
        id: &FormulaPropertyId,
        value: RuntimeValue,
    ) -> Result<(), RuntimePropertyFrameError> {
        let property = schema
            .get(id)
            .ok_or_else(|| RuntimePropertyFrameError::UnknownProperty(id.clone()))?;
        let actual = value.value_type();
        if actual != property.value_type {
            return Err(RuntimePropertyFrameError::InvalidOverrideType {
                property: id.clone(),
                expected: property.value_type.clone(),
                actual,
            });
        }
        self.values[property.slot.index()] = value;
        Ok(())
    }

    #[must_use]
    pub fn get(&self, slot: FormulaPropertySlotId) -> Option<&RuntimeValue> {
        self.values.get(slot.index())
    }

    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.values.len()
    }
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuntimePropertyFrameError {
    #[error("processor override references unknown property `{0}`")]
    UnknownProperty(FormulaPropertyId),
    #[error("processor override for property `{property}` has type `{actual}`, expected `{expected}`")]
    InvalidOverrideType {
        property: FormulaPropertyId,
        expected: ValueTypeId,
        actual: ValueTypeId,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeContextFrame {
    context_key: ContextKey,
}

impl RuntimeContextFrame {
    #[must_use]
    pub fn default_lane() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn new(context_key: ContextKey) -> Self {
        Self { context_key }
    }

    #[must_use]
    pub fn context_key(&self) -> &ContextKey {
        &self.context_key
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum LaneRuntimePool {
    #[default]
    Stateless,
    Stateful(IndexMap<ContextKey, AlchemistMemory>),
}

impl LaneRuntimePool {
    #[must_use]
    pub fn for_graph(compiled: &CompiledAlchemistGraph) -> Self {
        if compiled.state_layout.state_slot_count == 0 && !compiled.analysis.has_input_gated_nodes {
            Self::Stateless
        } else {
            Self::Stateful(IndexMap::new())
        }
    }

    #[must_use]
    pub fn is_stateless(&self) -> bool {
        matches!(self, Self::Stateless)
    }

    #[must_use]
    pub fn is_compatible_with_graph(&self, compiled: &CompiledAlchemistGraph) -> bool {
        let needs_stateful_pool = compiled.state_layout.state_slot_count > 0 || compiled.analysis.has_input_gated_nodes;
        match self {
            Self::Stateless => !needs_stateful_pool,
            Self::Stateful(lanes) => {
                needs_stateful_pool && lanes.values().all(|memory| memory.is_compatible_with_graph(compiled))
            }
        }
    }

    #[must_use]
    pub fn memory_count(&self) -> usize {
        match self {
            Self::Stateless => 0,
            Self::Stateful(lanes) => lanes.len(),
        }
    }

    pub fn clear(&mut self) {
        if let Self::Stateful(lanes) = self {
            lanes.clear();
        }
    }

    pub fn retain_keys(&mut self, keys: &IndexSet<ContextKey>) {
        if let Self::Stateful(lanes) = self {
            lanes.retain(|key, _| keys.contains(key));
        }
    }

    pub fn retain_where(&mut self, mut keep: impl FnMut(&ContextKey) -> bool) {
        if let Self::Stateful(lanes) = self {
            lanes.retain(|key, _| keep(key));
        }
    }

    pub fn memory_for_key(
        &mut self,
        key: ContextKey,
        compiled: &CompiledAlchemistGraph,
    ) -> Option<&mut AlchemistMemory> {
        match self {
            Self::Stateless => None,
            Self::Stateful(lanes) => Some(lanes.entry(key).or_insert_with(|| AlchemistMemory::for_graph(compiled))),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DebugCaptureSink {
    mode: DebugCaptureMode,
    samples: Vec<DebugValueSample>,
}

impl Default for DebugCaptureSink {
    fn default() -> Self {
        Self::new(DebugCaptureMode::default())
    }
}

impl DebugCaptureSink {
    #[must_use]
    pub fn new(mode: DebugCaptureMode) -> Self {
        Self {
            mode,
            samples: Vec::new(),
        }
    }

    #[must_use]
    pub fn off() -> Self {
        Self::new(DebugCaptureMode::Off)
    }

    #[must_use]
    pub fn mode(&self) -> &DebugCaptureMode {
        &self.mode
    }

    pub fn capture(&mut self, mut sample: DebugValueSample) -> Option<DebugValueSample> {
        if self.mode.is_off() || !self.mode.accepts(&sample) {
            return None;
        }
        sample.status = self.mode.sample_status()?;
        sample.formula_id = self.mode.formula_id().cloned();
        self.samples.push(sample.clone());
        let history_len = self.mode.history_len();
        if history_len != usize::MAX && self.samples.len() > history_len {
            let overflow = self.samples.len() - history_len;
            self.samples.drain(0..overflow);
        }
        Some(sample)
    }

    #[must_use]
    pub fn samples(&self) -> &[DebugValueSample] {
        &self.samples
    }

    #[must_use]
    pub fn into_samples(self) -> Vec<DebugValueSample> {
        self.samples
    }
}

pub struct EvaluationFrame<'a, 'ctx> {
    pub ctx: &'a EvaluationCtx<'ctx>,
    pub properties: &'a RuntimePropertyFrame,
    pub context: &'a RuntimeContextFrame,
    pub debug: Option<&'a mut DebugCaptureSink>,
    pub force_process_unchanged_inputs: bool,
    pub capture_unchanged_outputs: bool,
}

pub struct NodeEvaluation<'a, 'ctx> {
    pub exec_node: ExecNodeId,
    pub author_node_id: ANodeId,
    pub ctx: &'a EvaluationCtx<'ctx>,
    pub inputs: &'a [RuntimeValue],
    pub input_sources: &'a [InputValueSource],
    pub properties: &'a RuntimePropertyFrame,
    pub context: &'a RuntimeContextFrame,
    pub debug: Option<&'a mut DebugCaptureSink>,
    pub state: &'a mut [RuntimeValue],
    pub intents: &'a mut Vec<RuntimeIntent>,
    pub output_flow: &'a mut [NodeFlow],
}

impl<'a, 'ctx> NodeEvaluation<'a, 'ctx> {
    pub fn suppress_output(&mut self, index: usize) {
        self.output_flow[index] = NodeFlow::Suppress;
    }

    #[must_use]
    pub fn input_has_connection(&self, index: usize) -> bool {
        fn connected(source: &InputValueSource) -> bool {
            match source {
                InputValueSource::Slot(_) | InputValueSource::RuntimeInput { .. } => true,
                InputValueSource::Converted { source, .. } | InputValueSource::Component { source, .. } => {
                    connected(source)
                }
                InputValueSource::Composite { base, components, .. } => {
                    connected(base) || components.iter().any(|(_, source)| connected(source))
                }
                InputValueSource::Constant(_) | InputValueSource::Unset => false,
            }
        }
        self.input_sources.get(index).is_some_and(connected)
    }

    pub fn capture_debug_value(&mut self, output_socket: impl Into<SocketId>, value: RuntimeValue) {
        let Some(debug) = self.debug.as_deref_mut() else {
            return;
        };
        let value_type = value.value_type();
        debug.capture(DebugValueSample {
            formula_id: None,
            context_key: (!self.context.context_key().is_default_lane()).then(|| self.context.context_key().clone()),
            author_node_id: self.author_node_id,
            exec_node: self.exec_node,
            output_socket: output_socket.into(),
            output_slot: ValueSlotId::new(u32::MAX),
            value_type,
            value,
            logical_tick: self.ctx.logical_tick,
            status: OutputPreviewStatus::Unavailable,
        });
    }
}

pub trait CompiledNodeEvaluator: Send + Sync + Debug {
    fn change_detection_inputs(
        &self,
        _ctx: &EvaluationCtx<'_>,
        _context: &RuntimeContextFrame,
    ) -> Result<Vec<RuntimeValue>, String> {
        Ok(Vec::new())
    }

    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<NodeOutputs, String>;
}

/// Supplies instance-owned behavior at authored graph nodes while retaining a shared graph plan.
pub trait ExternalNodeEvaluator {
    fn active_nodes(&self) -> &[ExecNodeId];

    fn evaluate(&mut self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<NodeOutputs, String>;
}

pub struct AlchemistRuntime {
    pub compiled: Arc<CompiledAlchemistGraph>,
    pub memory: AlchemistMemory,
    pub properties: RuntimePropertyFrame,
    execution_counts: Vec<u64>,
    evaluating: bool,
}

impl AlchemistRuntime {
    #[must_use]
    pub fn new(compiled: Arc<CompiledAlchemistGraph>) -> Self {
        let memory = AlchemistMemory::for_graph(&compiled);
        let properties = RuntimePropertyFrame::from_defaults(&compiled.properties);
        let execution_counts = vec![0; compiled.exec_nodes.len()];
        Self {
            compiled,
            memory,
            properties,
            execution_counts,
            evaluating: false,
        }
    }

    #[must_use]
    pub fn with_property_frame(compiled: Arc<CompiledAlchemistGraph>, properties: RuntimePropertyFrame) -> Self {
        let memory = AlchemistMemory::for_graph(&compiled);
        let execution_counts = vec![0; compiled.exec_nodes.len()];
        Self {
            compiled,
            memory,
            properties,
            execution_counts,
            evaluating: false,
        }
    }

    pub fn set_property_frame(&mut self, properties: RuntimePropertyFrame) {
        self.properties = properties;
    }

    pub fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        self.evaluate_with_capture_mode(ctx, DebugCaptureMode::default())
    }

    pub fn evaluate_with_capture_mode(
        &mut self,
        ctx: &EvaluationCtx<'_>,
        capture_mode: DebugCaptureMode,
    ) -> RuntimeOutput {
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
        let mut debug = DebugCaptureSink::new(capture_mode);
        let context = RuntimeContextFrame::default_lane();
        let output = evaluate_compiled_graph(
            &self.compiled,
            &mut self.memory,
            EvaluationFrame {
                ctx,
                properties: &self.properties,
                context: &context,
                debug: Some(&mut debug),
                force_process_unchanged_inputs: false,
                capture_unchanged_outputs: false,
            },
        );
        for exec_id in self.memory.last_executed_nodes() {
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

pub fn evaluate_compiled_graph(
    compiled: &CompiledAlchemistGraph,
    memory: &mut AlchemistMemory,
    frame: EvaluationFrame<'_, '_>,
) -> RuntimeOutput {
    evaluate_compiled_graph_inner(compiled, memory, frame, None)
}

pub fn evaluate_compiled_graph_with_external(
    compiled: &CompiledAlchemistGraph,
    memory: &mut AlchemistMemory,
    frame: EvaluationFrame<'_, '_>,
    external: &mut dyn ExternalNodeEvaluator,
) -> RuntimeOutput {
    evaluate_compiled_graph_inner(compiled, memory, frame, Some(external))
}

fn evaluate_compiled_graph_inner(
    compiled: &CompiledAlchemistGraph,
    memory: &mut AlchemistMemory,
    mut frame: EvaluationFrame<'_, '_>,
    mut external: Option<&mut dyn ExternalNodeEvaluator>,
) -> RuntimeOutput {
    let mut output = RuntimeOutput::default();
    seed_dirty_nodes(compiled, memory, &frame, &mut output);
    if let Some(external) = external.as_ref() {
        for exec_id in external.active_nodes() {
            *memory
                .dirty_nodes
                .get_mut(exec_id.index())
                .expect("external node belongs to compiled graph") = true;
        }
    }
    for exec_id in &compiled.topo_order {
        if !memory.dirty_nodes[exec_id.index()] {
            continue;
        }
        memory.dirty_nodes[exec_id.index()] = false;
        let node = &compiled.exec_nodes[exec_id.index()];
        let externally_evaluated = external
            .as_ref()
            .is_some_and(|external| external.active_nodes().contains(exec_id));
        if node
            .inputs
            .iter()
            .any(|input| input_is_suppressed(input, memory, frame.ctx.inputs))
        {
            memory.node_initialized[exec_id.index()] = true;
            memory.node_flow[exec_id.index()] = NodeFlow::Suppress;
            for slot in &node.outputs {
                if memory.value_flow[slot.index()] != NodeFlow::Suppress {
                    memory.value_flow[slot.index()] = NodeFlow::Suppress;
                    memory.value_revisions[slot.index()] = memory.value_revisions[slot.index()].saturating_add(1);
                    mark_slot_dependents_dirty(compiled, memory, *slot);
                }
            }
            continue;
        }
        if let Err(message) = runtime_node_inputs_into(node, memory, frame.ctx) {
            suppress_failed_node(compiled, memory, *exec_id, &node.outputs);
            output.diagnostics.push(RuntimeDiagnostic {
                exec_node: *exec_id,
                message,
            });
            continue;
        }
        if let Err(message) = change_detection_inputs_into(
            &mut memory.change_inputs,
            &node.operation,
            &memory.runtime_inputs,
            frame.properties,
            frame.ctx,
            frame.context,
        ) {
            suppress_failed_node(compiled, memory, *exec_id, &node.outputs);
            output.diagnostics.push(RuntimeDiagnostic {
                exec_node: *exec_id,
                message,
            });
            continue;
        }
        if node.process_on_input_change_only && !frame.force_process_unchanged_inputs && !externally_evaluated {
            let previous_inputs = memory.node_inputs.get(exec_id.index()).and_then(Option::as_ref);
            if memory.node_initialized[exec_id.index()]
                && memory.node_flow[exec_id.index()] == NodeFlow::Deliver
                && previous_inputs.is_some_and(|previous| runtime_values_equivalent(previous, &memory.change_inputs))
            {
                continue;
            }
        }
        if let Some(previous_inputs) = memory.node_inputs.get_mut(exec_id.index()) {
            if let Some(retained) = previous_inputs {
                retained.clear();
                retained.extend(memory.change_inputs.iter().cloned());
            } else {
                *previous_inputs = Some(memory.change_inputs.clone());
            }
        }
        memory.node_initialized[exec_id.index()] = true;
        memory.last_executed_nodes.push(*exec_id);
        let intent_count_before = output.intents.len();
        let mut output_flow = SmallVec::<[NodeFlow; 4]>::new();
        output_flow.resize(node.outputs.len(), NodeFlow::Deliver);
        let state = &mut memory.states[node.state_range.clone()];
        let mut evaluation = NodeEvaluation {
            exec_node: *exec_id,
            author_node_id: node.authored_id,
            ctx: frame.ctx,
            inputs: &memory.runtime_inputs,
            input_sources: &node.inputs,
            properties: frame.properties,
            context: frame.context,
            debug: frame.debug.as_deref_mut(),
            state,
            intents: &mut output.intents,
            output_flow: output_flow.as_mut_slice(),
        };
        let result = if externally_evaluated {
            external
                .as_deref_mut()
                .expect("external node was identified")
                .evaluate(&mut evaluation)
        } else {
            evaluate_operation(&node.operation, evaluation)
        };
        match result {
            Ok(values) if values.len() == node.outputs.len() => {
                memory.node_flow[exec_id.index()] = NodeFlow::Deliver;
                let logged_output_values = node.log_enabled.then(|| values.clone());
                let capture_debug_outputs = frame.debug.as_ref().is_some_and(|debug| !debug.mode().is_off());
                for (output_index, (slot, value)) in node.outputs.iter().zip(values).enumerate() {
                    let flow = output_flow[output_index];
                    let previous_value = memory.values.get(slot.index());
                    let output_changed = !memory.value_initialized[slot.index()]
                        || memory.value_flow[slot.index()] != flow
                        || previous_value.is_none_or(|previous| !runtime_value_equivalent(previous, &value));
                    let captured_value = (flow == NodeFlow::Deliver
                        && capture_debug_outputs
                        && (!node.send_on_output_change_only || output_changed || frame.capture_unchanged_outputs))
                        .then(|| value.clone());
                    memory.values[slot.index()] = value;
                    memory.value_initialized[slot.index()] = true;
                    memory.value_flow[slot.index()] = flow;
                    if output_changed {
                        memory.value_revisions[slot.index()] = memory.value_revisions[slot.index()].saturating_add(1);
                        mark_slot_dependents_dirty(compiled, memory, *slot);
                    }
                    let Some(value) = captured_value else {
                        continue;
                    };
                    let output_socket = node
                        .output_sockets
                        .get(output_index)
                        .cloned()
                        .unwrap_or_else(|| SocketId::new(format!("slot_{}", slot.index())));
                    let value_type = node
                        .output_types
                        .get(output_index)
                        .and_then(Clone::clone)
                        .unwrap_or_else(|| value.value_type());
                    let sample = DebugValueSample {
                        formula_id: None,
                        context_key: (!frame.context.context_key().is_default_lane())
                            .then(|| frame.context.context_key().clone()),
                        author_node_id: node.authored_id,
                        exec_node: *exec_id,
                        output_socket,
                        output_slot: *slot,
                        value_type,
                        value,
                        logical_tick: frame.ctx.logical_tick,
                        status: OutputPreviewStatus::Unavailable,
                    };
                    if let Some(debug) = frame.debug.as_deref_mut() {
                        debug.capture(sample);
                    }
                }
                if let Some(output_values) = logged_output_values {
                    for (output_index, value) in output_values.into_iter().enumerate() {
                        if output_flow[output_index] == NodeFlow::Suppress {
                            continue;
                        }
                        output.intents.push(RuntimeIntent {
                            kind: Arc::from("debug.log"),
                            source_node: Some(node.authored_id),
                            source_socket: node.output_sockets.get(output_index).cloned(),
                            target: None,
                            payload: value,
                            logical_tick: frame.ctx.logical_tick,
                        });
                    }
                }
            }
            Ok(values) => {
                output.intents.truncate(intent_count_before);
                suppress_failed_node(compiled, memory, *exec_id, &node.outputs);
                output.diagnostics.push(RuntimeDiagnostic {
                    exec_node: *exec_id,
                    message: format!(
                        "node produced {} output(s), expected {}",
                        values.len(),
                        node.outputs.len()
                    ),
                });
            }
            Err(message) => {
                output.intents.truncate(intent_count_before);
                suppress_failed_node(compiled, memory, *exec_id, &node.outputs);
                output.diagnostics.push(RuntimeDiagnostic {
                    exec_node: *exec_id,
                    message,
                });
            }
        }
    }
    if frame.capture_unchanged_outputs
        && let Some(debug) = frame.debug.as_deref_mut().filter(|debug| !debug.mode().is_off())
    {
        capture_initialized_outputs(compiled, memory, frame.context, frame.ctx.logical_tick, debug);
    }
    output.debug_samples = frame
        .debug
        .as_ref()
        .map(|debug| debug.samples().to_vec())
        .unwrap_or_default();
    output
}

fn suppress_failed_node(
    compiled: &CompiledAlchemistGraph,
    memory: &mut AlchemistMemory,
    exec_id: ExecNodeId,
    outputs: &[ValueSlotId],
) {
    memory.node_initialized[exec_id.index()] = true;
    memory.node_flow[exec_id.index()] = NodeFlow::Suppress;
    for slot in outputs {
        if memory.value_flow[slot.index()] != NodeFlow::Suppress {
            memory.value_flow[slot.index()] = NodeFlow::Suppress;
            memory.value_revisions[slot.index()] = memory.value_revisions[slot.index()].saturating_add(1);
            mark_slot_dependents_dirty(compiled, memory, *slot);
        }
    }
}

fn capture_initialized_outputs(
    compiled: &CompiledAlchemistGraph,
    memory: &AlchemistMemory,
    context: &RuntimeContextFrame,
    logical_tick: u64,
    debug: &mut DebugCaptureSink,
) {
    let capture_mode = debug.mode().clone();
    let synthetic_samples = debug
        .samples()
        .iter()
        .filter(|sample| sample.output_slot.index() >= memory.values.len())
        .cloned()
        .collect::<Vec<_>>();
    let mut current_output_samples = debug
        .samples()
        .iter()
        .filter(|sample| sample.output_slot.index() < memory.values.len())
        .cloned()
        .map(|sample| ((sample.exec_node, sample.output_slot), sample))
        .collect::<HashMap<_, _>>();
    let mut current = DebugCaptureSink::new(capture_mode);
    for sample in synthetic_samples {
        current.capture(sample);
    }
    for exec_id in &compiled.topo_order {
        let node = &compiled.exec_nodes[exec_id.index()];
        for (output_index, slot) in node.outputs.iter().enumerate() {
            if !memory.value_initialized[slot.index()] || memory.value_flow[slot.index()] == NodeFlow::Suppress {
                continue;
            }
            if let Some(sample) = current_output_samples.remove(&(*exec_id, *slot)) {
                current.capture(sample);
                continue;
            }
            let value = match &memory.values[slot.index()] {
                RuntimeValue::Trigger(trigger) if trigger.fired => RuntimeValue::Trigger(TriggerValue {
                    fired: false,
                    ..*trigger
                }),
                value => value.clone(),
            };
            let output_socket = node
                .output_sockets
                .get(output_index)
                .cloned()
                .unwrap_or_else(|| SocketId::new(format!("slot_{}", slot.index())));
            let value_type = node
                .output_types
                .get(output_index)
                .and_then(Clone::clone)
                .unwrap_or_else(|| value.value_type());
            current.capture(DebugValueSample {
                formula_id: None,
                context_key: (!context.context_key().is_default_lane()).then(|| context.context_key().clone()),
                author_node_id: node.authored_id,
                exec_node: *exec_id,
                output_socket,
                output_slot: *slot,
                value_type,
                value,
                logical_tick,
                status: OutputPreviewStatus::Unavailable,
            });
        }
    }
    *debug = current;
}

fn seed_dirty_nodes(
    compiled: &CompiledAlchemistGraph,
    memory: &mut AlchemistMemory,
    frame: &EvaluationFrame<'_, '_>,
    output: &mut RuntimeOutput,
) {
    memory.dirty_nodes.fill(false);
    memory.last_executed_nodes.clear();

    if frame.force_process_unchanged_inputs {
        for exec_id in &compiled.topo_order {
            memory.dirty_nodes[exec_id.index()] = true;
        }
        return;
    }

    for exec_id in &compiled.topo_order {
        if !memory.node_initialized[exec_id.index()] {
            memory.dirty_nodes[exec_id.index()] = true;
        }
    }

    for exec_id in &compiled.dependencies.always_process_nodes {
        memory.dirty_nodes[exec_id.index()] = true;
    }

    for exec_id in &compiled.dependencies.external_input_nodes {
        match node_change_inputs_changed(compiled, memory, *exec_id, frame) {
            Ok(true) => {
                memory.dirty_nodes[exec_id.index()] = true;
            }
            Ok(false) => {}
            Err(message) => {
                output.diagnostics.push(RuntimeDiagnostic {
                    exec_node: *exec_id,
                    message,
                });
            }
        }
    }
}

fn node_change_inputs_changed(
    compiled: &CompiledAlchemistGraph,
    memory: &mut AlchemistMemory,
    exec_id: ExecNodeId,
    frame: &EvaluationFrame<'_, '_>,
) -> Result<bool, String> {
    if !memory.node_initialized[exec_id.index()] {
        return Ok(true);
    }
    let node = &compiled.exec_nodes[exec_id.index()];
    runtime_node_inputs_into(node, memory, frame.ctx)?;
    change_detection_inputs_into(
        &mut memory.change_inputs,
        &node.operation,
        &memory.runtime_inputs,
        frame.properties,
        frame.ctx,
        frame.context,
    )?;
    let previous_inputs = memory.node_inputs.get(exec_id.index()).and_then(Option::as_ref);
    Ok(!previous_inputs.is_some_and(|previous| runtime_values_equivalent(previous, &memory.change_inputs)))
}

fn mark_slot_dependents_dirty(compiled: &CompiledAlchemistGraph, memory: &mut AlchemistMemory, slot: ValueSlotId) {
    if let Some(dependents) = compiled.dependencies.slot_dependents.get(slot.index()) {
        for dependent in dependents {
            memory.dirty_nodes[dependent.index()] = true;
        }
    }
}

fn input_is_suppressed(source: &InputValueSource, memory: &AlchemistMemory, inputs: &RuntimeInputSnapshot) -> bool {
    match source {
        InputValueSource::Slot(slot) => memory.value_flow[slot.index()] == NodeFlow::Suppress,
        InputValueSource::Converted { source, .. } | InputValueSource::Component { source, .. } => {
            input_is_suppressed(source, memory, inputs)
        }
        InputValueSource::RuntimeInput { reference, fallback } => {
            inputs.get(reference).is_none() && input_is_suppressed(fallback, memory, inputs)
        }
        InputValueSource::Composite { base, components, .. } => {
            input_is_suppressed(base, memory, inputs)
                || components
                    .iter()
                    .any(|(_, source)| input_is_suppressed(source, memory, inputs))
        }
        InputValueSource::Constant(_) | InputValueSource::Unset => false,
    }
}

fn runtime_values_equivalent(left: &[RuntimeValue], right: &[RuntimeValue]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| runtime_value_equivalent(left, right))
}

fn runtime_value_equivalent(left: &RuntimeValue, right: &RuntimeValue) -> bool {
    match (left, right) {
        (RuntimeValue::Trigger(left), RuntimeValue::Trigger(right)) if !left.fired && !right.fired => true,
        _ => left == right,
    }
}

pub fn evaluate_compiled_graph_stateless(
    compiled: &CompiledAlchemistGraph,
    frame: EvaluationFrame<'_, '_>,
) -> RuntimeOutput {
    let mut memory = AlchemistMemory::for_graph(compiled);
    evaluate_compiled_graph(compiled, &mut memory, frame)
}

pub fn evaluate_compiled_graph_fresh_reusing(
    compiled: &CompiledAlchemistGraph,
    memory: &mut AlchemistMemory,
    frame: EvaluationFrame<'_, '_>,
) -> RuntimeOutput {
    memory.reset_for_fresh_evaluation(compiled);
    evaluate_compiled_graph(compiled, memory, frame)
}
