use std::{collections::HashMap, sync::Arc};

use chataigne_alchemist::{
    ChannelDescriptor, ChannelLayout, ChannelLayoutError, ChannelMetadata, Diagnostic, DiagnosticOrigin,
    DiagnosticSeverity, EvaluationCtx, ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance,
    ManagedRegionKind, MappingValueShape, StableRef, SurfaceItemKind, ValueTypeId,
};
use golden_values::Value as RuntimeValue;

use crate::{ChannelFrame, ChannelFrameError, ChannelValidity, ValueLaneKey, ValueSet, ValueSetEntry};

pub const INPUT_SOURCE_FIELD: &str = "source";

#[derive(Clone, Debug, PartialEq)]
pub struct ChannelSourceSchema {
    pub value_type: ValueTypeId,
    pub metadata: ChannelMetadata,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputSetItem {
    pub key: ValueLaneKey,
    pub label: String,
    pub source: StableRef,
    pub enabled: bool,
    pub value_type: Option<ValueTypeId>,
    pub metadata: ChannelMetadata,
}

impl InputSetItem {
    #[must_use]
    pub fn new(key: ValueLaneKey, label: impl Into<String>, source: StableRef) -> Self {
        Self {
            key,
            label: label.into(),
            source,
            enabled: true,
            value_type: None,
            metadata: ChannelMetadata::default(),
        }
    }

    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn with_value_type(mut self, value_type: ValueTypeId) -> Self {
        self.value_type = Some(value_type);
        self
    }

    #[must_use]
    pub fn with_metadata(mut self, metadata: ChannelMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

#[derive(Clone, Debug)]
pub struct InputSetRuntime {
    items: Vec<InputSetItem>,
    frame: ChannelFrame,
}

impl InputSetRuntime {
    pub fn new(items: Vec<InputSetItem>) -> Result<Self, InputSetError> {
        let layout = Arc::new(input_layout(&items)?);
        Ok(Self {
            items,
            frame: ChannelFrame::new(layout),
        })
    }

    pub fn reconcile_items(&mut self, mut items: Vec<InputSetItem>) -> Result<(), InputSetError> {
        let previous: HashMap<_, _> = self.items.iter().map(|item| (&item.key, item)).collect();
        for item in &mut items {
            if item.value_type.is_some() {
                continue;
            }
            if let Some(previous) = previous
                .get(&item.key)
                .filter(|previous| previous.source == item.source)
            {
                item.value_type = previous.value_type.clone();
                if item.metadata == ChannelMetadata::default() {
                    item.metadata = previous.metadata.clone();
                }
            }
        }
        let channels = input_descriptors(&items);
        let layout = Arc::new(self.frame.layout().reconcile(channels)?);
        self.frame = self.frame.with_layout(layout);
        self.items = items;
        Ok(())
    }

    /// Apply an explicit backend source-schema event. Normal value samples never change layouts.
    pub fn reconcile_source_schema(
        &mut self,
        mut resolve: impl FnMut(&StableRef) -> Option<ChannelSourceSchema>,
    ) -> Result<(), InputSetError> {
        let mut items = self.items.clone();
        for item in &mut items {
            if let Some(schema) = resolve(&item.source) {
                item.value_type = Some(schema.value_type);
                item.metadata = schema.metadata;
            }
        }
        self.reconcile_items(items)
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
                    key: ValueLaneKey::input(item.id),
                    label: item.anode.label.clone(),
                    source,
                    enabled: item.enabled && item.anode.enabled,
                    value_type: None,
                    metadata: ChannelMetadata::default(),
                })
            })
            .collect::<Result<Vec<_>, InputSetError>>()?;

        Self::new(items)
    }

    #[must_use]
    pub fn items(&self) -> &[InputSetItem] {
        &self.items
    }

    #[must_use]
    pub fn layout(&self) -> &Arc<ChannelLayout> {
        self.frame.layout()
    }

    #[must_use]
    pub fn value_shape(&self) -> MappingValueShape {
        self.frame.layout().mapping_value_shape()
    }

    pub fn materialize(&mut self, ctx: &EvaluationCtx<'_>) -> InputSetMaterialization<'_> {
        self.frame.begin_tick(ctx.logical_tick);
        let mut value_set = ValueSet::new(ctx.logical_tick);
        let mut diagnostics = Vec::new();

        if self.items.is_empty() {
            diagnostics.push(Diagnostic {
                code: "input_set_empty".into(),
                message: "Mapping has no inputs. Add an Input Source before it can dispatch.".into(),
                severity: DiagnosticSeverity::Info,
                origin: DiagnosticOrigin::Runtime,
            });
        }

        for (index, item) in self.items.iter().enumerate() {
            if !item.enabled {
                self.frame
                    .set(index, None, ChannelValidity::Disabled, false)
                    .expect("input frame and layout have equal length");
                continue;
            }
            match ctx.inputs.get(&item.source) {
                Some(value) => {
                    if let Err(error) = self.frame.set(index, Some(value.clone()), ChannelValidity::Valid, true) {
                        self.frame
                            .set(index, None, ChannelValidity::Invalid, false)
                            .expect("invalid source state remains representable");
                        diagnostics.push(Diagnostic::error(
                            "input_set_type_mismatch",
                            error.to_string(),
                            DiagnosticOrigin::Runtime,
                        ));
                        continue;
                    }
                    value_set.push(
                        ValueSetEntry::new(item.key.clone(), item.label.clone(), value.clone())
                            .with_source(item.source.clone()),
                    );
                }
                None => {
                    self.frame
                        .set(index, None, ChannelValidity::MissingSource, false)
                        .expect("missing source state remains representable");
                    diagnostics.push(missing_source_diagnostic(item));
                }
            }
        }

        InputSetMaterialization {
            value_set,
            frame: &self.frame,
            diagnostics,
        }
    }
}

fn input_descriptors(items: &[InputSetItem]) -> Vec<ChannelDescriptor> {
    items
        .iter()
        .map(|item| {
            let mut descriptor = ChannelDescriptor::input(
                item.key.clone(),
                item.label.clone(),
                item.source.clone(),
                item.value_type.clone(),
            );
            descriptor.metadata = item.metadata.clone();
            descriptor
        })
        .collect()
}

fn input_layout(items: &[InputSetItem]) -> Result<ChannelLayout, ChannelLayoutError> {
    ChannelLayout::new(input_descriptors(items))
}

#[derive(Clone, Debug)]
pub struct InputSetMaterialization<'a> {
    pub value_set: ValueSet,
    pub frame: &'a ChannelFrame,
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
    Layout(#[from] ChannelLayoutError),
    #[error("{0}")]
    Frame(#[from] ChannelFrameError),
}
