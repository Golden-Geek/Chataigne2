use chataigne_alchemist::{
    ANodeId, ANodeRegistry, AlchemistFormula, AlchemistFormulaInstance, ChannelLayout, CompileCtx, Diagnostic,
    DiagnosticOrigin, EvaluationCtx, ExecNodeId, ManagedItemInstance, ManagedRegionDefinition, ManagedRegionId,
    ManagedRegionInstance, ManagedRegionKind, PipelineCardinality, PipelineLoweringCtx, PipelineShape,
    PipelineShapeCheckItem, RuntimeDiagnostic, RuntimeIntent, RuntimeOutput, SignatureCtx, StableRef, SurfaceItemKind,
    ValueTypeId, ValueTypeRegistry, check_filter_pipeline_shapes, value_set_shape,
};
use golden_values::Value as RuntimeValue;

use crate::{
    COMMAND_INTENT_KIND, ChannelSourceSchema, INPUT_SOURCE_FIELD, InputSetRuntime, OUTPUT_TARGET_FIELD,
    OutputSetMaterialization, OutputSetRuntime, RuntimeInputBinding, ValueLaneKey, ValueSet, ValueSetEntry,
    ValueSetPipelineRuntime, ValueSetProjectionRuntime,
};

mod availability;
mod error;

pub use availability::{
    ExecutableFilterApplication, ManagedFilterAvailabilityError, executable_filter_applications,
    validate_executable_filter_application,
};
pub use error::ManagedFormulaError;

pub struct ManagedFormulaRuntime {
    kind: ManagedFormulaRuntimeKind,
}

enum ManagedFormulaRuntimeKind {
    ValuePipeline(ValuePipelineRuntime),
    TriggerPipeline(TriggerPipelineRuntime),
}

struct ValuePipelineRuntime {
    input_set: InputSetRuntime,
    filter_pipeline: ManagedFilterPipelineRuntime,
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

        Ok(Self {
            kind: ManagedFormulaRuntimeKind::ValuePipeline(ValuePipelineRuntime {
                input_set: InputSetRuntime::from_managed_region(input, input_instance)?,
                filter_pipeline: ManagedFilterPipelineRuntime::new(filter_instance, ctx)?,
                output_sets,
            }),
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
            kind: ManagedFormulaRuntimeKind::TriggerPipeline(TriggerPipelineRuntime {
                trigger: TriggerInputRuntime::from_managed_region(trigger, trigger_instance)?,
                filter_pipeline: ManagedFilterPipelineRuntime::new(filter_instance, ctx)?,
                commands,
            }),
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

    pub fn reconcile_input_source_schema(
        &mut self,
        resolve: impl FnMut(&StableRef) -> Option<ChannelSourceSchema>,
    ) -> Result<(), ManagedFormulaError> {
        if let ManagedFormulaRuntimeKind::ValuePipeline(runtime) = &mut self.kind {
            runtime.input_set.reconcile_source_schema(resolve)?;
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
            ManagedFormulaRuntimeKind::ValuePipeline(runtime) => {
                runtime.filter_pipeline.update_runtime_input(item, socket, binding)
            }
            ManagedFormulaRuntimeKind::TriggerPipeline(runtime) => {
                runtime.filter_pipeline.update_runtime_input(item, socket, binding)
            }
        }
    }
}

impl ValuePipelineRuntime {
    fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        let input = self.input_set.materialize(ctx);
        let mut output = RuntimeOutput::default();
        output
            .diagnostics
            .extend(input.diagnostics.into_iter().map(runtime_diagnostic));
        if !output.diagnostics.is_empty() {
            return output;
        }

        let filtered = match self.filter_pipeline.evaluate(input.value_set, ctx) {
            Ok(filtered) => filtered,
            Err(error) => return runtime_error_output(error),
        };

        match filtered {
            ManagedFilterOutput::ValueSet(values) => {
                for output_set in &self.output_sets {
                    merge_output_set(&mut output, output_set.materialize_values(&values, ctx));
                }
            }
            ManagedFilterOutput::Single(value) => {
                for output_set in &self.output_sets {
                    merge_output_set(&mut output, output_set.materialize(&value, ctx));
                }
            }
        }
        output
    }
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

struct ManagedFilterPipelineRuntime {
    definition: Option<ManagedRegionDefinition>,
    instance: Option<ManagedRegionInstance>,
    value_types: ValueTypeRegistry,
    nodes: ANodeRegistry,
    compiled_key: Option<ManagedFilterCompileKey>,
    compiled: ManagedFilterCompiledRuntime,
}

impl ManagedFilterPipelineRuntime {
    fn update_runtime_input(
        &mut self,
        item: chataigne_alchemist::ManagedItemId,
        socket: &chataigne_alchemist::SocketId,
        binding: RuntimeInputBinding,
    ) -> Result<(), ManagedFormulaError> {
        if !self
            .instance
            .as_ref()
            .is_some_and(|instance| instance.items.iter().any(|candidate| candidate.id == item))
        {
            return Err(ManagedFormulaError::MissingFilterItem(item));
        }
        match &mut self.compiled {
            ManagedFilterCompiledRuntime::Elementwise(runtime) => {
                runtime.update_runtime_input(item, socket, binding.clone())?;
            }
            ManagedFilterCompiledRuntime::Projection {
                prefix: Some(prefix), ..
            } => {
                prefix.update_runtime_input(item, socket, binding.clone())?;
            }
            ManagedFilterCompiledRuntime::PassThrough => {}
            ManagedFilterCompiledRuntime::Projection { prefix: None, .. } => {
                return Err(ManagedFormulaError::UnsupportedFilterPipeline(
                    "runtime edits to projection input bindings are not supported yet".into(),
                ));
            }
        }
        let instance = self
            .instance
            .as_mut()
            .ok_or(ManagedFormulaError::MissingFilterItem(item))?;
        let target = instance
            .items
            .iter_mut()
            .find(|candidate| candidate.id == item)
            .ok_or(ManagedFormulaError::MissingFilterItem(item))?;
        let value = match binding {
            RuntimeInputBinding::Constant(value) => value,
            RuntimeInputBinding::Reference(reference) => RuntimeValue::Ref(reference),
        };
        target.anode.input_defaults.insert(socket.clone(), value);
        Ok(())
    }

    fn new(
        filter: Option<(&ManagedRegionDefinition, &ManagedRegionInstance)>,
        ctx: &CompileCtx<'_>,
    ) -> Result<Self, ManagedFormulaError> {
        if let Some((definition, instance)) = filter {
            validate_filter_region(definition, instance)?;
            Ok(Self {
                definition: Some(definition.clone()),
                instance: Some(instance.clone()),
                value_types: ctx.value_types.clone(),
                nodes: ctx.nodes.clone(),
                compiled_key: None,
                compiled: ManagedFilterCompiledRuntime::PassThrough,
            })
        } else {
            Ok(Self {
                definition: None,
                instance: None,
                value_types: ctx.value_types.clone(),
                nodes: ctx.nodes.clone(),
                compiled_key: None,
                compiled: ManagedFilterCompiledRuntime::PassThrough,
            })
        }
    }

    fn evaluate(
        &mut self,
        values: ValueSet,
        ctx: &EvaluationCtx<'_>,
    ) -> Result<ManagedFilterOutput, ManagedFormulaError> {
        if self.definition.is_none() {
            return Ok(ManagedFilterOutput::ValueSet(values));
        }

        let enabled_items = self.enabled_items();
        if enabled_items.is_empty() {
            return Ok(ManagedFilterOutput::ValueSet(values));
        }

        let key = ManagedFilterCompileKey {
            item_type: value_set_item_type(&values)?,
            lane_count: values.entries.len(),
        };
        if self.compiled_key.as_ref() != Some(&key) {
            self.compiled = self.compile_for_key(&enabled_items, &key)?;
            self.compiled_key = Some(key);
        }
        self.compiled.evaluate(values, ctx)
    }

    fn evaluate_single(
        &mut self,
        value: RuntimeValue,
        ctx: &EvaluationCtx<'_>,
    ) -> Result<RuntimeValue, ManagedFormulaError> {
        let values = ValueSet::with_entries(
            ctx.logical_tick,
            vec![ValueSetEntry::new(
                ValueLaneKey::new("trigger").expect("static trigger channel identity is non-empty"),
                "Trigger",
                value,
            )],
        );
        match self.evaluate(values, ctx)? {
            ManagedFilterOutput::ValueSet(values) => {
                let actual = values.entries.len();
                let mut entries = values.entries.into_iter();
                let Some(entry) = entries.next() else {
                    return Err(ManagedFormulaError::TriggerFilterExpectedSingleValue { actual: 0 });
                };
                if entries.next().is_some() {
                    return Err(ManagedFormulaError::TriggerFilterExpectedSingleValue { actual });
                }
                Ok(entry.value)
            }
            ManagedFilterOutput::Single(value) => Ok(value),
        }
    }

    fn enabled_items(&self) -> Vec<ManagedItemInstance> {
        self.instance
            .as_ref()
            .map(|instance| {
                instance
                    .items
                    .iter()
                    .filter(|item| item.enabled && item.anode.enabled)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn compile_for_key(
        &self,
        items: &[ManagedItemInstance],
        key: &ManagedFilterCompileKey,
    ) -> Result<ManagedFilterCompiledRuntime, ManagedFormulaError> {
        if items.is_empty() {
            return Ok(ManagedFilterCompiledRuntime::PassThrough);
        }
        if key.lane_count == 0 {
            return Err(ManagedFormulaError::EmptyFilteredValueSet);
        }

        let mut check_items = Vec::with_capacity(items.len());
        for item in items {
            let declaration =
                self.nodes
                    .get(&item.anode.type_id)
                    .ok_or_else(|| ManagedFormulaError::MissingFilterDeclaration {
                        node_type: item.anode.type_id.clone(),
                    })?;
            check_items.push(PipelineShapeCheckItem {
                declaration: declaration.as_ref(),
                instance: &item.anode,
            });
        }

        let signature_ctx = SignatureCtx {
            value_types: &self.value_types,
            properties: None,
        };
        let shape = check_filter_pipeline_shapes(
            value_set_shape(key.item_type.clone(), None),
            check_items,
            &signature_ctx,
        );
        if !shape.is_valid() {
            return Err(ManagedFormulaError::InvalidFilterShape {
                messages: shape
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| diagnostic.message)
                    .collect(),
            });
        }

        let lowering_ctx = PipelineLoweringCtx {
            value_types: &self.value_types,
            nodes: &self.nodes,
            properties: None,
        };
        let projection_index = shape.steps.iter().position(|step| {
            matches!(
                step.cardinality,
                PipelineCardinality::Aggregate | PipelineCardinality::Reshape | PipelineCardinality::Expand
            )
        });

        let Some(projection_index) = projection_index else {
            let runtime =
                ValueSetPipelineRuntime::compile_elementwise(items.to_vec(), key.item_type.clone(), &lowering_ctx)?;
            return Ok(ManagedFilterCompiledRuntime::Elementwise(runtime));
        };

        if projection_index + 1 != shape.steps.len() {
            return Err(ManagedFormulaError::UnsupportedFilterPipeline(
                "aggregate, reshape, and expand filters must be the final ValueSet filter".into(),
            ));
        }
        if shape.steps[..projection_index].iter().any(|step| {
            !matches!(
                step.cardinality,
                PipelineCardinality::Elementwise | PipelineCardinality::WholeSet
            )
        }) {
            return Err(ManagedFormulaError::UnsupportedFilterPipeline(
                "only elementwise or gate filters may run before a projection filter".into(),
            ));
        }

        let prefix = if projection_index == 0 {
            None
        } else {
            Some(ValueSetPipelineRuntime::compile_elementwise(
                items[..projection_index].to_vec(),
                key.item_type.clone(),
                &lowering_ctx,
            )?)
        };
        let projection_item = items[projection_index].clone();
        let projection = match shape.steps[projection_index].cardinality {
            PipelineCardinality::Aggregate => ValueSetProjectionRuntime::compile_aggregate(
                projection_item,
                key.lane_count,
                key.item_type.clone(),
                &lowering_ctx,
            )?,
            PipelineCardinality::Reshape => match &shape.final_shape {
                PipelineShape::Single { value_type } if *value_type == ValueTypeId::new("vec3") => {
                    ValueSetProjectionRuntime::compile_pack_vec3(projection_item, &lowering_ctx)?
                }
                PipelineShape::Single { value_type } => {
                    return Err(ManagedFormulaError::UnsupportedFilterPipeline(format!(
                        "unsupported reshape output type `{value_type}`"
                    )));
                }
                _ => {
                    return Err(ManagedFormulaError::UnsupportedFilterPipeline(
                        "reshape filters must produce a single value".into(),
                    ));
                }
            },
            PipelineCardinality::Expand => {
                return Err(ManagedFormulaError::UnsupportedFilterPipeline(
                    "expand filters are not supported by managed ValueSet output yet".into(),
                ));
            }
            PipelineCardinality::Elementwise | PipelineCardinality::WholeSet => {
                unreachable!("projection_index only selects aggregate, reshape, or expand cardinalities")
            }
        };

        Ok(ManagedFilterCompiledRuntime::Projection {
            prefix,
            projection: Box::new(projection),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ManagedFilterCompileKey {
    item_type: ValueTypeId,
    lane_count: usize,
}

enum ManagedFilterCompiledRuntime {
    PassThrough,
    Elementwise(ValueSetPipelineRuntime),
    Projection {
        prefix: Option<ValueSetPipelineRuntime>,
        projection: Box<ValueSetProjectionRuntime>,
    },
}

impl ManagedFilterCompiledRuntime {
    fn evaluate(
        &mut self,
        values: ValueSet,
        ctx: &EvaluationCtx<'_>,
    ) -> Result<ManagedFilterOutput, ManagedFormulaError> {
        match self {
            Self::PassThrough => Ok(ManagedFilterOutput::ValueSet(values)),
            Self::Elementwise(runtime) => {
                let (values, output) = runtime.evaluate(&values, ctx)?;
                ensure_clean_filter_output(output)?;
                Ok(ManagedFilterOutput::ValueSet(values))
            }
            Self::Projection { prefix, projection } => {
                let values = if let Some(prefix) = prefix {
                    let (values, output) = prefix.evaluate(&values, ctx)?;
                    ensure_clean_filter_output(output)?;
                    values
                } else {
                    values
                };
                let (value, output) = projection.evaluate(&values, ctx)?;
                ensure_clean_filter_output(output)?;
                Ok(ManagedFilterOutput::Single(value))
            }
        }
    }
}

enum ManagedFilterOutput {
    ValueSet(ValueSet),
    Single(RuntimeValue),
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

fn value_set_item_type(values: &ValueSet) -> Result<ValueTypeId, ManagedFormulaError> {
    let Some(first) = values.entries.first() else {
        return Err(ManagedFormulaError::EmptyFilteredValueSet);
    };
    let value_type = first.value.value_type();
    if let Some(actual) = values
        .entries
        .iter()
        .skip(1)
        .map(|entry| entry.value.value_type())
        .find(|candidate| *candidate != value_type)
    {
        return Err(ManagedFormulaError::MixedValueSetTypes {
            expected: value_type,
            actual,
        });
    }
    Ok(value_type)
}

fn ensure_clean_filter_output(output: RuntimeOutput) -> Result<(), ManagedFormulaError> {
    if output.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(ManagedFormulaError::FilterDiagnostics {
            messages: output
                .diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect(),
        })
    }
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
