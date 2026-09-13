use super::*;

// The trigger pipeline still uses this runner until typed flow stages replace it.

pub(super) struct ManagedFilterPipelineRuntime {
    definition: Option<ManagedRegionDefinition>,
    instance: Option<ManagedRegionInstance>,
    value_types: ValueTypeRegistry,
    nodes: ANodeRegistry,
    compiled_key: Option<ManagedFilterCompileKey>,
    compiled: ManagedFilterCompiledRuntime,
}

impl ManagedFilterPipelineRuntime {
    pub(super) fn update_runtime_input(
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

    pub(super) fn new(
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

    pub(super) fn evaluate_single(
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
