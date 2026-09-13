use chataigne_alchemist::{
    ANodeId, ANodeRegistry, AlchemistFormula, AlchemistFormulaInstance, ChannelLayout, ChannelProvenance, CompileCtx,
    Diagnostic, DiagnosticOrigin, EvaluationCtx, ExecNodeId, FormulaPropertySchema, ManagedItemInstance,
    ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance, ManagedRegionKind, PipelineCardinality,
    PipelineLoweringCtx, PipelineShape, PipelineShapeCheckItem, RuntimeDiagnostic, RuntimeIntent, RuntimeOutput,
    SignatureCtx, StableRef, SurfaceItemKind, ValueTypeId, ValueTypeRegistry, check_filter_pipeline_shapes,
    value_set_shape,
};
use golden_values::Value as RuntimeValue;

use crate::{
    COMMAND_INTENT_KIND, ChannelFrame, ChannelSourceSchema, ChannelValidity, INPUT_SOURCE_FIELD, InputSetRuntime,
    ManagedStageChain, OUTPUT_TARGET_FIELD, OutputSetMaterialization, OutputSetRuntime, RuntimeInputBinding,
    ValueLaneKey, ValueSet, ValueSetEntry, ValueSetPipelineRuntime, ValueSetProjectionRuntime,
};

mod availability;
mod error;
mod legacy_filter;

use legacy_filter::ManagedFilterPipelineRuntime;

pub use availability::{
    ExecutableFilterApplication, ManagedFilterAvailabilityError, executable_filter_applications,
    validate_executable_filter_application,
};
pub use error::ManagedFormulaError;

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
    typed_stages: Option<ManagedStageChain>,
    value_types: ValueTypeRegistry,
    nodes: ANodeRegistry,
    properties: Option<FormulaPropertySchema>,
    output_sets: Vec<OutputSetRuntime>,
}

struct TriggerPipelineRuntime {
    trigger: TriggerInputRuntime,
    filter_pipeline: ManagedFilterPipelineRuntime,
    commands: Vec<CommandSetRuntime>,
}

impl ManagedFormulaRuntime {
    pub fn compile(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
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
            return Self::compile_trigger_pipeline(formula, instance, ctx).map(Some);
        }
        Self::compile_value_pipeline(formula, instance, ctx).map(Some)
    }

    fn compile_value_pipeline(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
    ) -> Result<Self, ManagedFormulaError> {
        let input = required_region(&formula.surface.managed_regions, ManagedRegionKind::InputSet)?;
        let outputs = required_regions(&formula.surface.managed_regions, ManagedRegionKind::OutputSet)?;
        let filter = optional_region(&formula.surface.managed_regions, ManagedRegionKind::FilterPipeline)?;

        let input_instance = required_region_instance(instance, &input.id)?;
        let output_sets = outputs
            .into_iter()
            .map(|definition| {
                let region = required_region_instance(instance, &definition.id)?;
                OutputSetRuntime::from_managed_region(definition, region).map_err(ManagedFormulaError::from)
            })
            .collect::<Result<Vec<_>, ManagedFormulaError>>()?;
        let filter_instance = filter
            .map(|definition| required_region_instance(instance, &definition.id).map(|region| (definition, region)))
            .transpose()?;

        let filter_items = if let Some((definition, region)) = filter_instance {
            validate_filter_region(definition, region)?;
            region.items.clone()
        } else {
            Vec::new()
        };
        let mut runtime = ValuePipelineRuntime {
            input_set: InputSetRuntime::from_managed_region(input, input_instance)?,
            filter_items,
            typed_stages: None,
            value_types: ctx.value_types.clone(),
            nodes: ctx.nodes.clone(),
            properties: ctx.properties.cloned(),
            output_sets,
        };
        runtime.prepare_typed_stages()?;
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

        Ok(Self {
            kind: ManagedFormulaRuntimeKind::TriggerPipeline(Box::new(TriggerPipelineRuntime {
                trigger: TriggerInputRuntime::from_managed_region(trigger, trigger_instance)?,
                filter_pipeline: ManagedFilterPipelineRuntime::new(filter_instance, ctx)?,
                commands,
            })),
        })
    }

    pub fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        match &mut self.kind {
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => runtime.evaluate(ctx),
            ManagedFormulaRuntimeKind::TriggerPipeline(runtime) => runtime.evaluate(ctx),
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
    pub fn filter_output_layout(&self) -> Option<&ChannelLayout> {
        match &self.kind {
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => runtime
                .typed_stages
                .as_ref()
                .map(|stages| stages.output_layout().as_ref()),
            ManagedFormulaRuntimeKind::TriggerPipeline(_) => None,
        }
    }

    pub fn reconcile_input_source_schema(
        &mut self,
        resolve: impl FnMut(&StableRef) -> Option<ChannelSourceSchema>,
    ) -> Result<(), ManagedFormulaError> {
        if let ManagedFormulaRuntimeKind::ValuePipeline(runtime) = &mut self.kind {
            runtime.input_set.reconcile_source_schema(resolve)?;
            runtime.prepare_typed_stages()?;
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
            ManagedFormulaRuntimeKind::TriggerPipeline(runtime) => {
                runtime.filter_pipeline.update_runtime_input(item, socket, binding)
            }
        }
    }
}

impl ValuePipelineRuntime {
    fn prepare_typed_stages(&mut self) -> Result<(), ManagedFormulaError> {
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
        self.typed_stages = Some(ManagedStageChain::compile(
            &self.filter_items,
            self.input_set.layout().clone(),
            &ctx,
        )?);
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

    fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        let input = self.input_set.materialize(ctx);
        let mut output = RuntimeOutput::default();
        output
            .diagnostics
            .extend(input.diagnostics.into_iter().map(runtime_diagnostic));
        if !output.diagnostics.is_empty() {
            return output;
        }

        let Some(stages) = self.typed_stages.as_mut() else {
            return runtime_error_output(ManagedFormulaError::UnresolvedManagedInputSchema);
        };
        let (frame, stage_output) = match stages.evaluate(input.frame, ctx) {
            Ok(result) => result,
            Err(error) => return runtime_error_output(error.into()),
        };
        merge_runtime_output(&mut output, stage_output);
        if !output.diagnostics.is_empty() {
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
    fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        let trigger = self.trigger.materialize(ctx);
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

        let value = match self.filter_pipeline.evaluate_single(value, ctx) {
            Ok(value) => value,
            Err(error) => return runtime_error_output(error),
        };
        for commands in &self.commands {
            merge_runtime_output(&mut output, commands.materialize(&value, ctx));
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
                    label: item.anode.label.clone(),
                    source,
                    enabled: item.enabled && item.anode.enabled,
                })
            })
            .collect::<Result<Vec<_>, ManagedFormulaError>>()?;

        Ok(Self { items })
    }

    fn materialize(&self, ctx: &EvaluationCtx<'_>) -> TriggerInputMaterialization {
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
        match ctx.inputs.get(&item.source) {
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
