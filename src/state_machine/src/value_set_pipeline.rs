use std::collections::HashSet;
use std::sync::Arc;

use golden_alchemist::{
    ANodeInstance, ANodeTypeId, AlchemistGraph, AlchemistMemory, CompileCtx, CompiledAlchemistGraph, ContextAxisId,
    ContextItemId, ContextKey, DebugCaptureMode, DebugCaptureSink, EvaluationCtx, EvaluationFrame, FormulaPropertyDecl,
    FormulaPropertyId, FormulaPropertySchema, InputSocketRef, LaneRuntimePool, ManagedItemInstance,
    ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance, ManagedRegionKind, ManagedSocketRef,
    OutputSocketRef, ParamUiHints, PipelineLoweringCtx, RuntimeContextFrame, RuntimeOutput, RuntimePropertyFrame,
    RuntimeValue, SocketId, StableRef, SurfaceItemKind, ValueSlotId, ValueTypeId, compile_graph,
    evaluate_compiled_graph, lower_filter_pipeline_region, single_shape,
};
use indexmap::IndexMap;

use crate::{ValueSet, ValueSetEntry};

const PIPELINE_INPUT_PROPERTY: &str = "pipeline_value";
const PIPELINE_REGION: &str = "filters";
const VALUE_LANE_AXIS: &str = "value_set_lane";

pub struct ValueSetPipelineRuntime {
    compiled: Arc<CompiledAlchemistGraph>,
    output_slot: ValueSlotId,
    memory: LaneRuntimePool,
    stateless_cache: IndexMap<String, CachedLaneOutput>,
    pending_invalidation: Option<PipelineInvalidationReason>,
    last_stats: PipelineEvaluationStats,
}

pub struct ValueSetProjectionRuntime {
    compiled: Arc<CompiledAlchemistGraph>,
    output_slot: ValueSlotId,
    property_ids: Vec<FormulaPropertyId>,
    memory: AlchemistMemory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PipelineInvalidationReason {
    InputChange,
    GraphChange,
    TimeDependentTick,
    ExternalSideEffect,
    DebugRequest,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PipelineEvaluationStats {
    pub evaluated_lanes: usize,
    pub reused_lanes: usize,
    pub reasons: Vec<PipelineInvalidationReason>,
}

struct CachedLaneOutput {
    input: RuntimeValue,
    output: RuntimeValue,
}

impl ValueSetPipelineRuntime {
    pub fn compile_elementwise(
        items: Vec<ManagedItemInstance>,
        item_type: ValueTypeId,
        ctx: &PipelineLoweringCtx<'_>,
    ) -> Result<Self, ValueSetPipelineError> {
        let items = normalize_elementwise_items(items);
        let mut graph = AlchemistGraph::new();
        let mut input = ANodeInstance::new(ANodeTypeId::new("property"), "Pipeline Input");
        input.config.set(
            "property_id",
            RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), PIPELINE_INPUT_PROPERTY)),
        );
        let input_node = graph.add_node(input).map_err(ValueSetPipelineError::Graph)?;
        let output = ANodeInstance::new(ANodeTypeId::new("debug_value"), "Pipeline Output");
        let output_node = output.id;
        graph.add_node(output).map_err(ValueSetPipelineError::Graph)?;

        let definition = ManagedRegionDefinition {
            id: ManagedRegionId::new(PIPELINE_REGION),
            kind: ManagedRegionKind::FilterPipeline,
            label: "Filters".into(),
            input_socket: Some(ManagedSocketRef::new(input_node, "value")),
            output_socket: Some(ManagedSocketRef::new(output_node, "value")),
            accepted_roles: vec![SurfaceItemKind::Filter],
        };
        let instance = ManagedRegionInstance {
            region_id: ManagedRegionId::new(PIPELINE_REGION),
            items,
        };

        let lowered =
            lower_filter_pipeline_region(&graph, &definition, &instance, single_shape(item_type.clone()), ctx);
        if !lowered.is_valid() {
            return Err(ValueSetPipelineError::Lowering {
                diagnostics: lowered.diagnostics,
                shape_diagnostics: lowered.shape.diagnostics,
            });
        }

        let default_value = ctx
            .value_types
            .default_value(&item_type)
            .ok_or_else(|| ValueSetPipelineError::MissingDefaultValue(item_type.clone()))?;
        let properties = pipeline_property_schema(item_type, default_value);
        let compiled = compile_graph(
            &lowered.graph,
            &CompileCtx {
                value_types: ctx.value_types,
                nodes: ctx.nodes,
                properties: Some(&properties),
            },
        );
        if compiled.has_errors() {
            return Err(ValueSetPipelineError::Compile(compiled.diagnostics));
        }
        let compiled = compiled.compiled.ok_or(ValueSetPipelineError::MissingCompiledGraph)?;
        let output_socket = SocketId::new("value");
        let output_slot = compiled
            .output_slot(output_node, &output_socket)
            .ok_or_else(|| ValueSetPipelineError::MissingOutput(output_socket.as_str().into()))?;
        let memory = LaneRuntimePool::for_graph(&compiled);

        Ok(Self {
            compiled,
            output_slot,
            memory,
            stateless_cache: IndexMap::new(),
            pending_invalidation: Some(PipelineInvalidationReason::GraphChange),
            last_stats: PipelineEvaluationStats::default(),
        })
    }

    pub fn evaluate(
        &mut self,
        values: &ValueSet,
        ctx: &EvaluationCtx<'_>,
    ) -> Result<(ValueSet, RuntimeOutput), ValueSetPipelineError> {
        self.evaluate_observed(values, ctx, None)
    }

    pub fn evaluate_with_debug(
        &mut self,
        values: &ValueSet,
        ctx: &EvaluationCtx<'_>,
        capture_mode: DebugCaptureMode,
    ) -> Result<(ValueSet, RuntimeOutput), ValueSetPipelineError> {
        self.pending_invalidation = Some(PipelineInvalidationReason::DebugRequest);
        self.evaluate_observed(values, ctx, Some(capture_mode))
    }

    pub fn invalidate(&mut self, reason: PipelineInvalidationReason) {
        self.pending_invalidation = Some(reason);
        if matches!(
            reason,
            PipelineInvalidationReason::GraphChange | PipelineInvalidationReason::ExternalSideEffect
        ) {
            self.stateless_cache.clear();
        }
    }

    #[must_use]
    pub fn last_evaluation_stats(&self) -> &PipelineEvaluationStats {
        &self.last_stats
    }

    fn evaluate_observed(
        &mut self,
        values: &ValueSet,
        ctx: &EvaluationCtx<'_>,
        capture_mode: Option<DebugCaptureMode>,
    ) -> Result<(ValueSet, RuntimeOutput), ValueSetPipelineError> {
        let active_keys = values
            .entries
            .iter()
            .map(|entry| lane_context_key(entry.key.as_str()))
            .collect();
        self.memory.retain_keys(&active_keys);
        let active_lane_keys = values
            .entries
            .iter()
            .map(|entry| entry.key.as_str())
            .collect::<HashSet<_>>();
        self.stateless_cache
            .retain(|key, _| active_lane_keys.contains(key.as_str()));

        let mut output = RuntimeOutput::default();
        let mut entries = Vec::with_capacity(values.entries.len());
        let pending_invalidation = self.pending_invalidation.take();
        let force_evaluation = pending_invalidation.is_some();
        self.last_stats = PipelineEvaluationStats::default();
        if let Some(reason) = pending_invalidation {
            push_reason(&mut self.last_stats.reasons, reason);
        }
        for entry in &values.entries {
            let context_key = lane_context_key(entry.key.as_str());
            let cached = self.stateless_cache.get(entry.key.as_str());
            let input_changed = cached.is_none_or(|cached| cached.input != entry.value);
            let time_dependent = self.compiled.analysis.has_always_process_nodes;
            let can_reuse_stateless = !force_evaluation && !time_dependent && !input_changed;

            let value = if self.compiled.state_layout.state_slot_count == 0 && can_reuse_stateless {
                self.last_stats.reused_lanes += 1;
                cached.expect("cache was checked above").output.clone()
            } else {
                if input_changed {
                    push_reason(&mut self.last_stats.reasons, PipelineInvalidationReason::InputChange);
                }
                if time_dependent {
                    push_reason(
                        &mut self.last_stats.reasons,
                        PipelineInvalidationReason::TimeDependentTick,
                    );
                }

                let properties = property_frame(&self.compiled, entry.value.clone())?;
                let context = RuntimeContextFrame::new(context_key.clone());
                let mut debug = DebugCaptureSink::new(capture_mode.clone().unwrap_or(DebugCaptureMode::Off));
                let frame = EvaluationFrame {
                    ctx,
                    properties: &properties,
                    context: &context,
                    debug: &mut debug,
                    force_process_unchanged_inputs: force_evaluation,
                    capture_unchanged_outputs: capture_mode.is_some(),
                };

                let (mut lane_output, value) =
                    if let Some(memory) = self.memory.memory_for_key(context_key, &self.compiled) {
                        let lane_output = evaluate_compiled_graph(&self.compiled, memory, frame);
                        let value = memory
                            .initialized_value(self.output_slot)
                            .cloned()
                            .ok_or_else(|| ValueSetPipelineError::MissingOutput(entry.label.clone()))?;
                        (lane_output, value)
                    } else {
                        let mut memory = AlchemistMemory::for_graph(&self.compiled);
                        let lane_output = evaluate_compiled_graph(&self.compiled, &mut memory, frame);
                        let value = memory
                            .initialized_value(self.output_slot)
                            .cloned()
                            .ok_or_else(|| ValueSetPipelineError::MissingOutput(entry.label.clone()))?;
                        (lane_output, value)
                    };
                self.last_stats.evaluated_lanes += 1;
                output.intents.append(&mut lane_output.intents);
                output.diagnostics.append(&mut lane_output.diagnostics);
                output.debug_samples.append(&mut lane_output.debug_samples);
                output.trigger_fired |= lane_output.trigger_fired;
                if self.compiled.state_layout.state_slot_count == 0 {
                    self.stateless_cache.insert(
                        entry.key.as_str().to_owned(),
                        CachedLaneOutput {
                            input: entry.value.clone(),
                            output: value.clone(),
                        },
                    );
                }
                value
            };
            entries.push(ValueSetEntry {
                key: entry.key.clone(),
                label: entry.label.clone(),
                source: entry.source.clone(),
                value,
            });
        }

        Ok((
            ValueSet::with_entries(ctx.logical_tick.max(values.logical_tick), entries),
            output,
        ))
    }

    #[cfg(test)]
    pub(crate) fn state_slot_count(&self) -> usize {
        self.compiled.state_layout.state_slot_count
    }

    #[cfg(test)]
    pub(crate) fn lane_memory_count(&self) -> usize {
        self.memory.memory_count()
    }
}

impl ValueSetProjectionRuntime {
    pub fn compile_aggregate(
        item: ManagedItemInstance,
        input_count: usize,
        item_type: ValueTypeId,
        ctx: &PipelineLoweringCtx<'_>,
    ) -> Result<Self, ValueSetPipelineError> {
        let input_sockets = (0..input_count)
            .map(|index| SocketId::new(format!("value{}", index + 1)))
            .collect::<Vec<_>>();
        compile_projection(item, input_sockets, SocketId::new("result"), item_type, ctx)
    }

    pub fn compile_pack_vec3(
        item: ManagedItemInstance,
        ctx: &PipelineLoweringCtx<'_>,
    ) -> Result<Self, ValueSetPipelineError> {
        compile_projection(
            item,
            vec![SocketId::new("x"), SocketId::new("y"), SocketId::new("z")],
            SocketId::new("value"),
            ValueTypeId::new("float"),
            ctx,
        )
    }

    pub fn evaluate(
        &mut self,
        values: &ValueSet,
        ctx: &EvaluationCtx<'_>,
    ) -> Result<(RuntimeValue, RuntimeOutput), ValueSetPipelineError> {
        if values.entries.len() != self.property_ids.len() {
            return Err(ValueSetPipelineError::LaneCountMismatch {
                expected: self.property_ids.len(),
                actual: values.entries.len(),
            });
        }

        let properties = property_frame_for_entries(&self.compiled, &self.property_ids, values)?;
        let context = RuntimeContextFrame::default_lane();
        let mut debug = DebugCaptureSink::new(DebugCaptureMode::Off);
        let frame = EvaluationFrame {
            ctx,
            properties: &properties,
            context: &context,
            debug: &mut debug,
            force_process_unchanged_inputs: false,
            capture_unchanged_outputs: false,
        };
        let output = evaluate_compiled_graph(&self.compiled, &mut self.memory, frame);
        let value = self
            .memory
            .initialized_value(self.output_slot)
            .cloned()
            .ok_or_else(|| ValueSetPipelineError::MissingOutput("projection".into()))?;
        Ok((value, output))
    }
}

fn compile_projection(
    item: ManagedItemInstance,
    input_sockets: Vec<SocketId>,
    output_socket: SocketId,
    item_type: ValueTypeId,
    ctx: &PipelineLoweringCtx<'_>,
) -> Result<ValueSetProjectionRuntime, ValueSetPipelineError> {
    if input_sockets.is_empty() {
        return Err(ValueSetPipelineError::EmptyProjection);
    }
    let default_value = ctx
        .value_types
        .default_value(&item_type)
        .ok_or_else(|| ValueSetPipelineError::MissingDefaultValue(item_type.clone()))?;
    let mut graph = AlchemistGraph::new();
    let output_node = item.anode.id;
    graph.add_node(item.anode).map_err(ValueSetPipelineError::Graph)?;

    let mut properties = FormulaPropertySchema::default();
    let mut property_ids = Vec::with_capacity(input_sockets.len());
    for (index, input_socket) in input_sockets.into_iter().enumerate() {
        let property_name = format!("{PIPELINE_INPUT_PROPERTY}_{index}");
        let property_id = FormulaPropertyId::new(property_name.as_str());
        properties.insert(FormulaPropertyDecl {
            id: property_id.clone(),
            label: format!("Pipeline Value {}", index + 1),
            description: None,
            value_type: item_type.clone(),
            default_value: default_value.clone(),
            ui: ParamUiHints::default(),
        });
        let mut input = ANodeInstance::new(ANodeTypeId::new("property"), format!("Pipeline Input {}", index + 1));
        input.config.set(
            "property_id",
            RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), property_name.as_str())),
        );
        let input_node = graph.add_node(input).map_err(ValueSetPipelineError::Graph)?;
        graph
            .connect(
                OutputSocketRef::new(input_node, "value"),
                InputSocketRef::new(output_node, input_socket),
            )
            .map_err(ValueSetPipelineError::Graph)?;
        property_ids.push(property_id);
    }

    let compiled = compile_graph(
        &graph,
        &CompileCtx {
            value_types: ctx.value_types,
            nodes: ctx.nodes,
            properties: Some(&properties),
        },
    );
    if compiled.has_errors() {
        return Err(ValueSetPipelineError::Compile(compiled.diagnostics));
    }
    let compiled = compiled.compiled.ok_or(ValueSetPipelineError::MissingCompiledGraph)?;
    let output_slot = compiled
        .output_slot(output_node, &output_socket)
        .ok_or_else(|| ValueSetPipelineError::MissingOutput(output_socket.as_str().into()))?;
    let memory = AlchemistMemory::for_graph(&compiled);

    Ok(ValueSetProjectionRuntime {
        compiled,
        output_slot,
        property_ids,
        memory,
    })
}

fn push_reason(reasons: &mut Vec<PipelineInvalidationReason>, reason: PipelineInvalidationReason) {
    if !reasons.contains(&reason) {
        reasons.push(reason);
    }
}

fn pipeline_property_schema(item_type: ValueTypeId, default_value: RuntimeValue) -> FormulaPropertySchema {
    let mut schema = FormulaPropertySchema::default();
    schema.insert(FormulaPropertyDecl {
        id: FormulaPropertyId::new(PIPELINE_INPUT_PROPERTY),
        label: "Pipeline Value".into(),
        description: None,
        value_type: item_type,
        default_value,
        ui: ParamUiHints::default(),
    });
    schema
}

fn property_frame_for_entries(
    compiled: &CompiledAlchemistGraph,
    property_ids: &[FormulaPropertyId],
    values: &ValueSet,
) -> Result<RuntimePropertyFrame, ValueSetPipelineError> {
    let overrides = property_ids
        .iter()
        .zip(values.entries.iter())
        .map(|(property_id, entry)| (property_id.clone(), entry.value.clone()))
        .collect::<IndexMap<_, _>>();
    RuntimePropertyFrame::with_overrides(&compiled.properties, &overrides).map_err(ValueSetPipelineError::PropertyFrame)
}

fn property_frame(
    compiled: &CompiledAlchemistGraph,
    value: RuntimeValue,
) -> Result<RuntimePropertyFrame, ValueSetPipelineError> {
    let mut overrides = IndexMap::new();
    overrides.insert(FormulaPropertyId::new(PIPELINE_INPUT_PROPERTY), value);
    RuntimePropertyFrame::with_overrides(&compiled.properties, &overrides).map_err(ValueSetPipelineError::PropertyFrame)
}

fn lane_context_key(key: &str) -> ContextKey {
    ContextKey::single(ContextAxisId::new(VALUE_LANE_AXIS), ContextItemId::new(key))
}

fn normalize_elementwise_items(mut items: Vec<ManagedItemInstance>) -> Vec<ManagedItemInstance> {
    for item in &mut items {
        if item.anode.type_id == ANodeTypeId::new("condition_gate") {
            // This runtime already lowers one scalar graph per ValueSet lane.
            item.anode
                .config
                .set("gate_application", RuntimeValue::String("whole".into()));
        }
    }
    items
}

#[derive(Debug, thiserror::Error)]
pub enum ValueSetPipelineError {
    #[error("{0}")]
    Graph(golden_alchemist::GraphEditError),
    #[error("managed filter pipeline failed to lower")]
    Lowering {
        diagnostics: Vec<golden_alchemist::PipelineLoweringDiagnostic>,
        shape_diagnostics: Vec<golden_alchemist::PipelineShapeDiagnostic>,
    },
    #[error("managed filter pipeline failed to compile")]
    Compile(Vec<golden_alchemist::Diagnostic>),
    #[error("managed filter pipeline did not produce a compiled graph")]
    MissingCompiledGraph,
    #[error("no default value registered for pipeline item type `{0:?}`")]
    MissingDefaultValue(ValueTypeId),
    #[error("projection pipeline requires at least one input lane")]
    EmptyProjection,
    #[error("projection pipeline expected {expected} lanes, got {actual}")]
    LaneCountMismatch { expected: usize, actual: usize },
    #[error("{0}")]
    PropertyFrame(golden_alchemist::RuntimePropertyFrameError),
    #[error("managed filter pipeline produced no output for `{0}`")]
    MissingOutput(String),
}
