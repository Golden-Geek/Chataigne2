use chataigne_alchemist::{
    ANodeId, Diagnostic, DiagnosticOrigin, EvaluationCtx, ManagedRegionDefinition, ManagedRegionId,
    ManagedRegionInstance, ManagedRegionKind, RuntimeIntent, RuntimeOutput, StableRef, SurfaceItemKind,
};
use golden_values::Value as RuntimeValue;

use crate::{ValueSet, ValueSetError};

pub const OUTPUT_TARGET_FIELD: &str = "target";
pub const COMMAND_INTENT_KIND: &str = "chataigne.command";

#[derive(Clone, Debug, PartialEq)]
pub struct OutputSetItem {
    pub label: String,
    pub target: StableRef,
    pub enabled: bool,
    pub source_node: Option<ANodeId>,
}

impl OutputSetItem {
    #[must_use]
    pub fn new(label: impl Into<String>, target: StableRef) -> Self {
        Self {
            label: label.into(),
            target,
            enabled: true,
            source_node: None,
        }
    }

    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn with_source_node(mut self, source_node: ANodeId) -> Self {
        self.source_node = Some(source_node);
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OutputSetRuntime {
    items: Vec<OutputSetItem>,
}

impl OutputSetRuntime {
    #[must_use]
    pub fn new(items: Vec<OutputSetItem>) -> Self {
        Self { items }
    }

    pub fn from_managed_region(
        definition: &ManagedRegionDefinition,
        instance: &ManagedRegionInstance,
    ) -> Result<Self, OutputSetError> {
        if definition.kind != ManagedRegionKind::OutputSet {
            return Err(OutputSetError::WrongRegionKind {
                region_id: definition.id.clone(),
                actual: definition.kind,
            });
        }
        if definition.id != instance.region_id {
            return Err(OutputSetError::RegionMismatch {
                definition_id: definition.id.clone(),
                instance_id: instance.region_id.clone(),
            });
        }
        if !definition.accepted_roles.contains(&SurfaceItemKind::Output) {
            return Err(OutputSetError::DoesNotAcceptOutputs {
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
                        return Err(OutputSetError::InvalidTargetConfig {
                            label: item.anode.label.clone(),
                            actual: value.value_type().to_string(),
                        });
                    }
                    None => {
                        return Err(OutputSetError::MissingTargetConfig {
                            label: item.anode.label.clone(),
                        });
                    }
                };
                Ok(OutputSetItem {
                    label: item.anode.label.clone(),
                    target,
                    enabled: item.enabled && item.anode.enabled,
                    source_node: Some(item.anode.id),
                })
            })
            .collect::<Result<Vec<_>, OutputSetError>>()?;

        Ok(Self { items })
    }

    #[must_use]
    pub fn items(&self) -> &[OutputSetItem] {
        &self.items
    }

    #[must_use]
    pub fn materialize(&self, value: &RuntimeValue, ctx: &EvaluationCtx<'_>) -> OutputSetMaterialization {
        let enabled_outputs = self.items.iter().filter(|item| item.enabled).collect::<Vec<_>>();
        if enabled_outputs.is_empty() {
            return OutputSetMaterialization::default();
        }

        match ValueSet::from_runtime_value(value) {
            Ok(values) => self.materialize_value_set(&values, &enabled_outputs, ctx),
            Err(ValueSetError::WrongValueType { .. }) => self.materialize_single(value, &enabled_outputs, ctx),
            Err(error) => OutputSetMaterialization {
                output: RuntimeOutput::default(),
                diagnostics: vec![Diagnostic::error(
                    "output_set_invalid_valueset",
                    error.to_string(),
                    DiagnosticOrigin::Runtime,
                )],
            },
        }
    }

    fn materialize_single(
        &self,
        value: &RuntimeValue,
        outputs: &[&OutputSetItem],
        ctx: &EvaluationCtx<'_>,
    ) -> OutputSetMaterialization {
        if !should_emit(value) {
            return OutputSetMaterialization::default();
        }
        if outputs.len() != 1 {
            return OutputSetMaterialization {
                output: RuntimeOutput::default(),
                diagnostics: vec![Diagnostic::error(
                    "output_set_single_value_requires_single_output",
                    format!(
                        "OutputSet received one `{}` value but has {} enabled outputs. Add an explicit Broadcast filter before targeting multiple outputs.",
                        value.value_type(),
                        outputs.len()
                    ),
                    DiagnosticOrigin::Runtime,
                )],
            };
        }

        OutputSetMaterialization {
            output: RuntimeOutput {
                intents: vec![command_intent(outputs[0], value.clone(), ctx.logical_tick)],
                ..RuntimeOutput::default()
            },
            diagnostics: Vec::new(),
        }
    }

    fn materialize_value_set(
        &self,
        values: &ValueSet,
        outputs: &[&OutputSetItem],
        ctx: &EvaluationCtx<'_>,
    ) -> OutputSetMaterialization {
        if values.entries.len() != outputs.len() {
            return OutputSetMaterialization {
                output: RuntimeOutput::default(),
                diagnostics: vec![Diagnostic::error(
                    "output_set_valueset_output_mismatch",
                    format!(
                        "OutputSet received {} ValueSet entries but has {} enabled outputs.",
                        values.entries.len(),
                        outputs.len()
                    ),
                    DiagnosticOrigin::Runtime,
                )],
            };
        }

        let intents = values
            .entries
            .iter()
            .zip(outputs.iter().copied())
            .filter(|(entry, _)| should_emit(&entry.value))
            .map(|(entry, output)| command_intent(output, entry.value.clone(), ctx.logical_tick))
            .collect();

        OutputSetMaterialization {
            output: RuntimeOutput {
                intents,
                ..RuntimeOutput::default()
            },
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct OutputSetMaterialization {
    pub output: RuntimeOutput,
    pub diagnostics: Vec<Diagnostic>,
}

fn command_intent(item: &OutputSetItem, payload: RuntimeValue, logical_tick: u64) -> RuntimeIntent {
    RuntimeIntent {
        kind: COMMAND_INTENT_KIND.into(),
        source_node: item.source_node,
        source_socket: None,
        target: Some(item.target.clone()),
        payload,
        logical_tick,
    }
}

fn should_emit(value: &RuntimeValue) -> bool {
    !matches!(value, RuntimeValue::Trigger(trigger) if !trigger.fired)
}

#[derive(Debug, thiserror::Error)]
pub enum OutputSetError {
    #[error("managed region `{region_id}` is `{actual:?}`, expected OutputSet")]
    WrongRegionKind {
        region_id: ManagedRegionId,
        actual: ManagedRegionKind,
    },
    #[error("managed region instance `{instance_id}` does not match definition `{definition_id}`")]
    RegionMismatch {
        definition_id: ManagedRegionId,
        instance_id: ManagedRegionId,
    },
    #[error("OutputSet region `{region_id}` must accept output items")]
    DoesNotAcceptOutputs { region_id: ManagedRegionId },
    #[error("OutputSet item `{label}` is missing a `{OUTPUT_TARGET_FIELD}` StableRef config field")]
    MissingTargetConfig { label: String },
    #[error("OutputSet item `{label}` has non-reference `{OUTPUT_TARGET_FIELD}` config value `{actual}`")]
    InvalidTargetConfig { label: String, actual: String },
}
