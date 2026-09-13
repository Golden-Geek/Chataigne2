use super::*;

pub(super) fn frame_values(frame: &ChannelFrame) -> Result<ValueSet, ManagedFormulaError> {
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
        match &descriptor.provenance {
            ChannelProvenance::Input(source) | ChannelProvenance::ProjectedInput { source, .. } => {
                entry = entry.with_source(source.clone());
            }
            _ => {}
        }
        values.push(entry);
    }
    Ok(values)
}

pub(super) fn validate_filter_region(
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

pub(super) fn merge_output_set(target: &mut RuntimeOutput, materialized: OutputSetMaterialization) {
    target.intents.extend(materialized.output.intents);
    target.diagnostics.extend(materialized.output.diagnostics);
    target.debug_samples.extend(materialized.output.debug_samples);
    target
        .diagnostics
        .extend(materialized.diagnostics.into_iter().map(runtime_diagnostic));
}

pub(super) fn merge_runtime_output(target: &mut RuntimeOutput, output: RuntimeOutput) {
    target.intents.extend(output.intents);
    target.diagnostics.extend(output.diagnostics);
    target.debug_samples.extend(output.debug_samples);
}

pub(super) fn should_emit(value: &RuntimeValue) -> bool {
    !matches!(value, RuntimeValue::Trigger(trigger) if !trigger.fired)
}

pub(super) fn runtime_error_output(error: ManagedFormulaError) -> RuntimeOutput {
    RuntimeOutput {
        diagnostics: vec![runtime_error(error.diagnostic_code(), error)],
        ..RuntimeOutput::default()
    }
}

pub(super) fn runtime_error(code: &'static str, error: impl ToString) -> RuntimeDiagnostic {
    RuntimeDiagnostic {
        exec_node: ExecNodeId::new(0),
        message: format!("{code}: {}", error.to_string()),
    }
}

pub(super) fn runtime_diagnostic(diagnostic: Diagnostic) -> RuntimeDiagnostic {
    RuntimeDiagnostic {
        exec_node: ExecNodeId::new(0),
        message: diagnostic.message,
    }
}
