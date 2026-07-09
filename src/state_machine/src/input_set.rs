use golden_alchemist::{
    Diagnostic, DiagnosticOrigin, EvaluationCtx, ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance,
    ManagedRegionKind, RuntimeValue, StableRef, SurfaceItemKind,
};

use crate::{ValueLaneKey, ValueSet, ValueSetEntry, ValueSetError};

pub const INPUT_SOURCE_FIELD: &str = "source";

#[derive(Clone, Debug, PartialEq)]
pub struct InputSetItem {
    pub key: ValueLaneKey,
    pub label: String,
    pub source: StableRef,
    pub enabled: bool,
}

impl InputSetItem {
    #[must_use]
    pub fn new(key: ValueLaneKey, label: impl Into<String>, source: StableRef) -> Self {
        Self {
            key,
            label: label.into(),
            source,
            enabled: true,
        }
    }

    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputSetRuntime {
    items: Vec<InputSetItem>,
}

impl InputSetRuntime {
    #[must_use]
    pub fn new(items: Vec<InputSetItem>) -> Self {
        Self { items }
    }

    pub fn from_managed_region(
        definition: &ManagedRegionDefinition,
        instance: &ManagedRegionInstance,
    ) -> Result<Self, InputSetError> {
        if definition.kind != ManagedRegionKind::InputSet {
            return Err(InputSetError::WrongRegionKind {
                region_id: definition.id.clone(),
                actual: definition.kind,
            });
        }
        if definition.id != instance.region_id {
            return Err(InputSetError::RegionMismatch {
                definition_id: definition.id.clone(),
                instance_id: instance.region_id.clone(),
            });
        }
        if !definition.accepted_roles.contains(&SurfaceItemKind::Input) {
            return Err(InputSetError::DoesNotAcceptInputs {
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
                        return Err(InputSetError::InvalidSourceConfig {
                            label: item.anode.label.clone(),
                            actual: value.value_type().to_string(),
                        });
                    }
                    None => {
                        return Err(InputSetError::MissingSourceConfig {
                            label: item.anode.label.clone(),
                        });
                    }
                };
                Ok(InputSetItem {
                    key: ValueLaneKey::new(format!("input:{}", item.id))?,
                    label: item.anode.label.clone(),
                    source,
                    enabled: item.enabled && item.anode.enabled,
                })
            })
            .collect::<Result<Vec<_>, InputSetError>>()?;

        Ok(Self { items })
    }

    #[must_use]
    pub fn items(&self) -> &[InputSetItem] {
        &self.items
    }

    #[must_use]
    pub fn materialize(&self, ctx: &EvaluationCtx<'_>) -> InputSetMaterialization {
        let mut value_set = ValueSet::new(ctx.logical_tick);
        let mut diagnostics = Vec::new();

        for item in self.items.iter().filter(|item| item.enabled) {
            match ctx.inputs.get(&item.source) {
                Some(value) => {
                    value_set.push(
                        ValueSetEntry::new(item.key.clone(), item.label.clone(), value.clone())
                            .with_source(item.source.clone()),
                    );
                }
                None => diagnostics.push(missing_source_diagnostic(item)),
            }
        }

        InputSetMaterialization { value_set, diagnostics }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputSetMaterialization {
    pub value_set: ValueSet,
    pub diagnostics: Vec<Diagnostic>,
}

fn missing_source_diagnostic(item: &InputSetItem) -> Diagnostic {
    Diagnostic::error(
        "input_set_missing_source",
        format!(
            "Input `{}` could not resolve source `{}` of type `{}`.",
            item.label, item.source.stable_id, item.source.value_type
        ),
        DiagnosticOrigin::Runtime,
    )
}

#[derive(Debug, thiserror::Error)]
pub enum InputSetError {
    #[error("managed region `{region_id}` is `{actual:?}`, expected InputSet")]
    WrongRegionKind {
        region_id: ManagedRegionId,
        actual: ManagedRegionKind,
    },
    #[error("managed region instance `{instance_id}` does not match definition `{definition_id}`")]
    RegionMismatch {
        definition_id: ManagedRegionId,
        instance_id: ManagedRegionId,
    },
    #[error("InputSet region `{region_id}` must accept input items")]
    DoesNotAcceptInputs { region_id: ManagedRegionId },
    #[error("InputSet item `{label}` is missing a `{INPUT_SOURCE_FIELD}` StableRef config field")]
    MissingSourceConfig { label: String },
    #[error("InputSet item `{label}` has non-reference `{INPUT_SOURCE_FIELD}` config value `{actual}`")]
    InvalidSourceConfig { label: String, actual: String },
    #[error("{0}")]
    ValueSet(#[from] ValueSetError),
}
