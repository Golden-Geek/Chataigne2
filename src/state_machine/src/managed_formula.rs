use golden_alchemist::{
    ANodeRegistry, AlchemistFormula, AlchemistFormulaInstance, CompileCtx, Diagnostic, DiagnosticOrigin, EvaluationCtx,
    ExecNodeId, FormulaMaterializationError, ManagedItemInstance, ManagedRegionDefinition, ManagedRegionId,
    ManagedRegionInstance, ManagedRegionKind, ManagedRegionValidationError, PipelineCardinality, PipelineLoweringCtx,
    PipelineShape, PipelineShapeCheckItem, RuntimeDiagnostic, RuntimeOutput, RuntimeValue, SignatureCtx,
    SurfaceItemKind, ValueTypeId, ValueTypeRegistry, check_filter_pipeline_shapes, value_set_shape,
};

use crate::{
    InputSetError, InputSetRuntime, OutputSetError, OutputSetMaterialization, OutputSetRuntime, ValueSet,
    ValueSetError, ValueSetPipelineError, ValueSetPipelineRuntime, ValueSetProjectionRuntime,
};

pub struct ManagedFormulaRuntime {
    input_set: InputSetRuntime,
    filter_pipeline: ManagedFilterPipelineRuntime,
    output_set: OutputSetRuntime,
}

impl ManagedFormulaRuntime {
    pub fn compile(
        formula: &AlchemistFormula,
        instance: &AlchemistFormulaInstance,
        ctx: &CompileCtx<'_>,
    ) -> Result<Option<Self>, ManagedFormulaError> {
        if !formula.surface.managed_regions.iter().any(is_managed_mapping_region) {
            return Ok(None);
        }
        instance
            .require_compatible(formula)
            .map_err(ManagedFormulaError::Formula)?;
        instance
            .managed_regions
            .validate_against(&formula.surface)
            .map_err(ManagedFormulaError::ManagedRegionValidation)?;

        let input = required_region(&formula.surface.managed_regions, ManagedRegionKind::InputSet)?;
        let output = required_region(&formula.surface.managed_regions, ManagedRegionKind::OutputSet)?;
        let filter = optional_region(&formula.surface.managed_regions, ManagedRegionKind::FilterPipeline)?;

        let input_instance = required_region_instance(instance, &input.id)?;
        let output_instance = required_region_instance(instance, &output.id)?;
        let filter_instance = filter
            .map(|definition| required_region_instance(instance, &definition.id).map(|region| (definition, region)))
            .transpose()?;

        Ok(Some(Self {
            input_set: InputSetRuntime::from_managed_region(input, input_instance)?,
            filter_pipeline: ManagedFilterPipelineRuntime::new(filter_instance, ctx)?,
            output_set: OutputSetRuntime::from_managed_region(output, output_instance)?,
        }))
    }

    pub fn evaluate(&mut self, ctx: &EvaluationCtx<'_>) -> RuntimeOutput {
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
            Err(error) => return runtime_error_output("managed_formula_filter_error", error),
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

fn is_managed_mapping_region(definition: &ManagedRegionDefinition) -> bool {
    matches!(
        definition.kind,
        ManagedRegionKind::InputSet | ManagedRegionKind::FilterPipeline | ManagedRegionKind::OutputSet
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

fn runtime_error_output(code: &'static str, error: ManagedFormulaError) -> RuntimeOutput {
    RuntimeOutput {
        diagnostics: vec![runtime_error(code, error)],
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
    #[error("managed filter produced diagnostics: {}", messages.join("; "))]
    FilterDiagnostics { messages: Vec<String> },
    #[error("{0}")]
    ValueSet(#[from] ValueSetError),
    #[error("{0}")]
    ValueSetPipeline(#[from] ValueSetPipelineError),
}

impl ManagedFormulaError {
    pub fn into_diagnostic(self) -> Diagnostic {
        Diagnostic::error(
            "managed_formula_compile_error",
            self.to_string(),
            DiagnosticOrigin::Graph,
        )
    }
}
