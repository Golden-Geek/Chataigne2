//! Executes managed value regions at declared Formula graph nodes.

use std::sync::Arc;

use chataigne_alchemist::{
    ANodeId, AlchemistFormula, AlchemistMemory, ChannelLayout, CompileCtx, CompiledAlchemistGraph, CompiledExecNode,
    ContextKey, DebugCaptureMode, DebugCaptureSink, EvaluationCtx, EvaluationFrame, ExternalNodeEvaluator,
    InputValueSource, LaneRuntimePool, ManagedRegionDefinition, ManagedSocketRef, NodeEvaluation, RuntimeContextFrame,
    RuntimeOutput, RuntimePropertyFrame, ValueSlotId, ValueTypeId, compile_graph,
    evaluate_compiled_graph_with_external,
};
use indexmap::IndexSet;

use crate::{ChannelFrame, ChannelValidity, ManagedStageChain, OutputSetRuntime, ValueSet};

use super::{ManagedFormulaError, frame_values};
use crate::value_set::VALUE_SET_TYPE;

struct InputBoundary {
    node: ANodeId,
}

struct FilterBoundary {
    node: ANodeId,
    input_index: usize,
}

struct OutputBoundary {
    node: ANodeId,
    input_index: usize,
    output_set: usize,
}

pub(super) struct GraphManagedExecution {
    compiled: Arc<CompiledAlchemistGraph>,
    memory: LaneRuntimePool,
    scratch: AlchemistMemory,
    default_properties: RuntimePropertyFrame,
    input: InputBoundary,
    filter: Option<FilterBoundary>,
    outputs: Vec<OutputBoundary>,
    boundary_exec_nodes: Vec<chataigne_alchemist::ExecNodeId>,
}

pub(super) struct GraphManagedFrame<'a, 'ctx> {
    pub ctx: &'a EvaluationCtx<'ctx>,
    pub context_key: &'a ContextKey,
    pub properties: Option<&'a RuntimePropertyFrame>,
    pub capture_mode: DebugCaptureMode,
}

impl GraphManagedExecution {
    pub(super) fn reset_memory(&mut self) {
        self.memory.clear();
        self.scratch.reset_for_fresh_evaluation(&self.compiled);
    }

    pub(super) fn retain_context_keys(&mut self, active: &IndexSet<ContextKey>) {
        self.memory.retain_keys(active);
    }

    pub(super) fn migrate_memory_from(&mut self, previous: Self) {
        if Arc::ptr_eq(&self.compiled, &previous.compiled) {
            self.memory = previous.memory;
        }
    }

    pub(super) fn compile(
        formula: &AlchemistFormula,
        input: &ManagedRegionDefinition,
        filter: Option<&ManagedRegionDefinition>,
        outputs: &[&ManagedRegionDefinition],
        ctx: &CompileCtx<'_>,
        shared_graph: Option<Arc<CompiledAlchemistGraph>>,
    ) -> Result<Option<Self>, ManagedFormulaError> {
        if formula.graph.nodes().next().is_none() {
            return Ok(None);
        }
        let compiled = match shared_graph {
            Some(compiled) => compiled,
            None => {
                let result = compile_graph(&formula.graph, ctx);
                result
                    .compiled
                    .ok_or(ManagedFormulaError::GraphCompile(result.diagnostics))?
            }
        };
        let input_socket = input
            .output_socket
            .as_ref()
            .ok_or_else(|| boundary_error("InputSet requires an output graph socket"))?;
        require_single_value_set_output(&compiled, input_socket)?;
        let input = InputBoundary {
            node: input_socket.node,
        };

        let filter = filter
            .map(|definition| {
                let source = definition
                    .input_socket
                    .as_ref()
                    .ok_or_else(|| boundary_error("FilterPipeline requires an input graph socket"))?;
                let output = definition
                    .output_socket
                    .as_ref()
                    .ok_or_else(|| boundary_error("FilterPipeline requires an output graph socket"))?;
                if source.node != output.node {
                    return Err(boundary_error(
                        "typed FilterPipeline boundaries must be input and output sockets of one graph node",
                    ));
                }
                let input_index = require_value_set_input(&compiled, source)?;
                require_single_value_set_output(&compiled, output)?;
                Ok(FilterBoundary {
                    node: source.node,
                    input_index,
                })
            })
            .transpose()?;

        let outputs = outputs
            .iter()
            .enumerate()
            .map(|(output_set, definition)| {
                let socket = definition
                    .input_socket
                    .as_ref()
                    .ok_or_else(|| boundary_error("OutputSet requires an input graph socket"))?;
                let input_index = require_value_set_input(&compiled, socket)?;
                let node = graph_node(&compiled, socket.node)?;
                if !node.outputs.is_empty() {
                    return Err(boundary_error("OutputSet graph sink must have no outputs"));
                }
                Ok(OutputBoundary {
                    node: socket.node,
                    input_index,
                    output_set,
                })
            })
            .collect::<Result<Vec<_>, ManagedFormulaError>>()?;

        if filter.as_ref().is_some_and(|filter| filter.node == input.node)
            || outputs.iter().any(|output| {
                output.node == input.node || filter.as_ref().is_some_and(|filter| filter.node == output.node)
            })
        {
            return Err(boundary_error("each managed graph boundary must use a distinct node"));
        }
        if let Some(filter) = &filter
            && !reaches_input(&compiled, input.node, filter.node, filter.input_index)
        {
            return Err(boundary_error(
                "InputSet graph output does not feed FilterPipeline input",
            ));
        }
        let source = filter.as_ref().map_or(input.node, |filter| filter.node);
        for output in &outputs {
            if !reaches_input(&compiled, source, output.node, output.input_index) {
                return Err(boundary_error("managed graph value does not reach OutputSet input"));
            }
        }
        let memory = LaneRuntimePool::for_graph(&compiled);
        let scratch = AlchemistMemory::for_graph(&compiled);
        let default_properties = RuntimePropertyFrame::from_defaults(&compiled.properties);
        let mut boundary_exec_nodes = vec![graph_node(&compiled, input.node)?.exec_id];
        if let Some(filter) = &filter {
            boundary_exec_nodes.push(graph_node(&compiled, filter.node)?.exec_id);
        }
        for output in &outputs {
            let exec_id = graph_node(&compiled, output.node)?.exec_id;
            if !boundary_exec_nodes.contains(&exec_id) {
                boundary_exec_nodes.push(exec_id);
            }
        }
        Ok(Some(Self {
            compiled,
            memory,
            scratch,
            default_properties,
            input,
            filter,
            outputs,
            boundary_exec_nodes,
        }))
    }

    pub(super) fn evaluate(
        &mut self,
        input: &ChannelFrame,
        stages: &mut ManagedStageChain,
        output_sets: &[OutputSetRuntime],
        frame: GraphManagedFrame<'_, '_>,
    ) -> RuntimeOutput {
        let mut bridge = ManagedGraphBridge {
            input,
            stages,
            output_sets,
            input_node: self.input.node,
            filter: self.filter.as_ref(),
            outputs: &self.outputs,
            boundary_exec_nodes: &self.boundary_exec_nodes,
            context_key: frame.context_key,
        };
        let context = RuntimeContextFrame::new(frame.context_key.clone());
        let capture_enabled = !frame.capture_mode.is_off();
        let mut debug = DebugCaptureSink::new(frame.capture_mode);
        let memory = match self.memory.memory_for_key(frame.context_key.clone(), &self.compiled) {
            Some(memory) => memory,
            None => {
                self.scratch.reset_for_fresh_evaluation(&self.compiled);
                &mut self.scratch
            }
        };
        let mut output = evaluate_compiled_graph_with_external(
            &self.compiled,
            memory,
            EvaluationFrame {
                ctx: frame.ctx,
                properties: frame.properties.unwrap_or(&self.default_properties),
                context: &context,
                debug: capture_enabled.then_some(&mut debug),
                force_process_unchanged_inputs: false,
                capture_unchanged_outputs: false,
            },
            &mut bridge,
        );
        if !output.diagnostics.is_empty() {
            output.intents.clear();
            output.debug_samples.clear();
        }
        output
    }
}

struct ManagedGraphBridge<'a> {
    input: &'a ChannelFrame,
    stages: &'a mut ManagedStageChain,
    output_sets: &'a [OutputSetRuntime],
    input_node: ANodeId,
    filter: Option<&'a FilterBoundary>,
    outputs: &'a [OutputBoundary],
    boundary_exec_nodes: &'a [chataigne_alchemist::ExecNodeId],
    context_key: &'a ContextKey,
}

impl ExternalNodeEvaluator for ManagedGraphBridge<'_> {
    fn active_nodes(&self) -> &[chataigne_alchemist::ExecNodeId] {
        self.boundary_exec_nodes
    }

    fn evaluate(
        &mut self,
        evaluation: &mut NodeEvaluation<'_, '_>,
    ) -> Result<chataigne_alchemist::NodeOutputs, String> {
        if evaluation.author_node_id == self.input_node {
            let values = frame_values(self.input).map_err(|error| error.to_string())?;
            return values
                .to_runtime_value()
                .map(|value| chataigne_alchemist::node_outputs![value])
                .map_err(|error| error.to_string());
        }
        if let Some(filter) = self.filter.filter(|filter| filter.node == evaluation.author_node_id) {
            let source = evaluation
                .inputs
                .get(filter.input_index)
                .ok_or_else(|| "FilterPipeline graph input is missing".to_string())?;
            let values = ValueSet::from_runtime_value(source).map_err(|error| error.to_string())?;
            let frame = frame_from_values(&values, self.stages.input_layout(), evaluation.ctx.logical_tick)?;
            let capture_mode = evaluation
                .debug
                .as_ref()
                .map_or(DebugCaptureMode::Off, |_| DebugCaptureMode::All {
                    history_len: usize::MAX,
                });
            let (filtered, effects) = self
                .stages
                .evaluate_with_capture_for_context(&frame, evaluation.ctx, capture_mode, self.context_key)
                .map_err(|error| error.to_string())?;
            if !effects.diagnostics.is_empty() {
                return Err(effects
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; "));
            }
            evaluation.intents.extend(effects.intents);
            if let Some(debug) = evaluation.debug.as_deref_mut() {
                for mut sample in effects.debug_samples {
                    sample.context_key = (!evaluation.context.context_key().is_default_lane())
                        .then(|| evaluation.context.context_key().clone());
                    debug.capture(sample);
                }
            }
            if filtered
                .slots()
                .iter()
                .any(|slot| slot.validity == ChannelValidity::Suppressed)
            {
                evaluation.suppress_output(0);
                return Ok(chataigne_alchemist::node_outputs![source.clone()]);
            }
            let values = frame_values(filtered).map_err(|error| error.to_string())?;
            return values
                .to_runtime_value()
                .map(|value| chataigne_alchemist::node_outputs![value])
                .map_err(|error| error.to_string());
        }
        for output in self
            .outputs
            .iter()
            .filter(|output| output.node == evaluation.author_node_id)
        {
            let value = evaluation
                .inputs
                .get(output.input_index)
                .ok_or_else(|| "OutputSet graph input is missing".to_string())?;
            let materialized = self.output_sets[output.output_set].materialize(value, evaluation.ctx);
            if !materialized.diagnostics.is_empty() {
                return Err(materialized
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; "));
            }
            if !materialized.output.diagnostics.is_empty() {
                return Err(materialized
                    .output
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("; "));
            }
            evaluation.intents.extend(materialized.output.intents);
        }
        Ok(chataigne_alchemist::NodeOutputs::new())
    }
}

fn frame_from_values(values: &ValueSet, layout: &Arc<ChannelLayout>, tick: u64) -> Result<ChannelFrame, String> {
    if values.entries.len() != layout.channels().len() {
        return Err(format!(
            "Formula graph supplied {} tuple elements; FilterPipeline expects {}",
            values.entries.len(),
            layout.channels().len()
        ));
    }
    let mut frame = ChannelFrame::new(Arc::clone(layout));
    frame.begin_tick(tick);
    for (index, (entry, channel)) in values.entries.iter().zip(layout.channels()).enumerate() {
        if entry.key != channel.id || channel.value_type.as_ref() != Some(&entry.value.value_type()) {
            return Err(format!(
                "Formula graph changed tuple element `{}` before FilterPipeline",
                channel.id.as_str()
            ));
        }
        frame
            .set(index, Some(entry.value.clone()), ChannelValidity::Valid, true)
            .map_err(|error| error.to_string())?;
    }
    Ok(frame)
}

fn graph_node(compiled: &CompiledAlchemistGraph, authored: ANodeId) -> Result<&CompiledExecNode, ManagedFormulaError> {
    compiled
        .exec_nodes
        .iter()
        .find(|node| node.authored_id == authored)
        .ok_or_else(|| boundary_error(format!("graph node `{authored}` is missing")))
}

fn reaches_input(compiled: &CompiledAlchemistGraph, source: ANodeId, target: ANodeId, input_index: usize) -> bool {
    let Some(source) = compiled.exec_nodes.iter().find(|node| node.authored_id == source) else {
        return false;
    };
    let mut pending = source.outputs.clone();
    let mut visited_slots = vec![false; compiled.state_layout.value_slot_count];
    let mut visited_nodes = vec![false; compiled.exec_nodes.len()];
    while let Some(slot) = pending.pop() {
        if std::mem::replace(&mut visited_slots[slot.index()], true) {
            continue;
        }
        for dependent in &compiled.dependencies.slot_dependents[slot.index()] {
            let node = &compiled.exec_nodes[dependent.index()];
            if node.authored_id == target
                && node
                    .inputs
                    .get(input_index)
                    .is_some_and(|input| input_depends_on_slot(input, slot))
            {
                return true;
            }
            if !std::mem::replace(&mut visited_nodes[dependent.index()], true) {
                pending.extend(node.outputs.iter().copied());
            }
        }
    }
    false
}

fn input_depends_on_slot(source: &InputValueSource, slot: ValueSlotId) -> bool {
    match source {
        InputValueSource::Slot(source) => *source == slot,
        InputValueSource::Converted { source, .. }
        | InputValueSource::Component { source, .. }
        | InputValueSource::RuntimeInput { fallback: source, .. } => input_depends_on_slot(source, slot),
        InputValueSource::Composite { base, components, .. } => {
            input_depends_on_slot(base, slot)
                || components.iter().any(|(_, source)| input_depends_on_slot(source, slot))
        }
        InputValueSource::Constant(_) | InputValueSource::Unset => false,
    }
}

fn require_single_value_set_output(
    compiled: &CompiledAlchemistGraph,
    socket: &ManagedSocketRef,
) -> Result<(), ManagedFormulaError> {
    let node = graph_node(compiled, socket.node)?;
    if node.outputs.len() != 1
        || node.output_sockets.first() != Some(&socket.socket)
        || node.output_types.first() != Some(&Some(ValueTypeId::new(VALUE_SET_TYPE)))
    {
        return Err(boundary_error(format!(
            "graph node `{}` must expose one `{VALUE_SET_TYPE}` output socket `{}`",
            socket.node, socket.socket
        )));
    }
    Ok(())
}

fn require_value_set_input(
    compiled: &CompiledAlchemistGraph,
    socket: &ManagedSocketRef,
) -> Result<usize, ManagedFormulaError> {
    let node = graph_node(compiled, socket.node)?;
    let index = node
        .input_sockets
        .iter()
        .position(|candidate| candidate == &socket.socket)
        .ok_or_else(|| {
            boundary_error(format!(
                "graph node `{}` has no input socket `{}`",
                socket.node, socket.socket
            ))
        })?;
    if node.input_types.get(index) != Some(&Some(ValueTypeId::new(VALUE_SET_TYPE))) {
        return Err(boundary_error(format!(
            "graph node `{}` input `{}` must accept `{VALUE_SET_TYPE}`",
            socket.node, socket.socket
        )));
    }
    Ok(index)
}

fn boundary_error(message: impl Into<String>) -> ManagedFormulaError {
    ManagedFormulaError::GraphBoundary(message.into())
}
