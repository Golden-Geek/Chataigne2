use chataigne_alchemist::{
    ANodeId, ChannelLayout, Diagnostic, DiagnosticOrigin, EvaluationCtx, ExtensionValue, ManagedRegionDefinition,
    ManagedRegionId, ManagedRegionInstance, ManagedRegionKind, RuntimeIntent, RuntimeOutput, StableRef,
    SurfaceItemKind, ValueComponent, ValueLaneKey, ValueTypeId, component_value_type,
};
use golden_values::Value as RuntimeValue;
use serde::{Deserialize, Serialize};

use crate::{ChannelFrame, ValueSet, ValueSetError};

mod authoring;
pub use authoring::{
    MappingConstantDto, MappingOutputArgumentDto, MappingOutputBindingsDto, MappingOutputSendPolicyDto,
    MappingOutputSourceDto, MappingTargetParameterDto, MappingValueComponentDto,
};

pub const OUTPUT_TARGET_FIELD: &str = "target";
pub const OUTPUT_BINDINGS_FIELD: &str = "bindings";
pub const COMMAND_INTENT_KIND: &str = "chataigne.command";
const OUTPUT_BINDINGS_TYPE: &str = "chataigne.output_bindings";
const COMMAND_ARGUMENTS_TYPE: &str = "chataigne.command_arguments";
const MAX_OUTPUT_ARGUMENTS: usize = 256;

/// A selector addresses the complete result or one stable tuple element. It never
/// depends on the position of an enabled OutputSet item.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum OutputValueSource {
    #[default]
    Whole,
    Element(ValueLaneKey),
    Component {
        element: Option<ValueLaneKey>,
        component: ValueComponent,
    },
    Constant(RuntimeValue),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputArgumentBinding {
    pub parameter: StableRef,
    pub source: OutputValueSource,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputSendPolicy {
    #[default]
    EveryDelivery,
    OnChange,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputBindingConfig {
    pub value: OutputValueSource,
    pub arguments: Vec<OutputArgumentBinding>,
    pub send_policy: OutputSendPolicy,
}

impl OutputBindingConfig {
    pub fn to_authoring_json(&self) -> Result<String, String> {
        serde_json::to_string(&MappingOutputBindingsDto::from_domain(self)?).map_err(|error| error.to_string())
    }

    pub fn from_authoring_json(value: &str) -> Result<Self, String> {
        serde_json::from_str::<MappingOutputBindingsDto>(value)
            .map_err(|error| error.to_string())?
            .into_domain()
    }

    pub fn to_runtime_value(&self) -> Result<RuntimeValue, String> {
        self.validate()?;
        let payload = serde_json::to_vec(self).map_err(|error| error.to_string())?;
        Ok(RuntimeValue::Extension(ExtensionValue::new(
            ValueTypeId::new(OUTPUT_BINDINGS_TYPE),
            payload,
        )))
    }

    fn from_runtime_value(value: &RuntimeValue) -> Result<Self, String> {
        if let RuntimeValue::String(value) = value {
            return Self::from_authoring_json(value);
        }
        let RuntimeValue::Extension(extension) = value else {
            return Err(format!(
                "expected `{OUTPUT_BINDINGS_TYPE}`, got `{}`",
                value.value_type()
            ));
        };
        if extension.value_type.as_str() != OUTPUT_BINDINGS_TYPE {
            return Err(format!(
                "expected `{OUTPUT_BINDINGS_TYPE}`, got `{}`",
                extension.value_type
            ));
        }
        let parsed: Self = serde_json::from_slice(&extension.payload).map_err(|error| error.to_string())?;
        parsed.validate()?;
        Ok(parsed)
    }

    fn validate(&self) -> Result<(), String> {
        if self.arguments.len() > MAX_OUTPUT_ARGUMENTS {
            return Err(format!("an output supports at most {MAX_OUTPUT_ARGUMENTS} arguments"));
        }
        if matches!(&self.value, OutputValueSource::Constant(value) if !runtime_value_is_finite(value))
            || self.arguments.iter().any(|binding| {
                matches!(&binding.source, OutputValueSource::Constant(value) if !runtime_value_is_finite(value))
            })
        {
            return Err("output bindings contain a non-finite constant".to_owned());
        }
        let mut seen = std::collections::HashSet::new();
        for argument in &self.arguments {
            if !seen.insert(&argument.parameter) {
                return Err(format!("argument `{}` is bound twice", argument.parameter.stable_id));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResolvedCommandArgument {
    pub parameter: StableRef,
    pub value: RuntimeValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandArgumentValues {
    pub value: RuntimeValue,
    pub arguments: Vec<ResolvedCommandArgument>,
    pub send_policy: OutputSendPolicy,
}

impl CommandArgumentValues {
    pub fn from_runtime_value(value: &RuntimeValue) -> Result<Option<Self>, String> {
        let RuntimeValue::Extension(extension) = value else {
            return Ok(None);
        };
        if extension.value_type.as_str() != COMMAND_ARGUMENTS_TYPE {
            return Ok(None);
        }
        let parsed: Self = serde_json::from_slice(&extension.payload).map_err(|error| error.to_string())?;
        if parsed.arguments.len() > MAX_OUTPUT_ARGUMENTS {
            return Err(format!("command payload exceeds {MAX_OUTPUT_ARGUMENTS} arguments"));
        }
        if !runtime_value_is_finite(&parsed.value)
            || parsed
                .arguments
                .iter()
                .any(|argument| !runtime_value_is_finite(&argument.value))
        {
            return Err("command arguments contain a non-finite number".to_owned());
        }
        Ok(Some(parsed))
    }

    pub fn into_runtime_value(self) -> Result<RuntimeValue, String> {
        if self.arguments.len() > MAX_OUTPUT_ARGUMENTS {
            return Err(format!("command payload exceeds {MAX_OUTPUT_ARGUMENTS} arguments"));
        }
        if !runtime_value_is_finite(&self.value)
            || self
                .arguments
                .iter()
                .any(|argument| !runtime_value_is_finite(&argument.value))
        {
            return Err("command arguments contain a non-finite number".to_owned());
        }
        let payload = serde_json::to_vec(&self).map_err(|error| error.to_string())?;
        Ok(RuntimeValue::Extension(ExtensionValue::new(
            ValueTypeId::new(COMMAND_ARGUMENTS_TYPE),
            payload,
        )))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OutputSetItem {
    pub label: String,
    pub target: StableRef,
    pub enabled: bool,
    pub source_node: Option<ANodeId>,
    pub bindings: OutputBindingConfig,
}

impl OutputSetItem {
    #[must_use]
    pub fn new(label: impl Into<String>, target: StableRef) -> Self {
        Self {
            label: label.into(),
            target,
            enabled: true,
            source_node: None,
            bindings: OutputBindingConfig::default(),
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

    #[must_use]
    pub fn with_bindings(mut self, bindings: OutputBindingConfig) -> Self {
        self.bindings = bindings;
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
                let bindings = item
                    .anode
                    .config
                    .get(OUTPUT_BINDINGS_FIELD)
                    .map(OutputBindingConfig::from_runtime_value)
                    .transpose()
                    .map_err(|detail| OutputSetError::InvalidBindings {
                        label: item.anode.label.clone(),
                        detail,
                    })?
                    .unwrap_or_default();
                Ok(OutputSetItem {
                    label: item.anode.label.clone(),
                    target,
                    enabled: item.enabled && item.anode.enabled,
                    source_node: Some(item.anode.id),
                    bindings,
                })
            })
            .collect::<Result<Vec<_>, OutputSetError>>()?;

        Ok(Self { items })
    }

    #[must_use]
    pub fn items(&self) -> &[OutputSetItem] {
        &self.items
    }

    pub fn validate_layout(&self, layout: &ChannelLayout) -> Result<(), OutputSetError> {
        for item in self.items.iter().filter(|item| item.enabled) {
            item.bindings
                .validate()
                .map_err(|detail| OutputSetError::InvalidBindings {
                    label: item.label.clone(),
                    detail,
                })?;
            for argument in &item.bindings.arguments {
                validate_source(&argument.source, layout).map_err(|detail| OutputSetError::InvalidBindings {
                    label: item.label.clone(),
                    detail,
                })?;
            }
            if !(matches!(item.bindings.value, OutputValueSource::Whole)
                && !item.bindings.arguments.is_empty()
                && layout.channels().len() != 1)
            {
                validate_source(&item.bindings.value, layout).map_err(|detail| OutputSetError::InvalidBindings {
                    label: item.label.clone(),
                    detail,
                })?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn materialize(&self, value: &RuntimeValue, ctx: &EvaluationCtx<'_>) -> OutputSetMaterialization {
        match ValueSet::from_runtime_value(value) {
            Ok(values) => self.materialize_values(&values, ctx),
            Err(ValueSetError::WrongValueType { .. }) => self.materialize_input(OutputInput::Single(value), ctx),
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

    #[must_use]
    pub fn materialize_values(&self, values: &ValueSet, ctx: &EvaluationCtx<'_>) -> OutputSetMaterialization {
        self.materialize_input(OutputInput::Tuple(values), ctx)
    }

    pub(crate) fn materialize_frame(&self, frame: &ChannelFrame, ctx: &EvaluationCtx<'_>) -> OutputSetMaterialization {
        self.materialize_input(OutputInput::Frame(frame), ctx)
    }

    fn materialize_input(&self, input: OutputInput<'_>, ctx: &EvaluationCtx<'_>) -> OutputSetMaterialization {
        let mut materialized = OutputSetMaterialization::default();
        for item in self.items.iter().filter(|item| item.enabled) {
            match resolve_item(item, input) {
                Ok(Some(payload)) => materialized
                    .output
                    .intents
                    .push(command_intent(item, payload, ctx.logical_tick)),
                Ok(None) => {}
                Err(detail) => materialized.diagnostics.push(Diagnostic::error(
                    "output_set_invalid_binding",
                    format!("Output `{}`: {detail}", item.label),
                    DiagnosticOrigin::Runtime,
                )),
            }
        }
        if !materialized.diagnostics.is_empty() {
            materialized.output.intents.clear();
        }
        materialized
    }
}

#[derive(Clone, Copy)]
enum OutputInput<'a> {
    Single(&'a RuntimeValue),
    Tuple(&'a ValueSet),
    Frame(&'a ChannelFrame),
}

fn resolve_item(item: &OutputSetItem, input: OutputInput<'_>) -> Result<Option<RuntimeValue>, String> {
    let mut arguments = Vec::with_capacity(item.bindings.arguments.len());
    for binding in &item.bindings.arguments {
        let value = resolve_source(&binding.source, input)?;
        if !should_emit(&value) {
            return Ok(None);
        }
        arguments.push(ResolvedCommandArgument {
            parameter: binding.parameter.clone(),
            value,
        });
    }
    let value = match resolve_source(&item.bindings.value, input) {
        Ok(value) => value,
        Err(_) if !arguments.is_empty() && matches!(item.bindings.value, OutputValueSource::Whole) => {
            RuntimeValue::Unit
        }
        Err(error) => return Err(error),
    };
    if !should_emit(&value) {
        return Ok(None);
    }
    if arguments.is_empty() && item.bindings.send_policy == OutputSendPolicy::EveryDelivery {
        return Ok(Some(value));
    }
    CommandArgumentValues {
        value,
        arguments,
        send_policy: item.bindings.send_policy,
    }
    .into_runtime_value()
    .map(Some)
}

fn resolve_source(source: &OutputValueSource, input: OutputInput<'_>) -> Result<RuntimeValue, String> {
    match source {
        OutputValueSource::Whole => match input {
            OutputInput::Single(value) => Ok(value.clone()),
            OutputInput::Tuple(values) if values.entries.len() == 1 => Ok(values.entries[0].value.clone()),
            OutputInput::Tuple(values) => Err(format!(
                "the result has {} tuple elements; select a stable element or use an explicit argument binding",
                values.entries.len()
            )),
            OutputInput::Frame(frame) if frame.slots().len() == 1 => Ok(frame.slots()[0]
                .value
                .clone()
                .expect("Mapping frame was validated before output materialization")),
            OutputInput::Frame(frame) => Err(format!(
                "the result has {} tuple elements; select a stable element or use an explicit argument binding",
                frame.slots().len()
            )),
        },
        OutputValueSource::Element(key) => match input {
            OutputInput::Tuple(values) => values
                .entries
                .iter()
                .find(|entry| &entry.key == key)
                .map(|entry| entry.value.clone())
                .ok_or_else(|| format!("tuple element `{}` is unavailable", key.as_str())),
            OutputInput::Frame(frame) => frame
                .layout()
                .channels()
                .iter()
                .zip(frame.slots())
                .find(|(descriptor, _)| &descriptor.id == key)
                .map(|(_, slot)| {
                    slot.value
                        .clone()
                        .expect("Mapping frame was validated before output materialization")
                })
                .ok_or_else(|| format!("tuple element `{}` is unavailable", key.as_str())),
            OutputInput::Single(_) => Err(format!(
                "result is scalar; tuple element `{}` is unavailable",
                key.as_str()
            )),
        },
        OutputValueSource::Component { element, component } => {
            let selected = if let Some(key) = element {
                resolve_source(&OutputValueSource::Element(key.clone()), input)?
            } else {
                resolve_source(&OutputValueSource::Whole, input)?
            };
            selected.component(*component).ok_or_else(|| {
                format!(
                    "component `{component:?}` is unavailable on `{}`",
                    selected.value_type()
                )
            })
        }
        OutputValueSource::Constant(value) => Ok(value.clone()),
    }
}

fn validate_source(source: &OutputValueSource, layout: &ChannelLayout) -> Result<(), String> {
    let selected_type = match source {
        OutputValueSource::Whole => {
            let [channel] = layout.channels() else {
                return Err(format!(
                    "the result has {} tuple elements; select a stable element or configure command arguments",
                    layout.channels().len()
                ));
            };
            channel.value_type.as_ref()
        }
        OutputValueSource::Element(key) => layout
            .channels()
            .iter()
            .find(|channel| &channel.id == key)
            .ok_or_else(|| format!("tuple element `{}` is unavailable", key.as_str()))?
            .value_type
            .as_ref(),
        OutputValueSource::Component { element, component } => {
            let selected = match element {
                Some(key) => OutputValueSource::Element(key.clone()),
                None => OutputValueSource::Whole,
            };
            validate_source(&selected, layout)?;
            let value_type = match selected {
                OutputValueSource::Element(ref key) => layout
                    .channels()
                    .iter()
                    .find(|channel| &channel.id == key)
                    .and_then(|channel| channel.value_type.as_ref()),
                _ => layout
                    .channels()
                    .first()
                    .and_then(|channel| channel.value_type.as_ref()),
            };
            if let Some(value_type) = value_type
                && component_value_type(value_type, *component).is_none()
            {
                return Err(format!("component `{component:?}` is unavailable on `{value_type}`"));
            }
            return Ok(());
        }
        OutputValueSource::Constant(value) => {
            return runtime_value_is_finite(value)
                .then_some(())
                .ok_or_else(|| "constant contains a non-finite number".to_owned());
        }
    };
    if selected_type.is_none() {
        return Err("the selected result type is unresolved".to_owned());
    }
    Ok(())
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

fn runtime_value_is_finite(value: &RuntimeValue) -> bool {
    match value {
        RuntimeValue::Float(value) => value.is_finite(),
        RuntimeValue::Vec2(value) => value.iter().all(|value| value.is_finite()),
        RuntimeValue::Vec3(value) => value.iter().all(|value| value.is_finite()),
        RuntimeValue::Color(value) => [value.red, value.green, value.blue, value.alpha]
            .iter()
            .all(|value| value.is_finite()),
        RuntimeValue::Array(values) => values.iter().all(runtime_value_is_finite),
        _ => true,
    }
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
    #[error("OutputSet item `{label}` has invalid bindings: {detail}")]
    InvalidBindings { label: String, detail: String },
}
