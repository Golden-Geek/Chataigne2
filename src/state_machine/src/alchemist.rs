//! Chataigne-specific value types and nodes for `golden_alchemist`.

use std::{fmt::Debug, sync::Arc};

use golden_alchemist::{
    ANodeDeclaration, ANodeInstance, ANodeRegistry, ANodeRoleCapability, ANodeSignature, ANodeTypeId,
    CompiledNodeEvaluator, CompiledNodeOperation, Diagnostic, EvaluationCtx, ExecutionKind, ExtensionValue, FacetId,
    InputSocketDecl, NodeEvaluation, OutputSocketDecl, RegistryError, ResolvedANodeSignature, RuntimeIntent,
    RuntimeValue, SignatureCtx, StableRef, TriggerValue, TypeBindingSource, TypeBindings, TypeConstraint, TypeVar,
    ValueStorageKind, ValueTypeDescriptor, ValueTypeId, ValueTypeRegistry,
};

pub use golden_alchemist as alchemist;

pub use crate::value_set::VALUE_SET_TYPE;
use crate::value_set::ValueSet;

pub const MODULE_TYPE: &str = "chataigne.module";
pub const MODULE_ENDPOINT_TYPE: &str = "chataigne.module_endpoint";
pub const PROPERTY_GETTER_TYPE: &str = "property";
pub const CONDITIONS_MANAGER_TYPE: &str = "chataigne.conditions_manager";
pub const FILTERS_MANAGER_TYPE: &str = "chataigne.filters_manager";
pub const INPUTS_MANAGER_TYPE: &str = "chataigne.inputs_manager";
pub const OUTPUTS_MANAGER_TYPE: &str = "chataigne.outputs_manager";
pub const ROUTING_TYPE: &str = "chataigne.routing";
pub const COMMAND_TARGET_TYPE: &str = "chataigne.command_target";
pub const COMMAND_DRAFT_TYPE: &str = "chataigne.command_draft";
pub const SEQUENCE_TYPE: &str = "chataigne.sequence";
pub const STATE_TYPE: &str = "chataigne.state";
pub const PROCESSOR_TYPE: &str = "chataigne.processor";
pub const DASHBOARD_TARGET_TYPE: &str = "chataigne.dashboard_target";
pub const MANAGER_PROPERTY_FIELD: &str = "manager_id";
pub const TRIGGER_ON_VALUES_SIGNAL_FIELD: &str = "trigger_on_values_signal";

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
    ConditionsManager,
    FiltersManager,
    InputsManager,
    OutputsManager,
    Routing,
}

impl ChataigneNodeKind {
    const ALL: [Self; 5] = [
        Self::ConditionsManager,
        Self::FiltersManager,
        Self::InputsManager,
        Self::OutputsManager,
        Self::Routing,
    ];

    fn type_id(self) -> &'static str {
        match self {
            Self::ConditionsManager => CONDITIONS_MANAGER_TYPE,
            Self::FiltersManager => FILTERS_MANAGER_TYPE,
            Self::InputsManager => INPUTS_MANAGER_TYPE,
            Self::OutputsManager => OUTPUTS_MANAGER_TYPE,
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
            ChataigneNodeKind::ConditionsManager => "Conditions",
            ChataigneNodeKind::FiltersManager => "Filters",
            ChataigneNodeKind::InputsManager => "Inputs",
            ChataigneNodeKind::OutputsManager => "Outputs",
            ChataigneNodeKind::Routing => "Routing",
        }
    }

    fn category(&self) -> &'static str {
        match self.0 {
            ChataigneNodeKind::Routing => "Routing",
            _ => "Managers",
        }
    }

    fn execution_kind(&self) -> ExecutionKind {
        match self.0 {
            ChataigneNodeKind::OutputsManager => ExecutionKind::Stateful,
            _ => ExecutionKind::Pure,
        }
    }

    fn role_capabilities(&self) -> Vec<ANodeRoleCapability> {
        Vec::new()
    }

    fn config_fields(&self) -> Vec<golden_alchemist::ANodeConfigFieldDecl> {
        match self.0 {
            ChataigneNodeKind::ConditionsManager
            | ChataigneNodeKind::FiltersManager
            | ChataigneNodeKind::InputsManager => {
                vec![
                    golden_alchemist::ANodeConfigFieldDecl::new(
                        MANAGER_PROPERTY_FIELD,
                        "Manager",
                        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), "")),
                    )
                    .with_description("Referenced Formula manager property."),
                ]
            }
            ChataigneNodeKind::OutputsManager => {
                vec![
                    golden_alchemist::ANodeConfigFieldDecl::new(
                        MANAGER_PROPERTY_FIELD,
                        "Manager",
                        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), "")),
                    )
                    .with_description("Referenced Formula manager property."),
                    golden_alchemist::ANodeConfigFieldDecl::new(
                        TRIGGER_ON_VALUES_SIGNAL_FIELD,
                        "Trigger On Values Signal",
                        RuntimeValue::Bool(true),
                    )
                    .with_description(
                        "Emit output commands whenever the Values input receives a fresh signal, even without a trigger.",
                    ),
                ]
            }
            ChataigneNodeKind::Routing => Vec::new(),
        }
    }

    fn signature(
        &self,
        _ctx: &SignatureCtx<'_>,
        _instance: &ANodeInstance,
        _bindings: &TypeBindings,
    ) -> ANodeSignature {
        if self.0 == ChataigneNodeKind::ConditionsManager {
            return ANodeSignature {
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
            };
        }
        if self.0 == ChataigneNodeKind::InputsManager {
            return ANodeSignature {
                outputs: vec![OutputSocketDecl::new(
                    "values",
                    "Values",
                    TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE)),
                )],
                ..ANodeSignature::default()
            };
        }
        if self.0 == ChataigneNodeKind::FiltersManager {
            return ANodeSignature {
                inputs: vec![InputSocketDecl::new(
                    "values",
                    "Values",
                    TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE)),
                )],
                outputs: vec![OutputSocketDecl::new(
                    "values",
                    "Values",
                    TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE)),
                )],
                ..ANodeSignature::default()
            };
        }
        if self.0 == ChataigneNodeKind::OutputsManager {
            return ANodeSignature {
                inputs: vec![
                    InputSocketDecl::new(
                        "values",
                        "Values",
                        TypeConstraint::Exact(ValueTypeId::new(VALUE_SET_TYPE)),
                    ),
                    InputSocketDecl::new("trigger", "Trigger", TypeConstraint::Exact(ValueTypeId::new("trigger"))),
                ],
                ..ANodeSignature::default()
            };
        }

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
        signature
            .default_bindings
            .insert(variable.clone(), ValueTypeId::new("float"), TypeBindingSource::Default);
        signature.generic_constraints.insert(variable, TypeConstraint::Any);
        signature
    }

    fn compile_operation(
        &self,
        instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, Diagnostic> {
        match self.0 {
            ChataigneNodeKind::ConditionsManager => Ok(CompiledNodeOperation::Custom(Arc::new(ConditionManagerEval {
                source: manager_ref_from_config(instance, self.0),
            }))),
            ChataigneNodeKind::InputsManager => Ok(CompiledNodeOperation::Custom(Arc::new(ManagerSourceEval {
                source: manager_ref_from_config(instance, self.0),
            }))),
            ChataigneNodeKind::FiltersManager => Ok(CompiledNodeOperation::Custom(Arc::new(ManagerFilterEval))),
            ChataigneNodeKind::OutputsManager => Ok(CompiledNodeOperation::Custom(Arc::new(ManagerOutputEval {
                target: manager_ref_from_config(instance, self.0),
                trigger_on_values_signal: config_bool(instance, TRIGGER_ON_VALUES_SIGNAL_FIELD, true),
            }))),
            ChataigneNodeKind::Routing => Ok(CompiledNodeOperation::Custom(Arc::new(RoutingEval))),
        }
    }
}

fn config_bool(instance: &ANodeInstance, field: &str, fallback: bool) -> bool {
    match instance.config.get(field) {
        Some(RuntimeValue::Bool(value)) => *value,
        _ => fallback,
    }
}

fn manager_ref_from_config(instance: &ANodeInstance, kind: ChataigneNodeKind) -> Option<StableRef> {
    let RuntimeValue::Ref(value) = instance.config.get(MANAGER_PROPERTY_FIELD)? else {
        return None;
    };
    (!value.stable_id.is_empty()).then(|| StableRef::new(ValueTypeId::new(kind.type_id()), value.stable_id.clone()))
}

#[derive(Debug)]
struct RoutingEval;

impl CompiledNodeEvaluator for RoutingEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        Ok(vec![evaluation.inputs.first().cloned().unwrap_or(RuntimeValue::Unit)])
    }
}

#[derive(Debug)]
struct ManagerSourceEval {
    source: Option<StableRef>,
}

impl CompiledNodeEvaluator for ManagerSourceEval {
    fn change_detection_inputs(&self, ctx: &EvaluationCtx<'_>) -> Result<Vec<RuntimeValue>, String> {
        Ok(vec![manager_source_change_value(&self.source, ctx)])
    }

    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let value = self
            .source
            .as_ref()
            .and_then(|source| evaluation.ctx.inputs.get(source))
            .cloned()
            .unwrap_or_else(|| {
                ValueSet::new(evaluation.ctx.logical_tick)
                    .to_runtime_value()
                    .expect("empty ValueSet must serialize")
            });
        Ok(vec![value])
    }
}

#[derive(Debug)]
struct ConditionManagerEval {
    source: Option<StableRef>,
}

impl CompiledNodeEvaluator for ConditionManagerEval {
    fn change_detection_inputs(&self, ctx: &EvaluationCtx<'_>) -> Result<Vec<RuntimeValue>, String> {
        Ok(vec![manager_source_change_value(&self.source, ctx)])
    }

    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let values = self
            .source
            .as_ref()
            .and_then(|source| evaluation.ctx.inputs.get(source))
            .and_then(|value| ValueSet::from_runtime_value(value).ok());
        let mut valid = false;
        let mut on_true = TriggerValue::default();
        let mut on_false = TriggerValue::default();

        if let Some(values) = values {
            for entry in values.entries {
                match (entry.key.as_str(), entry.value) {
                    ("valid", RuntimeValue::Bool(value)) => valid = value,
                    ("on_true", RuntimeValue::Trigger(trigger)) => on_true = trigger,
                    ("on_false", RuntimeValue::Trigger(trigger)) => on_false = trigger,
                    _ => {}
                }
            }
        }

        Ok(vec![
            RuntimeValue::Bool(valid),
            RuntimeValue::Trigger(on_true),
            RuntimeValue::Trigger(on_false),
        ])
    }
}

fn manager_source_change_value(source: &Option<StableRef>, ctx: &EvaluationCtx<'_>) -> RuntimeValue {
    source
        .as_ref()
        .and_then(|source| ctx.inputs.get(source))
        .cloned()
        .unwrap_or(RuntimeValue::Unit)
}

#[derive(Debug)]
struct ManagerFilterEval;

impl CompiledNodeEvaluator for ManagerFilterEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        Ok(vec![evaluation.inputs.first().cloned().unwrap_or_else(|| {
            ValueSet::new(evaluation.ctx.logical_tick)
                .to_runtime_value()
                .expect("empty ValueSet must serialize")
        })])
    }
}

#[derive(Debug)]
struct ManagerOutputEval {
    target: Option<StableRef>,
    trigger_on_values_signal: bool,
}

impl CompiledNodeEvaluator for ManagerOutputEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let value = evaluation.inputs.first().cloned().unwrap_or_else(|| {
            ValueSet::new(evaluation.ctx.logical_tick)
                .to_runtime_value()
                .expect("empty ValueSet must serialize")
        });
        let trigger_allows_emit = evaluation
            .inputs
            .get(1)
            .and_then(|value| match value {
                RuntimeValue::Trigger(trigger) => Some(trigger.fired),
                _ => None,
            })
            .unwrap_or(false);
        let values_signal_received = if self.trigger_on_values_signal {
            let signal_token = values_signal_token(&value);
            if let Some(previous_value) = evaluation.state.first_mut() {
                let is_fresh_signal = *previous_value != signal_token;
                *previous_value = signal_token;
                is_fresh_signal
            } else {
                false
            }
        } else {
            false
        };
        if trigger_allows_emit || values_signal_received {
            evaluation.capture_debug_value("values", value.clone());
            evaluation.intents.push(RuntimeIntent {
                kind: crate::COMMAND_INTENT_KIND.into(),
                source_node: Some(evaluation.author_node_id),
                source_socket: None,
                target: self.target.clone(),
                payload: value,
                logical_tick: evaluation.ctx.logical_tick,
            });
        }
        Ok(Vec::new())
    }
}

fn values_signal_token(value: &RuntimeValue) -> RuntimeValue {
    ValueSet::from_runtime_value(value)
        .map(|values| RuntimeValue::String(values.logical_tick.to_string().into()))
        .unwrap_or_else(|_| value.clone())
}

#[cfg(test)]
#[path = "alchemist_tests.rs"]
mod tests;
