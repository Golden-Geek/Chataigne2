use golden_alchemist::{
    ANodeId, ANodeRegistry, AlchemistFormula, AlchemistFormulaInstance, CompileCtx, Diagnostic, DiagnosticOrigin,
    EvaluationCtx, ExecNodeId, FormulaMaterializationError, ManagedItemInstance, ManagedRegionDefinition,
    ManagedRegionId, ManagedRegionInstance, ManagedRegionKind, ManagedRegionValidationError, PipelineCardinality,
    PipelineLoweringCtx, PipelineShape, PipelineShapeCheckItem, RuntimeDiagnostic, RuntimeIntent, RuntimeOutput,
    RuntimeValue, SignatureCtx, StableRef, SurfaceItemKind, ValueTypeId, ValueTypeRegistry,
    check_filter_pipeline_shapes, value_set_shape,
};

use crate::{
    COMMAND_INTENT_KIND, INPUT_SOURCE_FIELD, InputSetError, InputSetRuntime, OUTPUT_TARGET_FIELD, OutputSetError,
    OutputSetMaterialization, OutputSetRuntime, ValueLaneKey, ValueSet, ValueSetEntry, ValueSetError,
    ValueSetPipelineError, ValueSetPipelineRuntime, ValueSetProjectionRuntime,
};

pub struct ManagedFormulaRuntime {
    kind: ManagedFormulaRuntimeKind,
}

enum ManagedFormulaRuntimeKind {
    Mapping(ManagedMappingRuntime),
    Action(ManagedActionRuntime),
}

struct ManagedMappingRuntime {
    input_set: InputSetRuntime,
    filter_pipeline: ManagedFilterPipelineRuntime,
    output_set: OutputSetRuntime,
}

struct ManagedActionRuntime {
    trigger: ActionTriggerRuntime,
    filter_pipeline: ManagedFilterPipelineRuntime,
    commands: ActionCommandsRuntime,
}

impl ManagedFormulaRuntime {
    pub fn compile(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
    ) -> Result<Option<Self>, ManagedFormulaError> {
        let has_mapping_regions = formula.surface.managed_regions.iter().any(is_mapping_region);
        let has_action_regions = formula.surface.managed_regions.iter().any(is_action_region);
        if !has_mapping_regions && !has_action_regions {
            return Ok(None);
        }
        if has_mapping_regions && has_action_regions {
            return Err(ManagedFormulaError::MixedManagedFormulaKinds);
        }
        instance
            .require_compatible(formula)
            .map_err(ManagedFormulaError::Formula)?;
        instance
            .managed_regions
            .validate_against(&formula.surface)
            .map_err(ManagedFormulaError::ManagedRegionValidation)?;

        if has_action_regions {
            return Self::compile_action(formula, instance, ctx).map(Some);
        }
        Self::compile_mapping(formula, instance, ctx).map(Some)
    }

    fn compile_mapping(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
    ) -> Result<Self, ManagedFormulaError> {
        let input = required_region(&formula.surface.managed_regions, ManagedRegionKind::InputSet)?;
        let output = required_region(&formula.surface.managed_regions, ManagedRegionKind::OutputSet)?;
        let filter = optional_region(&formula.surface.managed_regions, ManagedRegionKind::FilterPipeline)?;

        let input_instance = required_region_instance(instance, &input.id)?;
        let output_instance = required_region_instance(instance, &output.id)?;
        let filter_instance = filter
            .map(|definition| required_region_instance(instance, &definition.id).map(|region| (definition, region)))
            .transpose()?;

        Ok(Self {
            kind: ManagedFormulaRuntimeKind::Mapping(ManagedMappingRuntime {
                input_set: InputSetRuntime::from_managed_region(input, input_instance)?,
                filter_pipeline: ManagedFilterPipelineRuntime::new(filter_instance, ctx)?,
                output_set: OutputSetRuntime::from_managed_region(output, output_instance)?,
            }),
        })
    }

    fn compile_action(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
    ) -> Result<Self, ManagedFormulaError> {
        let trigger = required_region(&formula.surface.managed_regions, ManagedRegionKind::ActionTrigger)?;
        let commands = required_region(&formula.surface.managed_regions, ManagedRegionKind::ActionCommands)?;
        let filter = optional_region(&formula.surface.managed_regions, ManagedRegionKind::FilterPipeline)?;

        let trigger_instance = required_region_instance(instance, &trigger.id)?;
        let commands_instance = required_region_instance(instance, &commands.id)?;
        let filter_instance = filter
            .map(|definition| required_region_instance(instance, &definition.id).map(|region| (definition, region)))
            .transpose()?;

        Ok(Self {
            kind: ManagedFormulaRuntimeKind::Action(ManagedActionRuntime {
                trigger: ActionTriggerRuntime::from_managed_region(trigger, trigger_instance)?,
                filter_pipeline: ManagedFilterPipelineRuntime::new(filter_instance, ctx)?,
                commands: ActionCommandsRuntime::from_managed_region(commands, commands_instance)?,
            }),
        })
    }

    pub fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
        match &mut self.kind {
            ManagedFormulaRuntimeKind::Mapping(runtime) => runtime.evaluate(ctx),
            ManagedFormulaRuntimeKind::Action(runtime) => runtime.evaluate(ctx),
        }
    }
}

impl ManagedMappingRuntime {
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
            ManagedFilterOutput::ValueSet(values) => match values.to_runtime_value() {
                Ok(value) => merge_output_set(&mut output, self.output_set.materialize(&value, ctx)),
                Err(error) => output
                    .diagnostics
                    .push(runtime_error("managed_formula_valueset_error", error)),
            },
            ManagedFilterOutput::Single(value) => {
                merge_output_set(&mut output, self.output_set.materialize(&value, ctx));
            }
        }
        output
    }
}

impl ManagedActionRuntime {
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
        merge_runtime_output(&mut output, self.commands.materialize(&value, ctx));
        output
    }
}

fn is_mapping_region(definition: &ManagedRegionDefinition) -> bool {
    matches!(
        definition.kind,
        ManagedRegionKind::InputSet | ManagedRegionKind::OutputSet
    )
}

fn is_action_region(definition: &ManagedRegionDefinition) -> bool {
    matches!(
        definition.kind,
        ManagedRegionKind::ActionTrigger | ManagedRegionKind::ActionCommands
    )
}

fn required_region(
    definitions: &[ManagedRegionDefinition],
    kind: ManagedRegionKind,
) -> Result<&ManagedRegionDefinition, ManagedFormulaError> {
    optional_region(definitions, kind)?.ok_or(ManagedFormulaError::MissingRegion { kind })
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

struct ActionTriggerRuntime {
    items: Vec<ActionTriggerItem>,
}

struct ActionTriggerItem {
    label: String,
    source: StableRef,
    enabled: bool,
}

struct ActionTriggerMaterialization {
    value: Option<RuntimeValue>,
    diagnostics: Vec<Diagnostic>,
}

impl ActionTriggerRuntime {
    fn from_managed_region(
        definition: &ManagedRegionDefinition,
        instance: &ManagedRegionInstance,
    ) -> Result<Self, ManagedFormulaError> {
        if definition.kind != ManagedRegionKind::ActionTrigger {
            return Err(ManagedFormulaError::WrongActionTriggerRegionKind {
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
            return Err(ManagedFormulaError::DoesNotAcceptActionTriggers {
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
                        return Err(ManagedFormulaError::InvalidActionTriggerSourceConfig {
                            label: item.anode.label.clone(),
                            actual: value.value_type().to_string(),
                        });
                    }
                    None => {
                        return Err(ManagedFormulaError::MissingActionTriggerSourceConfig {
                            label: item.anode.label.clone(),
                        });
                    }
                };
                Ok(ActionTriggerItem {
                    label: item.anode.label.clone(),
                    source,
                    enabled: item.enabled && item.anode.enabled,
                })
            })
            .collect::<Result<Vec<_>, ManagedFormulaError>>()?;

        Ok(Self { items })
    }

    fn materialize(&self, ctx: &EvaluationCtx<'_>) -> ActionTriggerMaterialization {
        let enabled = self.items.iter().filter(|item| item.enabled).collect::<Vec<_>>();
        if enabled.is_empty() {
            return ActionTriggerMaterialization {
                value: None,
                diagnostics: Vec::new(),
            };
        }
        if enabled.len() != 1 {
            return ActionTriggerMaterialization {
                value: None,
                diagnostics: vec![Diagnostic::error(
                    "action_trigger_requires_single_enabled_trigger",
                    format!(
                        "ActionTrigger expected one enabled trigger input, got {}.",
                        enabled.len()
                    ),
                    DiagnosticOrigin::Runtime,
                )],
            };
        }

        let item = enabled[0];
        match ctx.inputs.get(&item.source) {
            Some(RuntimeValue::Trigger(trigger)) => ActionTriggerMaterialization {
                value: Some(RuntimeValue::Trigger(*trigger)),
                diagnostics: Vec::new(),
            },
            Some(value) => ActionTriggerMaterialization {
                value: None,
                diagnostics: vec![Diagnostic::error(
                    "action_trigger_expected_trigger",
                    format!(
                        "Action trigger `{}` resolved `{}` from `{}`; expected `trigger`.",
                        item.label,
                        value.value_type(),
                        item.source.stable_id
                    ),
                    DiagnosticOrigin::Runtime,
                )],
            },
            None => ActionTriggerMaterialization {
                value: None,
                diagnostics: vec![Diagnostic::error(
                    "action_trigger_missing_source",
                    format!(
                        "Action trigger `{}` could not resolve source `{}`.",
                        item.label, item.source.stable_id
                    ),
                    DiagnosticOrigin::Runtime,
                )],
            },
        }
    }
}

struct ActionCommandsRuntime {
    items: Vec<ActionCommandItem>,
}

struct ActionCommandItem {
    target: StableRef,
    enabled: bool,
    source_node: Option<ANodeId>,
}

impl ActionCommandsRuntime {
    fn from_managed_region(
        definition: &ManagedRegionDefinition,
        instance: &ManagedRegionInstance,
    ) -> Result<Self, ManagedFormulaError> {
        if definition.kind != ManagedRegionKind::ActionCommands {
            return Err(ManagedFormulaError::WrongActionCommandsRegionKind {
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
        if !definition.accepted_roles.contains(&SurfaceItemKind::Action) {
            return Err(ManagedFormulaError::DoesNotAcceptActionCommands {
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
                        return Err(ManagedFormulaError::InvalidActionCommandTargetConfig {
                            label: item.anode.label.clone(),
                            actual: value.value_type().to_string(),
                        });
                    }
                    None => {
                        return Err(ManagedFormulaError::MissingActionCommandTargetConfig {
                            label: item.anode.label.clone(),
                        });
                    }
                };
                Ok(ActionCommandItem {
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
            vec![ValueSetEntry::new(ValueLaneKey::new("trigger")?, "Trigger", value)],
        );
        match self.evaluate(values, ctx)? {
            ManagedFilterOutput::ValueSet(values) => {
                let actual = values.entries.len();
                let mut entries = values.entries.into_iter();
                let Some(entry) = entries.next() else {
                    return Err(ManagedFormulaError::ActionFilterExpectedSingleValue { actual: 0 });
                };
                if entries.next().is_some() {
                    return Err(ManagedFormulaError::ActionFilterExpectedSingleValue { actual });
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

        Ok(ManagedFilterCompiledRuntime::Projection { prefix, projection })
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
        projection: ValueSetProjectionRuntime,
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

#[derive(Debug, thiserror::Error)]
pub enum ManagedFormulaError {
    #[error("{0}")]
    Formula(#[from] FormulaMaterializationError),
    #[error("{0}")]
    ManagedRegionValidation(#[from] ManagedRegionValidationError),
    #[error("managed formula declares both Mapping and Action region families")]
    MixedManagedFormulaKinds,
    #[error("managed formula is missing a `{kind:?}` region")]
    MissingRegion { kind: ManagedRegionKind },
    #[error("managed formula declares more than one `{kind:?}` region")]
    DuplicateRegion { kind: ManagedRegionKind },
    #[error("managed region `{region_id}` has no instance")]
    MissingRegionInstance { region_id: ManagedRegionId },
    #[error("{0}")]
    InputSet(#[from] InputSetError),
    #[error("{0}")]
    OutputSet(#[from] OutputSetError),
    #[error("managed action trigger region `{region_id}` is `{actual:?}`, expected ActionTrigger")]
    WrongActionTriggerRegionKind {
        region_id: ManagedRegionId,
        actual: ManagedRegionKind,
    },
    #[error("ActionTrigger region `{region_id}` must accept input items")]
    DoesNotAcceptActionTriggers { region_id: ManagedRegionId },
    #[error("ActionTrigger item `{label}` is missing a `{INPUT_SOURCE_FIELD}` StableRef config field")]
    MissingActionTriggerSourceConfig { label: String },
    #[error("ActionTrigger item `{label}` has non-reference `{INPUT_SOURCE_FIELD}` config value `{actual}`")]
    InvalidActionTriggerSourceConfig { label: String, actual: String },
    #[error("managed action commands region `{region_id}` is `{actual:?}`, expected ActionCommands")]
    WrongActionCommandsRegionKind {
        region_id: ManagedRegionId,
        actual: ManagedRegionKind,
    },
    #[error("ActionCommands region `{region_id}` must accept action items")]
    DoesNotAcceptActionCommands { region_id: ManagedRegionId },
    #[error("ActionCommands item `{label}` is missing a `{OUTPUT_TARGET_FIELD}` StableRef config field")]
    MissingActionCommandTargetConfig { label: String },
    #[error("ActionCommands item `{label}` has non-reference `{OUTPUT_TARGET_FIELD}` config value `{actual}`")]
    InvalidActionCommandTargetConfig { label: String, actual: String },
    #[error("managed filter region `{region_id}` is `{actual:?}`, expected FilterPipeline")]
    WrongFilterRegionKind {
        region_id: ManagedRegionId,
        actual: ManagedRegionKind,
    },
    #[error("managed region instance `{instance_id}` does not match definition `{definition_id}`")]
    RegionMismatch {
        definition_id: ManagedRegionId,
        instance_id: ManagedRegionId,
    },
    #[error("FilterPipeline region `{region_id}` must accept filter items")]
    DoesNotAcceptFilters { region_id: ManagedRegionId },
    #[error("managed filter item declaration `{node_type}` is not registered")]
    MissingFilterDeclaration { node_type: golden_alchemist::ANodeTypeId },
    #[error("managed filter shape is invalid: {}", messages.join("; "))]
    InvalidFilterShape { messages: Vec<String> },
    #[error("unsupported managed filter pipeline: {0}")]
    UnsupportedFilterPipeline(String),
    #[error("managed filter pipeline requires at least one input value")]
    EmptyFilteredValueSet,
    #[error("managed ValueSet contains mixed value types `{expected}` and `{actual}`")]
    MixedValueSetTypes { expected: ValueTypeId, actual: ValueTypeId },
    #[error("Action filter expected one trigger value, got {actual}")]
    ActionFilterExpectedSingleValue { actual: usize },
    #[error("managed filter produced diagnostics: {}", messages.join("; "))]
    FilterDiagnostics { messages: Vec<String> },
    #[error("{0}")]
    ValueSet(#[from] ValueSetError),
    #[error("{0}")]
    ValueSetPipeline(#[from] ValueSetPipelineError),
}

impl ManagedFormulaError {
    pub fn into_diagnostic(self) -> Diagnostic {
        Diagnostic::error(self.diagnostic_code(), self.to_string(), DiagnosticOrigin::Graph)
    }

    fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::Formula(_) => "managed_formula_materialization_error",
            Self::ManagedRegionValidation(_) => "managed_formula_region_validation_error",
            Self::MixedManagedFormulaKinds => "managed_formula_mixed_region_kinds",
            Self::MissingRegion { .. } => "managed_formula_missing_region",
            Self::DuplicateRegion { .. } => "managed_formula_duplicate_region",
            Self::MissingRegionInstance { .. } => "managed_formula_missing_region_instance",
            Self::InputSet(_) => "managed_formula_input_set_error",
            Self::OutputSet(_) => "managed_formula_output_set_error",
            Self::WrongActionTriggerRegionKind { .. } => "managed_formula_wrong_action_trigger_region_kind",
            Self::DoesNotAcceptActionTriggers { .. } => "managed_formula_action_trigger_role_rejected",
            Self::MissingActionTriggerSourceConfig { .. } => "managed_formula_missing_action_trigger_source",
            Self::InvalidActionTriggerSourceConfig { .. } => "managed_formula_invalid_action_trigger_source",
            Self::WrongActionCommandsRegionKind { .. } => "managed_formula_wrong_action_commands_region_kind",
            Self::DoesNotAcceptActionCommands { .. } => "managed_formula_action_commands_role_rejected",
            Self::MissingActionCommandTargetConfig { .. } => "managed_formula_missing_action_command_target",
            Self::InvalidActionCommandTargetConfig { .. } => "managed_formula_invalid_action_command_target",
            Self::WrongFilterRegionKind { .. } => "managed_formula_wrong_filter_region_kind",
            Self::RegionMismatch { .. } => "managed_formula_region_mismatch",
            Self::DoesNotAcceptFilters { .. } => "managed_formula_filter_role_rejected",
            Self::MissingFilterDeclaration { .. } => "managed_formula_missing_filter_declaration",
            Self::InvalidFilterShape { .. } => "managed_formula_invalid_filter_shape",
            Self::UnsupportedFilterPipeline(_) => "managed_formula_unsupported_filter_pipeline",
            Self::EmptyFilteredValueSet => "managed_formula_empty_filtered_valueset",
            Self::MixedValueSetTypes { .. } => "managed_formula_mixed_valueset_types",
            Self::ActionFilterExpectedSingleValue { .. } => "managed_formula_action_filter_expected_single_value",
            Self::FilterDiagnostics { .. } => "managed_formula_filter_diagnostics",
            Self::ValueSet(_) => "managed_formula_valueset_error",
            Self::ValueSetPipeline(_) => "managed_formula_valueset_pipeline_error",
        }
    }
}
