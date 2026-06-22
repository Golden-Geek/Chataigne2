//! Chataigne-specific value types and nodes for `golden_alchemist`.

use std::{fmt::Debug, sync::Arc};

use golden_alchemist::{
    ANodeConfigFieldDecl, ANodeDeclaration, ANodeInstance, ANodeRegistry, ANodeSignature, ANodeTypeId,
    CompiledNodeEvaluator, CompiledNodeOperation, Diagnostic, DiagnosticOrigin, ExecutionKind, ExtensionValue, FacetId,
    InputSocketDecl, NodeEvaluation, OutputSocketDecl, RegistryError, ResolvedANodeSignature, RuntimeIntent,
    RuntimeValue, SignatureCtx, StableRef, TypeBindingSource, TypeBindings, TypeConstraint, TypeVar, ValueStorageKind,
    ValueTypeDescriptor, ValueTypeId, ValueTypeRegistry,
};

pub use golden_alchemist as alchemist;

use crate::{
    input_set::INPUT_SOURCE_FIELD,
    output_set::{COMMAND_INTENT_KIND, OUTPUT_TARGET_FIELD},
    value_set::ValueSet,
};

pub use crate::value_set::VALUE_SET_TYPE;

pub const MODULE_TYPE: &str = "chataigne.module";
pub const MODULE_ENDPOINT_TYPE: &str = "chataigne.module_endpoint";
pub const PROPERTY_GETTER_TYPE: &str = "property";
pub const CONDITIONS_MANAGER_TYPE: &str = "chataigne.conditions_manager";
pub const INPUTS_MANAGER_TYPE: &str = "chataigne.inputs_manager";
pub const OUTPUTS_MANAGER_TYPE: &str = "chataigne.outputs_manager";
pub const ROUTING_TYPE: &str = "chataigne.routing";
pub const COMMAND_TARGET_TYPE: &str = "chataigne.command_target";
pub const COMMAND_DRAFT_TYPE: &str = "chataigne.command_draft";
pub const SEQUENCE_TYPE: &str = "chataigne.sequence";
pub const STATE_TYPE: &str = "chataigne.state";
pub const PROCESSOR_TYPE: &str = "chataigne.processor";
pub const DASHBOARD_TARGET_TYPE: &str = "chataigne.dashboard_target";

pub fn register_value_types(registry: &mut ValueTypeRegistry) -> Result<(), RegistryError> {
    registry.register(ValueTypeDescriptor::new(
        ValueTypeId::new(VALUE_SET_TYPE),
        "Value Set",
        ValueStorageKind::Extension,
        || {
            ValueSet::new(0)
                .to_runtime_value()
                .expect("empty ValueSet must serialize")
        },
    ))?;
    register_ref(registry, MODULE_TYPE, "Module", &["node_ref", "command_target"])?;
    register_ref(
        registry,
        MODULE_ENDPOINT_TYPE,
        "Module Endpoint",
        &["node_ref", "command_target"],
    )?;
    register_ref(
        registry,
        CONDITIONS_MANAGER_TYPE,
        "Conditions Manager",
        &["condition_manager"],
    )?;
    register_ref(registry, INPUTS_MANAGER_TYPE, "Inputs Manager", &["inputs_manager"])?;
    register_ref(
        registry,
        OUTPUTS_MANAGER_TYPE,
        "Outputs Manager",
        &["outputs_manager", "command_target"],
    )?;
    register_ref(registry, COMMAND_TARGET_TYPE, "Command Target", &["command_target"])?;
    register_ref(registry, SEQUENCE_TYPE, "Sequence", &["launchable", "time_source"])?;
    register_ref(registry, STATE_TYPE, "State", &["activatable"])?;
    register_ref(registry, PROCESSOR_TYPE, "Processor", &["processor"])?;
    register_ref(
        registry,
        DASHBOARD_TARGET_TYPE,
        "Dashboard Target",
        &["dashboard_target"],
    )?;
    registry.register(ValueTypeDescriptor::new(
        ValueTypeId::new(COMMAND_DRAFT_TYPE),
        "Command Draft",
        ValueStorageKind::Extension,
        || {
            RuntimeValue::Extension(ExtensionValue::new(
                ValueTypeId::new(COMMAND_DRAFT_TYPE),
                Arc::<[u8]>::from([]),
            ))
        },
    ))?;
    Ok(())
}

pub fn register_nodes(registry: &mut ANodeRegistry) -> Result<(), RegistryError> {
    for kind in ChataigneNodeKind::ALL {
        registry.register(ChataigneNodeDeclaration(kind))?;
    }
    Ok(())
}

#[must_use]
pub fn value_type_registry() -> ValueTypeRegistry {
    let mut registry = ValueTypeRegistry::with_primitives();
    register_value_types(&mut registry).expect("Chataigne value type IDs must be unique");
    registry
}

#[must_use]
pub fn node_registry() -> ANodeRegistry {
    let mut registry = golden_alchemist::primitive_node_registry();
    register_nodes(&mut registry).expect("Chataigne ANode IDs must be unique");
    registry
}

fn register_ref(
    registry: &mut ValueTypeRegistry,
    id: &'static str,
    label: &'static str,
    facets: &[&str],
) -> Result<(), RegistryError> {
    registry.register(
        ValueTypeDescriptor::new(ValueTypeId::new(id), label, ValueStorageKind::StableRef, move || {
            RuntimeValue::Ref(StableRef::new(ValueTypeId::new(id), ""))
        })
        .with_facets(facets.iter().map(|facet| FacetId::new(*facet))),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChataigneNodeKind {
    ConditionsManagerRef,
    InputsManagerRef,
    OutputsManagerRef,
    Routing,
}

impl ChataigneNodeKind {
    const ALL: [Self; 4] = [
        Self::ConditionsManagerRef,
        Self::InputsManagerRef,
        Self::OutputsManagerRef,
        Self::Routing,
    ];

    fn type_id(self) -> &'static str {
        match self {
            Self::ConditionsManagerRef => CONDITIONS_MANAGER_TYPE,
            Self::InputsManagerRef => INPUTS_MANAGER_TYPE,
            Self::OutputsManagerRef => OUTPUTS_MANAGER_TYPE,
            Self::Routing => ROUTING_TYPE,
        }
    }
}

struct ChataigneNodeDeclaration(ChataigneNodeKind);

impl ANodeDeclaration for ChataigneNodeDeclaration {
    fn type_id(&self) -> ANodeTypeId {
        ANodeTypeId::new(self.0.type_id())
    }

    fn label(&self) -> &'static str {
        match self.0 {
            ChataigneNodeKind::ConditionsManagerRef => "Conditions",
            ChataigneNodeKind::InputsManagerRef => "Inputs",
            ChataigneNodeKind::OutputsManagerRef => "Output Commands",
            ChataigneNodeKind::Routing => "Routing",
        }
    }

    fn category(&self) -> &'static str {
        match self.0 {
            ChataigneNodeKind::Routing => "Routing",
            _ => "Chataigne",
        }
    }

    fn execution_kind(&self) -> ExecutionKind {
        match self.0 {
            ChataigneNodeKind::InputsManagerRef => ExecutionKind::EventSource,
            ChataigneNodeKind::OutputsManagerRef => ExecutionKind::EffectEmitter,
            ChataigneNodeKind::ConditionsManagerRef | ChataigneNodeKind::Routing => ExecutionKind::Pure,
        }
    }

    fn config_fields(&self) -> Vec<ANodeConfigFieldDecl> {
        match self.0 {
            ChataigneNodeKind::ConditionsManagerRef => vec![manager_ref_config_field(
                INPUT_SOURCE_FIELD,
                "Source",
                CONDITIONS_MANAGER_TYPE,
                "Conditions manager source to bridge into compact condition sockets.",
            )],
            ChataigneNodeKind::InputsManagerRef => vec![manager_ref_config_field(
                INPUT_SOURCE_FIELD,
                "Source",
                INPUTS_MANAGER_TYPE,
                "Inputs manager source to bridge as a ValueSet.",
            )],
            ChataigneNodeKind::OutputsManagerRef => vec![manager_ref_config_field(
                OUTPUT_TARGET_FIELD,
                "Target",
                OUTPUTS_MANAGER_TYPE,
                "Outputs manager target that receives bridged command payloads.",
            )],
            ChataigneNodeKind::Routing => Vec::new(),
        }
    }

    fn signature(
        &self,
        _ctx: &SignatureCtx<'_>,
        _instance: &ANodeInstance,
        _bindings: &TypeBindings,
    ) -> ANodeSignature {
        match self.0 {
            ChataigneNodeKind::ConditionsManagerRef => ANodeSignature {
                outputs: vec![
                    OutputSocketDecl::new("valid", "Valid", TypeConstraint::Exact(ValueTypeId::new("bool"))),
                    OutputSocketDecl::new("on_true", "On True", TypeConstraint::Exact(ValueTypeId::new("trigger"))),
                    OutputSocketDecl::new(
                        "on_false",
                        "On False",
                        TypeConstraint::Exact(ValueTypeId::new("trigger")),
                    ),
                ],
                ..ANodeSignature::default()
            },
            ChataigneNodeKind::InputsManagerRef => ANodeSignature {
                outputs: vec![OutputSocketDecl::new(
                    "values",
                    "Values",
                    TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE)),
                )],
                ..ANodeSignature::default()
            },
            ChataigneNodeKind::OutputsManagerRef => ANodeSignature {
                inputs: vec![
                    InputSocketDecl::new(
                        "values",
                        "Values",
                        TypeConstraint::OneOf(vec![
                            TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE)),
                            TypeConstraint::Primitive,
                        ]),
                    ),
                    InputSocketDecl::new(
                        "trigger",
                        "Trigger",
                        TypeConstraint::OneOf(vec![
                            TypeConstraint::Exact(ValueTypeId::new("trigger")),
                            TypeConstraint::Exact(ValueTypeId::new("unit")),
                        ]),
                    )
                    .with_default(RuntimeValue::Unit),
                ],
                ..ANodeSignature::default()
            },
            ChataigneNodeKind::Routing => {
                let variable = TypeVar::new("TValue");
                let mut signature = ANodeSignature {
                    inputs: vec![InputSocketDecl::new(
                        "in",
                        "In",
                        TypeConstraint::Generic(variable.clone()),
                    )],
                    outputs: vec![OutputSocketDecl::new(
                        "out",
                        "Out",
                        TypeConstraint::Generic(variable.clone()),
                    )],
                    ..ANodeSignature::default()
                };
                signature.default_bindings.insert(
                    variable.clone(),
                    ValueTypeId::new("float"),
                    TypeBindingSource::Default,
                );
                signature.generic_constraints.insert(variable, TypeConstraint::Any);
                signature
            }
        }
    }

    fn compile_operation(
        &self,
        instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, Diagnostic> {
        let evaluator: Arc<dyn CompiledNodeEvaluator> = match self.0 {
            ChataigneNodeKind::ConditionsManagerRef => Arc::new(ConditionsManagerRefEval {
                source: manager_ref_config(instance, INPUT_SOURCE_FIELD, "Conditions", CONDITIONS_MANAGER_TYPE)?,
            }),
            ChataigneNodeKind::InputsManagerRef => Arc::new(InputsManagerRefEval {
                source: manager_ref_config(instance, INPUT_SOURCE_FIELD, "Inputs", INPUTS_MANAGER_TYPE)?,
            }),
            ChataigneNodeKind::OutputsManagerRef => Arc::new(OutputsManagerRefEval {
                target: manager_ref_config(instance, OUTPUT_TARGET_FIELD, "Output Commands", OUTPUTS_MANAGER_TYPE)?,
            }),
            ChataigneNodeKind::Routing => Arc::new(RoutingEval),
        };
        Ok(CompiledNodeOperation::Custom(evaluator))
    }
}

fn manager_ref_config_field(
    id: &'static str,
    label: &'static str,
    value_type: &'static str,
    description: &'static str,
) -> ANodeConfigFieldDecl {
    ANodeConfigFieldDecl::new(
        id,
        label,
        RuntimeValue::Ref(StableRef::new(ValueTypeId::new(value_type), "")),
    )
    .with_description(description)
}

fn manager_ref_config(
    instance: &ANodeInstance,
    field: &'static str,
    label: &'static str,
    expected_type: &'static str,
) -> Result<StableRef, Diagnostic> {
    match instance.config.get(field) {
        Some(RuntimeValue::Ref(reference)) if reference.value_type.as_str() == expected_type => {
            if reference.stable_id.is_empty() {
                return Err(unbound_manager_ref_diagnostic(instance, label, field));
            }
            Ok(reference.clone())
        }
        Some(RuntimeValue::Ref(reference)) => Err(invalid_manager_ref_diagnostic(
            instance,
            label,
            field,
            &format!("reference type `{}`", reference.value_type),
            expected_type,
        )),
        Some(value) => Err(invalid_manager_ref_diagnostic(
            instance,
            label,
            field,
            &format!("runtime value `{}`", value.value_type()),
            expected_type,
        )),
        None => Err(missing_manager_ref_diagnostic(instance, label, field, expected_type)),
    }
}

fn missing_manager_ref_diagnostic(
    instance: &ANodeInstance,
    label: &'static str,
    field: &'static str,
    expected_type: &'static str,
) -> Diagnostic {
    Diagnostic::error(
        "chataigne_manager_bridge_missing_ref",
        format!(
            "Chataigne {label} manager bridge is missing `{field}` StableRef config of type `{expected_type}`. It does not return fallback values."
        ),
        DiagnosticOrigin::Node(instance.id),
    )
}

fn invalid_manager_ref_diagnostic(
    instance: &ANodeInstance,
    label: &'static str,
    field: &'static str,
    actual: &str,
    expected_type: &'static str,
) -> Diagnostic {
    Diagnostic::error(
        "chataigne_manager_bridge_invalid_ref",
        format!(
            "Chataigne {label} manager bridge has invalid `{field}` config {actual}; expected StableRef type `{expected_type}`. It does not return fallback values."
        ),
        DiagnosticOrigin::Node(instance.id),
    )
}

fn unbound_manager_ref_diagnostic(instance: &ANodeInstance, label: &'static str, field: &'static str) -> Diagnostic {
    Diagnostic::error(
        "chataigne_manager_bridge_unbound_ref",
        format!(
            "Chataigne {label} manager bridge has an unbound `{field}` reference. Select a manager target before compiling; it does not return fallback values."
        ),
        DiagnosticOrigin::Node(instance.id),
    )
}

#[derive(Debug)]
struct ConditionsManagerRefEval {
    source: StableRef,
}

impl CompiledNodeEvaluator for ConditionsManagerRefEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let value = evaluation.ctx.inputs.get(&self.source).ok_or_else(|| {
            format!(
                "Conditions manager bridge could not resolve source `{}`.",
                self.source.stable_id
            )
        })?;
        let values = ValueSet::from_runtime_value(value).map_err(|error| {
            format!(
                "Conditions manager bridge source `{}` did not produce a ValueSet: {error}",
                self.source.stable_id
            )
        })?;

        Ok(vec![
            condition_lane_bool(&values, "valid")?,
            condition_lane_trigger(&values, "on_true")?,
            condition_lane_trigger(&values, "on_false")?,
        ])
    }
}

#[derive(Debug)]
struct InputsManagerRefEval {
    source: StableRef,
}

impl CompiledNodeEvaluator for InputsManagerRefEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let value = evaluation.ctx.inputs.get(&self.source).ok_or_else(|| {
            format!(
                "Inputs manager bridge could not resolve source `{}`.",
                self.source.stable_id
            )
        })?;
        ValueSet::from_runtime_value(value).map_err(|error| {
            format!(
                "Inputs manager bridge source `{}` did not produce a ValueSet: {error}",
                self.source.stable_id
            )
        })?;
        Ok(vec![value.clone()])
    }
}

#[derive(Debug)]
struct OutputsManagerRefEval {
    target: StableRef,
}

impl CompiledNodeEvaluator for OutputsManagerRefEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let value = evaluation
            .inputs
            .first()
            .ok_or_else(|| "Outputs manager bridge expects a values input.".to_string())?;
        let should_emit = match evaluation.inputs.get(1).unwrap_or(&RuntimeValue::Unit) {
            RuntimeValue::Unit => true,
            RuntimeValue::Trigger(trigger) => trigger.fired,
            value => {
                return Err(format!(
                    "Outputs manager bridge trigger input resolved `{}`; expected `trigger` or `unit`.",
                    value.value_type()
                ));
            }
        };

        if should_emit {
            evaluation.intents.push(RuntimeIntent {
                kind: COMMAND_INTENT_KIND.into(),
                source_node: Some(evaluation.author_node_id),
                source_socket: None,
                target: Some(self.target.clone()),
                payload: value.clone(),
                logical_tick: evaluation.ctx.logical_tick,
            });
        }
        Ok(Vec::new())
    }
}

fn condition_lane_bool(values: &ValueSet, key: &'static str) -> Result<RuntimeValue, String> {
    match condition_lane(values, key)? {
        RuntimeValue::Bool(value) => Ok(RuntimeValue::Bool(*value)),
        value => Err(format!(
            "Conditions manager bridge lane `{key}` resolved `{}`; expected `bool`.",
            value.value_type()
        )),
    }
}

fn condition_lane_trigger(values: &ValueSet, key: &'static str) -> Result<RuntimeValue, String> {
    match condition_lane(values, key)? {
        RuntimeValue::Trigger(value) => Ok(RuntimeValue::Trigger(*value)),
        value => Err(format!(
            "Conditions manager bridge lane `{key}` resolved `{}`; expected `trigger`.",
            value.value_type()
        )),
    }
}

fn condition_lane<'a>(values: &'a ValueSet, key: &'static str) -> Result<&'a RuntimeValue, String> {
    values
        .entries
        .iter()
        .find(|entry| entry.key.as_str() == key)
        .map(|entry| &entry.value)
        .ok_or_else(|| format!("Conditions manager bridge ValueSet is missing `{key}` lane."))
}

#[derive(Debug)]
struct RoutingEval;

impl CompiledNodeEvaluator for RoutingEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        Ok(vec![evaluation.inputs.first().cloned().unwrap_or(RuntimeValue::Unit)])
    }
}

#[cfg(test)]
#[path = "alchemist_tests.rs"]
mod tests;
