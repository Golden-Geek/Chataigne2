use std::sync::Arc;

use chataigne_alchemist::{
    ANodeId, ANodeRegistry, AlchemistFormula, AlchemistFormulaInstance, ChannelLayout, ChannelProvenance, CompileCtx,
    CompiledAlchemistGraph, ContextKey, DebugCaptureMode, Diagnostic, DiagnosticOrigin, EvaluationCtx, ExecNodeId,
    FormulaPropertySchema, ManagedFilterValueMode, ManagedItemInstance, ManagedRegionDefinition, ManagedRegionId,
    ManagedRegionInstance, ManagedRegionKind, RuntimeDiagnostic, RuntimeIntent, RuntimeOutput, RuntimePropertyFrame,
    StableRef, SurfaceItemKind, ValueTypeId, ValueTypeRegistry,
};
use golden_values::Value as RuntimeValue;
use indexmap::IndexSet;

use crate::{
    COMMAND_INTENT_KIND, ChannelFrame, ChannelSourceSchema, ChannelValidity, INPUT_SOURCE_FIELD, InputSetRuntime,
    ManagedStageChain, ManagedStageRuntime, ManagedStageSpecializationCache, OUTPUT_TARGET_FIELD,
    OutputSetMaterialization, OutputSetRuntime, RuntimeInputBinding, ValueLaneKey, ValueSet, ValueSetEntry,
};

mod availability;
mod error;
mod graph;

use graph::{GraphManagedExecution, GraphManagedFrame};

pub use availability::{
    ExecutableFilterApplication, ManagedFilterAvailabilityError, executable_filter_applications,
    validate_executable_filter_application, validate_mapping_filter_application,
};
pub use error::ManagedFormulaError;

pub fn validate_trigger_filter_application(
    anode: &chataigne_alchemist::ANodeInstance,
    ctx: &CompileCtx<'_>,
) -> Result<(), ManagedFormulaError> {
    let layout = ChannelLayout::new(vec![chataigne_alchemist::ChannelDescriptor::input(
        ValueLaneKey::new("trigger").expect("static trigger identity"),
        "Trigger",
        StableRef::new(ValueTypeId::new("source"), "trigger"),
        Some(ValueTypeId::new("trigger")),
    )])
    .expect("single trigger layout");
    let item = ManagedItemInstance {
        id: chataigne_alchemist::ManagedItemId::new(),
        anode: anode.clone(),
        enabled: true,
        ui_state: chataigne_alchemist::ManagedItemUiState::default(),
    };
    let stage = ManagedStageRuntime::compile(item, &layout, ctx, ManagedFilterValueMode::Routed)?.ok_or_else(|| {
        ManagedFormulaError::UnsupportedFilterPipeline("trigger filter has no compatible input".into())
    })?;
    require_single_trigger_output(stage.output_layout())
}

fn require_single_trigger_output(layout: &ChannelLayout) -> Result<(), ManagedFormulaError> {
    if layout.channels().len() != 1 {
        return Err(ManagedFormulaError::TriggerFilterExpectedSingleValue {
            actual: layout.channels().len(),
        });
    }
    if layout.channels()[0].value_type.as_ref() != Some(&ValueTypeId::new("trigger")) {
        return Err(ManagedFormulaError::UnsupportedFilterPipeline(
            "trigger filter must preserve a single trigger value".into(),
        ));
    }
    Ok(())
}

pub struct ManagedFormulaRuntime {
    kind: ManagedFormulaRuntimeKind,
}

enum ManagedFormulaRuntimeKind {
    ValuePipeline(Box<ValuePipelineRuntime>),
    TriggerPipeline(Box<TriggerPipelineRuntime>),
}

struct ValuePipelineRuntime {
    input_set: InputSetRuntime,
    filter_items: Vec<ManagedItemInstance>,
    filter_value_mode: ManagedFilterValueMode,
    typed_stages: Option<ManagedStageChain>,
    graph: Option<GraphManagedExecution>,
    value_types: ValueTypeRegistry,
    nodes: ANodeRegistry,
    properties: Option<FormulaPropertySchema>,
    output_sets: Vec<OutputSetRuntime>,
}

struct TriggerPipelineRuntime {
    trigger: TriggerInputRuntime,
    typed_stages: Option<ManagedStageChain>,
    input_frame: Option<ChannelFrame>,
    commands: Vec<CommandSetRuntime>,
}

impl ManagedFormulaRuntime {
    pub fn compile(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
    ) -> Result<Option<Self>, ManagedFormulaError> {
        Self::compile_inner(formula, instance, ctx, None)
    }

    pub fn compile_with_shared_graph(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
        graph: Arc<CompiledAlchemistGraph>,
    ) -> Result<Option<Self>, ManagedFormulaError> {
        Self::compile_inner(formula, instance, ctx, Some(graph))
    }

    fn compile_inner(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
        graph: Option<Arc<CompiledAlchemistGraph>>,
    ) -> Result<Option<Self>, ManagedFormulaError> {
        let has_value_pipeline_regions = formula.surface.managed_regions.iter().any(is_value_pipeline_region);
        let has_trigger_pipeline_regions = formula.surface.managed_regions.iter().any(is_trigger_pipeline_region);
        if !has_value_pipeline_regions && !has_trigger_pipeline_regions {
            return Ok(None);
        }
        if has_value_pipeline_regions && has_trigger_pipeline_regions {
            return Err(ManagedFormulaError::MixedManagedFormulaPipelines);
        }
        instance
            .require_compatible(formula)
            .map_err(ManagedFormulaError::Formula)?;
        instance
            .managed_regions
            .validate_against(&formula.surface)
            .map_err(ManagedFormulaError::ManagedRegionValidation)?;

        if has_trigger_pipeline_regions {
            if formula.graph.nodes().next().is_some() {
                return Err(ManagedFormulaError::GraphBoundary(
                    "trigger managed regions in an authored Formula graph require explicit graph boundary lowering"
                        .to_owned(),
                ));
            }
            return Self::compile_trigger_pipeline(formula, instance, ctx).map(Some);
        }
        Self::compile_value_pipeline(formula, instance, ctx, graph).map(Some)
    }

    fn compile_value_pipeline(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
        shared_graph: Option<Arc<CompiledAlchemistGraph>>,
    ) -> Result<Self, ManagedFormulaError> {
        let input = required_region(&formula.surface.managed_regions, ManagedRegionKind::InputSet)?;
        let outputs = required_regions(&formula.surface.managed_regions, ManagedRegionKind::OutputSet)?;
        let filter = optional_region(&formula.surface.managed_regions, ManagedRegionKind::FilterPipeline)?;

        let input_instance = required_region_instance(instance, &input.id)?;
        let output_sets = outputs
            .iter()
            .map(|definition| {
                let region = required_region_instance(instance, &definition.id)?;
                OutputSetRuntime::from_managed_region(definition, region).map_err(ManagedFormulaError::from)
            })
            .collect::<Result<Vec<_>, ManagedFormulaError>>()?;
        let filter_instance = filter
            .map(|definition| required_region_instance(instance, &definition.id).map(|region| (definition, region)))
            .transpose()?;
        if let Some((definition, region)) = filter_instance {
            validate_filter_region(definition, region)?;
        }

        let (filter_items, filter_value_mode) = if let Some((definition, region)) = filter_instance {
            validate_filter_region(definition, region)?;
            (region.items.clone(), definition.filter_value_mode)
        } else {
            (Vec::new(), ManagedFilterValueMode::Routed)
        };
        let graph = GraphManagedExecution::compile(formula, input, filter, &outputs, ctx, shared_graph)?;
        let mut runtime = ValuePipelineRuntime {
            input_set: InputSetRuntime::from_managed_region(input, input_instance)?,
            filter_items,
            filter_value_mode,
            typed_stages: None,
            graph,
            value_types: ctx.value_types.clone(),
            nodes: ctx.nodes.clone(),
            properties: ctx.properties.cloned(),
            output_sets,
        };
        runtime.prepare_typed_stages(None)?;
        Ok(Self {
            kind: ManagedFormulaRuntimeKind::ValuePipeline(Box::new(runtime)),
        })
    }

    fn compile_trigger_pipeline(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
    ) -> Result<Self, ManagedFormulaError> {
        let trigger = required_region(&formula.surface.managed_regions, ManagedRegionKind::TriggerInput)?;
        let command_regions = required_regions(&formula.surface.managed_regions, ManagedRegionKind::CommandSet)?;
        let filter = optional_region(&formula.surface.managed_regions, ManagedRegionKind::FilterPipeline)?;

        let trigger_instance = required_region_instance(instance, &trigger.id)?;
        let commands = command_regions
            .into_iter()
            .map(|definition| {
                let region = required_region_instance(instance, &definition.id)?;
                CommandSetRuntime::from_managed_region(definition, region)
            })
            .collect::<Result<Vec<_>, ManagedFormulaError>>()?;
        let filter_instance = filter
            .map(|definition| required_region_instance(instance, &definition.id).map(|region| (definition, region)))
            .transpose()?;

        let trigger = TriggerInputRuntime::from_managed_region(trigger, trigger_instance)?;
        let layout = trigger.layout();
        let typed_stages = layout
            .as_ref()
            .map(|layout| {
                ManagedStageChain::compile(
                    filter_instance.map_or(&[][..], |(_, region)| region.items.as_slice()),
                    Arc::clone(layout),
                    ctx,
                    filter_instance.map_or(ManagedFilterValueMode::Routed, |(definition, _)| {
                        definition.filter_value_mode
                    }),
                )
                .map_err(ManagedFormulaError::from)
            })
            .transpose()?;
        if let Some(stages) = &typed_stages {
            require_single_trigger_output(stages.output_layout())?;
        }
        Ok(Self {
            kind: ManagedFormulaRuntimeKind::TriggerPipeline(Box::new(TriggerPipelineRuntime {
                trigger,
                typed_stages,
                input_frame: layout.map(ChannelFrame::new),
                commands,
            })),
        })
    }

    pub fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        self.evaluate_with_graph_frame(ctx, None, DebugCaptureMode::Off)
    }

    pub fn evaluate_with_graph_frame(
        &mut self,
        ctx: &EvaluationCtx<'_>,
        properties: Option<&RuntimePropertyFrame>,
        capture_mode: DebugCaptureMode,
    ) -> RuntimeOutput {
        self.evaluate_with_context_frame(ctx, &ContextKey::default_lane(), properties, capture_mode)
    }

    pub fn evaluate_with_context_frame(
        &mut self,
        ctx: &EvaluationCtx<'_>,
        context_key: &ContextKey,
        properties: Option<&RuntimePropertyFrame>,
        capture_mode: DebugCaptureMode,
    ) -> RuntimeOutput {
        match &mut self.kind {
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => {
                runtime.evaluate(ctx, context_key, properties, capture_mode)
            }
            ManagedFormulaRuntimeKind::TriggerPipeline(runtime) => runtime.evaluate(ctx, context_key),
        }
    }

    #[must_use]
    pub fn uses_authored_graph(&self) -> bool {
        matches!(&self.kind, ManagedFormulaRuntimeKind::ValuePipeline(runtime) if runtime.graph.is_some())
    }

    #[must_use]
    pub fn needs_continuous_evaluation(&self) -> bool {
        match &self.kind {
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => runtime
                .typed_stages
                .as_ref()
                .is_some_and(ManagedStageChain::needs_continuous_evaluation),
            ManagedFormulaRuntimeKind::TriggerPipeline(runtime) => runtime
                .typed_stages
                .as_ref()
                .is_some_and(ManagedStageChain::needs_continuous_evaluation),
        }
    }

    #[must_use]
    pub fn input_layout(&self) -> Option<&ChannelLayout> {
        match &self.kind {
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => Some(runtime.input_set.layout()),
            ManagedFormulaRuntimeKind::TriggerPipeline(_) => None,
        }
    }

    #[must_use]
    pub fn input_value_shape(&self) -> Option<chataigne_alchemist::MappingValueShape> {
        match &self.kind {
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => Some(runtime.input_set.value_shape()),
            ManagedFormulaRuntimeKind::TriggerPipeline(_) => None,
        }
    }

    #[must_use]
    pub fn filter_output_layout(&self) -> Option<&ChannelLayout> {
        match &self.kind {
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => runtime
                .typed_stages
                .as_ref()
                .map(|stages| stages.output_layout().as_ref()),
            ManagedFormulaRuntimeKind::TriggerPipeline(_) => None,
        }
    }

    #[must_use]
    pub fn filter_output_value_shape(&self) -> Option<chataigne_alchemist::MappingValueShape> {
        self.filter_output_layout().map(ChannelLayout::mapping_value_shape)
    }

    pub fn reconcile_input_source_schema(
        &mut self,
        resolve: impl FnMut(&StableRef) -> Option<ChannelSourceSchema>,
    ) -> Result<(), ManagedFormulaError> {
        self.reconcile_input_source_schema_with_cache(resolve, None)
    }

    pub fn reconcile_input_source_schema_with_cache(
        &mut self,
        resolve: impl FnMut(&StableRef) -> Option<ChannelSourceSchema>,
        cache: Option<&mut ManagedStageSpecializationCache>,
    ) -> Result<(), ManagedFormulaError> {
        if let ManagedFormulaRuntimeKind::ValuePipeline(runtime) = &mut self.kind {
            runtime.input_set.reconcile_source_schema(resolve)?;
            runtime.prepare_typed_stages(cache)?;
        }
        Ok(())
    }

    pub fn update_filter_input(
        &mut self,
        item: chataigne_alchemist::ManagedItemId,
        socket: &chataigne_alchemist::SocketId,
        binding: RuntimeInputBinding,
    ) -> Result<(), ManagedFormulaError> {
        match &mut self.kind {
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => runtime.update_runtime_input(item, socket, binding),
            ManagedFormulaRuntimeKind::TriggerPipeline(runtime) => runtime
                .typed_stages
                .as_mut()
                .ok_or(ManagedFormulaError::UnresolvedManagedInputSchema)?
                .update_runtime_input(item, socket, binding)
                .map_err(ManagedFormulaError::from),
        }
    }

    pub fn reset_memory(&mut self) {
        match &mut self.kind {
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => {
                if let Some(stages) = runtime.typed_stages.as_mut() {
                    stages.reset_memory();
                }
                if let Some(graph) = runtime.graph.as_mut() {
                    graph.reset_memory();
                }
            }
            ManagedFormulaRuntimeKind::TriggerPipeline(runtime) => {
                if let Some(stages) = runtime.typed_stages.as_mut() {
                    stages.reset_memory();
                }
            }
        }
    }

    pub fn retain_context_keys(&mut self, active: &IndexSet<ContextKey>) {
        match &mut self.kind {
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => {
                if let Some(stages) = runtime.typed_stages.as_mut() {
                    stages.retain_context_keys(active);
                }
                if let Some(graph) = runtime.graph.as_mut() {
                    graph.retain_context_keys(active);
                }
            }
            ManagedFormulaRuntimeKind::TriggerPipeline(runtime) => {
                if let Some(stages) = runtime.typed_stages.as_mut() {
                    stages.retain_context_keys(active);
                }
            }
        }
    }

    pub fn migrate_memory_from(&mut self, previous: Self) {
        match (&mut self.kind, previous.kind) {
            (ManagedFormulaRuntimeKind::ValuePipeline(current), ManagedFormulaRuntimeKind::ValuePipeline(old)) => {
                if let (Some(current), Some(old)) = (current.typed_stages.as_mut(), old.typed_stages) {
                    current.migrate_memory_from(old);
                }
                if let (Some(current), Some(old)) = (current.graph.as_mut(), old.graph) {
                    current.migrate_memory_from(old);
                }
            }
            (ManagedFormulaRuntimeKind::TriggerPipeline(current), ManagedFormulaRuntimeKind::TriggerPipeline(old)) => {
                if let (Some(current), Some(old)) = (current.typed_stages.as_mut(), old.typed_stages) {
                    current.migrate_memory_from(old);
                }
            }
            _ => {}
        }
    }
}

impl ValuePipelineRuntime {
    fn prepare_typed_stages(
        &mut self,
        cache: Option<&mut ManagedStageSpecializationCache>,
    ) -> Result<(), ManagedFormulaError> {
        let unresolved = self
            .input_set
            .layout()
            .channels()
            .iter()
            .any(|channel| channel.value_type.is_none());
        if unresolved && self.filter_items.iter().any(|item| item.enabled && item.anode.enabled) {
            self.typed_stages = None;
            return Ok(());
        }
        let ctx = CompileCtx {
            value_types: &self.value_types,
            nodes: &self.nodes,
            properties: self.properties.as_ref(),
        };
        let mut compiled = match ManagedStageChain::compile_with_cache(
            &self.filter_items,
            self.input_set.layout().clone(),
            &ctx,
            self.filter_value_mode,
            cache,
        ) {
            Ok(compiled) => compiled,
            Err(error) => {
                self.typed_stages = None;
                return Err(error.into());
            }
        };
        if let Some(previous) = self.typed_stages.take() {
            compiled.migrate_memory_from(previous);
        }
        self.typed_stages = Some(compiled);
        Ok(())
    }

    fn update_runtime_input(
        &mut self,
        item: chataigne_alchemist::ManagedItemId,
        socket: &chataigne_alchemist::SocketId,
        binding: RuntimeInputBinding,
    ) -> Result<(), ManagedFormulaError> {
        let chain = self
            .typed_stages
            .as_mut()
            .ok_or(ManagedFormulaError::UnresolvedManagedInputSchema)?;
        chain.update_runtime_input(item, socket, binding.clone())?;
        let authored = self
            .filter_items
            .iter_mut()
            .find(|candidate| candidate.id == item)
            .ok_or(ManagedFormulaError::MissingFilterItem(item))?;
        let value = match binding {
            RuntimeInputBinding::Constant(value) => value,
            RuntimeInputBinding::Reference(reference) => RuntimeValue::Ref(reference),
        };
        authored.anode.input_defaults.insert(socket.clone(), value);
        Ok(())
    }

    fn evaluate(
        &mut self,
        ctx: &EvaluationCtx<'_>,
        context_key: &ContextKey,
        properties: Option<&RuntimePropertyFrame>,
        capture_mode: DebugCaptureMode,
    ) -> RuntimeOutput {
        let input = self.input_set.materialize_for_context(ctx, context_key);
        let mut output = RuntimeOutput::default();
        output
            .diagnostics
            .extend(input.diagnostics.into_iter().map(runtime_diagnostic));
        if !output.diagnostics.is_empty() {
            if let Some(stages) = self.typed_stages.as_mut() {
                stages.suspend_context(context_key);
            }
            return output;
        }

        let Some(stages) = self.typed_stages.as_mut() else {
            return runtime_error_output(ManagedFormulaError::UnresolvedManagedInputSchema);
        };
        if let Some(graph) = self.graph.as_mut() {
            merge_runtime_output(
                &mut output,
                graph.evaluate(
                    input.frame,
                    stages,
                    &self.output_sets,
                    GraphManagedFrame {
                        ctx,
                        context_key,
                        properties,
                        capture_mode,
                    },
                ),
            );
            return output;
        }
        let (frame, stage_output) =
            match stages.evaluate_with_capture_for_context(input.frame, ctx, capture_mode, context_key) {
                Ok(result) => result,
                Err(error) => return runtime_error_output(error.into()),
            };
        merge_runtime_output(&mut output, stage_output);
        if !output.diagnostics.is_empty() {
            return output;
        }
        if frame
            .slots()
            .iter()
            .any(|slot| slot.validity == ChannelValidity::Suppressed)
        {
            return output;
        }
        let values = match frame_values(frame) {
            Ok(values) => values,
            Err(error) => return runtime_error_output(error),
        };
        for output_set in &self.output_sets {
            merge_output_set(&mut output, output_set.materialize_values(&values, ctx));
        }
        output
    }
}

fn frame_values(frame: &ChannelFrame) -> Result<ValueSet, ManagedFormulaError> {
    let mut values = ValueSet::new(frame.logical_tick());
    for (descriptor, slot) in frame.layout().channels().iter().zip(frame.slots()) {
        if slot.validity != ChannelValidity::Valid || slot.value.is_none() {
            return Err(ManagedFormulaError::InvalidStageChannel(descriptor.id.clone()));
        }
        let mut entry = ValueSetEntry::new(
            descriptor.id.clone(),
            descriptor.label.clone(),
            slot.value.clone().expect("validated channel slot"),
        );
        if let ChannelProvenance::Input(source) = &descriptor.provenance {
            entry = entry.with_source(source.clone());
        }
        values.push(entry);
    }
    Ok(values)
}

impl TriggerPipelineRuntime {
    fn evaluate(&mut self, ctx: &EvaluationCtx<'_>, context_key: &ContextKey) -> RuntimeOutput {
        let trigger = self.trigger.materialize(ctx, context_key);
        let mut output = RuntimeOutput::default();
        output
            .diagnostics
            .extend(trigger.diagnostics.into_iter().map(runtime_diagnostic));
        let Some(value) = trigger.value else {
            return output;
        };
        if !output.diagnostics.is_empty() {
            return output;
        }

        let Some(input_frame) = self.input_frame.as_mut() else {
            return output;
        };
        input_frame.begin_tick(ctx.logical_tick);
        if let Err(error) = input_frame.set(0, Some(value), ChannelValidity::Valid, true) {
            return runtime_error_output(ManagedFormulaError::ManagedStage(crate::ManagedStageError::Frame(
                error,
            )));
        }
        let Some(stages) = self.typed_stages.as_mut() else {
            return output;
        };
        let (frame, effects) =
            match stages.evaluate_with_capture_for_context(input_frame, ctx, DebugCaptureMode::Off, context_key) {
                Ok(result) => result,
                Err(error) => return runtime_error_output(error.into()),
            };
        merge_runtime_output(&mut output, effects);
        if !output.diagnostics.is_empty() {
            return output;
        }
        let Some(slot) = frame.slots().first() else {
            return output;
        };
        if slot.validity != ChannelValidity::Valid || !slot.deliver {
            return output;
        }
        let Some(value) = slot.value.as_ref() else {
            return output;
        };
        for commands in &self.commands {
            merge_runtime_output(&mut output, commands.materialize(value, ctx));
        }
        output
    }
}

fn is_value_pipeline_region(definition: &ManagedRegionDefinition) -> bool {
    matches!(
        definition.kind,
        ManagedRegionKind::InputSet | ManagedRegionKind::OutputSet
    )
}

fn is_trigger_pipeline_region(definition: &ManagedRegionDefinition) -> bool {
    matches!(
        definition.kind,
        ManagedRegionKind::TriggerInput | ManagedRegionKind::CommandSet
    )
}

fn required_region(
    definitions: &[ManagedRegionDefinition],
    kind: ManagedRegionKind,
) -> Result<&ManagedRegionDefinition, ManagedFormulaError> {
    optional_region(definitions, kind)?.ok_or(ManagedFormulaError::MissingRegion { kind })
}

fn required_regions(
    definitions: &[ManagedRegionDefinition],
    kind: ManagedRegionKind,
) -> Result<Vec<&ManagedRegionDefinition>, ManagedFormulaError> {
    let matching = definitions
        .iter()
        .filter(|definition| definition.kind == kind)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Err(ManagedFormulaError::MissingRegion { kind });
    }
    Ok(matching)
}

fn optional_region(
    definitions: &[ManagedRegionDefinition],
    kind: ManagedRegionKind,
) -> Result<Option<&ManagedRegionDefinition>, ManagedFormulaError> {
    let mut matching = definitions.iter().filter(|definition| definition.kind == kind);
    let Some(first) = matching.next() else {
        return Ok(None);
    };
    if matching.next().is_some() {
        return Err(ManagedFormulaError::DuplicateRegion { kind });
    }
    Ok(Some(first))
}

fn required_region_instance<'a>(
    instance: &'a AlchemistFormulaInstance,
    region_id: &ManagedRegionId,
) -> Result<&'a ManagedRegionInstance, ManagedFormulaError> {
    instance
        .managed_regions
        .regions
        .get(region_id)
        .ok_or_else(|| ManagedFormulaError::MissingRegionInstance {
            region_id: region_id.clone(),
        })
}

struct TriggerInputRuntime {
    items: Vec<TriggerInputItem>,
}

struct TriggerInputItem {
    id: chataigne_alchemist::ManagedItemId,
    label: String,
    source: StableRef,
    enabled: bool,
}

struct TriggerInputMaterialization {
    value: Option<RuntimeValue>,
    diagnostics: Vec<Diagnostic>,
}

impl TriggerInputRuntime {
    fn from_managed_region(
        definition: &ManagedRegionDefinition,
        instance: &ManagedRegionInstance,
    ) -> Result<Self, ManagedFormulaError> {
        if definition.kind != ManagedRegionKind::TriggerInput {
            return Err(ManagedFormulaError::WrongTriggerInputRegionKind {
                region_id: definition.id.clone(),
                actual: definition.kind,
            });
        }
        if definition.id != instance.region_id {
            return Err(ManagedFormulaError::RegionMismatch {
                definition_id: definition.id.clone(),
                instance_id: instance.region_id.clone(),
            });
        }
        if !definition.accepted_roles.contains(&SurfaceItemKind::Input) {
            return Err(ManagedFormulaError::DoesNotAcceptTriggerInputs {
                region_id: definition.id.clone(),
            });
        }

        let items = instance
            .items
            .iter()
            .map(|item| {
                let source = match item.anode.config.get(INPUT_SOURCE_FIELD) {
                    Some(RuntimeValue::Ref(source)) => source.clone(),
                    Some(value) => {
                        return Err(ManagedFormulaError::InvalidTriggerInputSourceConfig {
                            label: item.anode.label.clone(),
                            actual: value.value_type().to_string(),
                        });
                    }
                    None => {
                        return Err(ManagedFormulaError::MissingTriggerInputSourceConfig {
                            label: item.anode.label.clone(),
                        });
                    }
                };
                Ok(TriggerInputItem {
                    id: item.id,
                    label: item.anode.label.clone(),
                    source,
                    enabled: item.enabled && item.anode.enabled,
                })
            })
            .collect::<Result<Vec<_>, ManagedFormulaError>>()?;

        Ok(Self { items })
    }

    fn layout(&self) -> Option<Arc<ChannelLayout>> {
        let mut enabled = self.items.iter().filter(|item| item.enabled);
        let item = enabled.next()?;
        if enabled.next().is_some() {
            return None;
        }
        let descriptor = chataigne_alchemist::ChannelDescriptor::input(
            ValueLaneKey::input(item.id),
            item.label.clone(),
            item.source.clone(),
            Some(ValueTypeId::new("trigger")),
        );
        Some(Arc::new(
            ChannelLayout::new(vec![descriptor]).expect("one trigger input has a unique identity"),
        ))
    }

    fn materialize(&self, ctx: &EvaluationCtx<'_>, context_key: &ContextKey) -> TriggerInputMaterialization {
        let enabled = self.items.iter().filter(|item| item.enabled).collect::<Vec<_>>();
        if enabled.is_empty() {
            return TriggerInputMaterialization {
                value: None,
                diagnostics: Vec::new(),
            };
        }
        if enabled.len() != 1 {
            return TriggerInputMaterialization {
                value: None,
                diagnostics: vec![Diagnostic::error(
                    "trigger_input_requires_single_enabled_source",
                    format!(
                        "TriggerInput expected one enabled trigger source, got {}.",
                        enabled.len()
                    ),
                    DiagnosticOrigin::Runtime,
                )],
            };
        }

        let item = enabled[0];
        match ctx
            .inputs
            .get_context(&item.source, context_key)
            .or_else(|| ctx.inputs.get(&item.source))
        {
            Some(RuntimeValue::Trigger(trigger)) => TriggerInputMaterialization {
                value: Some(RuntimeValue::Trigger(*trigger)),
                diagnostics: Vec::new(),
            },
            Some(value) => TriggerInputMaterialization {
                value: None,
                diagnostics: vec![Diagnostic::error(
                    "trigger_input_expected_trigger",
                    format!(
                        "Trigger input `{}` resolved `{}` from `{}`; expected `trigger`.",
                        item.label,
                        value.value_type(),
                        item.source.stable_id
                    ),
                    DiagnosticOrigin::Runtime,
                )],
            },
            None => TriggerInputMaterialization {
                value: None,
                diagnostics: vec![Diagnostic::error(
                    "trigger_input_missing_source",
                    format!(
                        "Trigger input `{}` could not resolve source `{}`.",
                        item.label, item.source.stable_id
                    ),
                    DiagnosticOrigin::Runtime,
                )],
            },
        }
    }
}

struct CommandSetRuntime {
    items: Vec<CommandItem>,
}

struct CommandItem {
    target: StableRef,
    enabled: bool,
    source_node: Option<ANodeId>,
}

impl CommandSetRuntime {
    fn from_managed_region(
        definition: &ManagedRegionDefinition,
        instance: &ManagedRegionInstance,
    ) -> Result<Self, ManagedFormulaError> {
        if definition.kind != ManagedRegionKind::CommandSet {
            return Err(ManagedFormulaError::WrongCommandSetRegionKind {
                region_id: definition.id.clone(),
                actual: definition.kind,
            });
        }
        if definition.id != instance.region_id {
            return Err(ManagedFormulaError::RegionMismatch {
                definition_id: definition.id.clone(),
                instance_id: instance.region_id.clone(),
            });
        }
        if !definition.accepted_roles.contains(&SurfaceItemKind::Command) {
            return Err(ManagedFormulaError::DoesNotAcceptCommands {
                region_id: definition.id.clone(),
            });
        }

        let items = instance
            .items
            .iter()
            .map(|item| {
                let target = match item.anode.config.get(OUTPUT_TARGET_FIELD) {
                    Some(RuntimeValue::Ref(target)) => target.clone(),
                    Some(value) => {
                        return Err(ManagedFormulaError::InvalidCommandTargetConfig {
                            label: item.anode.label.clone(),
                            actual: value.value_type().to_string(),
                        });
                    }
                    None => {
                        return Err(ManagedFormulaError::MissingCommandTargetConfig {
                            label: item.anode.label.clone(),
                        });
                    }
                };
                Ok(CommandItem {
                    target,
                    enabled: item.enabled && item.anode.enabled,
                    source_node: Some(item.anode.id),
                })
            })
            .collect::<Result<Vec<_>, ManagedFormulaError>>()?;

        Ok(Self { items })
    }

    fn materialize(&self, value: &RuntimeValue, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        if !should_emit(value) {
            return RuntimeOutput::default();
        }
        RuntimeOutput {
            intents: self
                .items
                .iter()
                .filter(|item| item.enabled)
                .map(|item| RuntimeIntent {
                    kind: COMMAND_INTENT_KIND.into(),
                    source_node: item.source_node,
                    source_socket: None,
                    target: Some(item.target.clone()),
                    payload: value.clone(),
                    logical_tick: ctx.logical_tick,
                })
                .collect(),
            ..RuntimeOutput::default()
        }
    }
}

fn validate_filter_region(
    definition: &ManagedRegionDefinition,
    instance: &ManagedRegionInstance,
) -> Result<(), ManagedFormulaError> {
    if definition.kind != ManagedRegionKind::FilterPipeline {
        return Err(ManagedFormulaError::WrongFilterRegionKind {
            region_id: definition.id.clone(),
            actual: definition.kind,
        });
    }
    if definition.id != instance.region_id {
        return Err(ManagedFormulaError::RegionMismatch {
            definition_id: definition.id.clone(),
            instance_id: instance.region_id.clone(),
        });
    }
    if !definition.accepted_roles.contains(&SurfaceItemKind::Filter) {
        return Err(ManagedFormulaError::DoesNotAcceptFilters {
            region_id: definition.id.clone(),
        });
    }
    Ok(())
}

fn merge_output_set(target: &mut RuntimeOutput, materialized: OutputSetMaterialization) {
    target.intents.extend(materialized.output.intents);
    target.diagnostics.extend(materialized.output.diagnostics);
    target.debug_samples.extend(materialized.output.debug_samples);
    target
        .diagnostics
        .extend(materialized.diagnostics.into_iter().map(runtime_diagnostic));
}

fn merge_runtime_output(target: &mut RuntimeOutput, output: RuntimeOutput) {
    target.intents.extend(output.intents);
    target.diagnostics.extend(output.diagnostics);
    target.debug_samples.extend(output.debug_samples);
}

fn should_emit(value: &RuntimeValue) -> bool {
    !matches!(value, RuntimeValue::Trigger(trigger) if !trigger.fired)
}

fn runtime_error_output(error: ManagedFormulaError) -> RuntimeOutput {
    RuntimeOutput {
        diagnostics: vec![runtime_error(error.diagnostic_code(), error)],
        ..RuntimeOutput::default()
    }
}

fn runtime_error(code: &'static str, error: impl ToString) -> RuntimeDiagnostic {
    RuntimeDiagnostic {
        exec_node: ExecNodeId::new(0),
        message: format!("{code}: {}", error.to_string()),
    }
}

fn runtime_diagnostic(diagnostic: Diagnostic) -> RuntimeDiagnostic {
    RuntimeDiagnostic {
        exec_node: ExecNodeId::new(0),
        message: diagnostic.message,
    }
}
