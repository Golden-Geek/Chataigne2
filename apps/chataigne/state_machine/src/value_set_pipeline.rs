use std::sync::Arc;

use golden_alchemist::{
    ANodeId, ANodeInstance, ANodeTypeId, AlchemistGraph, CompileCtx, CompiledAlchemistGraph, ContextAxisId,
    ContextItemId, ContextKey, DebugCaptureMode, DebugCaptureSink, EvaluationCtx, EvaluationFrame, FormulaPropertyDecl,
    FormulaPropertyId, FormulaPropertySchema, InputSocketRef, LaneRuntimePool, ManagedItemInstance,
    ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance, ManagedRegionKind, ManagedSocketRef,
    OutputSocketRef, ParamUiHints, PipelineLoweringCtx, RuntimeContextFrame, RuntimeOutput, RuntimePropertyFrame,
    RuntimeValue, SocketId, StableRef, SurfaceItemKind, ValueTypeId, compile_graph, evaluate_compiled_graph,
    evaluate_compiled_graph_stateless, lower_filter_pipeline_region, single_shape,
};
use indexmap::IndexMap;

use crate::{ValueSet, ValueSetEntry};

const PIPELINE_INPUT_PROPERTY: &str = "pipeline_value";
const PIPELINE_REGION: &str = "filters";
const VALUE_LANE_AXIS: &str = "value_set_lane";

pub struct ValueSetPipelineRuntime {
    compiled: Arc<CompiledAlchemistGraph>,
    output_node: ANodeId,
    output_socket: SocketId,
    memory: LaneRuntimePool,
}

pub struct ValueSetProjectionRuntime {
    compiled: Arc<CompiledAlchemistGraph>,
    output_node: ANodeId,
    output_socket: SocketId,
    property_ids: Vec<FormulaPropertyId>,
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
        let memory = LaneRuntimePool::for_graph(&compiled);

        Ok(Self {
            compiled,
            output_node,
            output_socket: SocketId::new("value"),
            memory,
        })
    }

    pub fn evaluate(
        &mut self,
        values: &ValueSet,
        ctx: &EvaluationCtx<'_>,
    ) -> Result<(ValueSet, RuntimeOutput), ValueSetPipelineError> {
        let active_keys = values
            .entries
            .iter()
            .map(|entry| lane_context_key(entry.key.as_str()))
            .collect();
        self.memory.retain_keys(&active_keys);

        let mut output = RuntimeOutput::default();
        let mut entries = Vec::with_capacity(values.entries.len());
        for entry in &values.entries {
            let context_key = lane_context_key(entry.key.as_str());
            let properties = property_frame(&self.compiled, entry.value.clone())?;
            let context = RuntimeContextFrame::new(context_key.clone());
            let mut debug = DebugCaptureSink::new(DebugCaptureMode::SelectedNodes {
                formula_id: None,
                context_key: Some(context_key.clone()),
                nodes: [self.output_node].into_iter().collect(),
                history_len: 1,
            });
            let frame = EvaluationFrame {
                ctx,
                properties: &properties,
                context: &context,
                debug: &mut debug,
                force_process_unchanged_inputs: true,
                capture_unchanged_outputs: true,
            };
            let lane_output = match self.memory.memory_for_key(context_key, &self.compiled) {
                Some(memory) => evaluate_compiled_graph(&self.compiled, memory, frame),
                None => evaluate_compiled_graph_stateless(&self.compiled, frame),
            };
            output.intents.extend(lane_output.intents);
            output.diagnostics.extend(lane_output.diagnostics);
            output.debug_samples.extend(lane_output.debug_samples.clone());
            let value = lane_output
                .debug_samples
                .iter()
                .rev()
                .find(|sample| sample.author_node_id == self.output_node && sample.output_socket == self.output_socket)
                .map(|sample| sample.value.clone())
                .ok_or_else(|| ValueSetPipelineError::MissingOutput(entry.label.clone()))?;
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
        &self,
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
        let mut debug = DebugCaptureSink::new(DebugCaptureMode::SelectedNodes {
            formula_id: None,
            context_key: None,
            nodes: [self.output_node].into_iter().collect(),
            history_len: 1,
        });
        let frame = EvaluationFrame {
            ctx,
            properties: &properties,
            context: &context,
            debug: &mut debug,
            force_process_unchanged_inputs: true,
            capture_unchanged_outputs: true,
        };
        let output = evaluate_compiled_graph_stateless(&self.compiled, frame);
        let value = output
            .debug_samples
            .iter()
            .rev()
            .find(|sample| sample.author_node_id == self.output_node && sample.output_socket == self.output_socket)
            .map(|sample| sample.value.clone())
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

    Ok(ValueSetProjectionRuntime {
        compiled,
        output_node,
        output_socket,
        property_ids,
    })
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
