//! Chataigne-specific value types and nodes for `golden_alchemist`.

use std::{fmt::Debug, sync::Arc};

use golden_alchemist::{
    ANodeConfigFieldDecl, ANodeDeclaration, ANodeInstance, ANodeRegistry, ANodeSignature, ANodeTypeId,
    CompiledNodeEvaluator, CompiledNodeOperation, Diagnostic, ExecutionKind, ExtensionValue, FacetId, InputSocketDecl,
    NodeEvaluation, OutputSocketDecl, RegistryError, ResolvedANodeSignature, RuntimeValue, SignatureCtx, StableRef,
    TriggerValue, TypeBindingSource, TypeBindings, TypeConstraint, TypeVar, ValueStorageKind, ValueTypeDescriptor,
    ValueTypeId, ValueTypeRegistry,
};

pub use golden_alchemist as alchemist;

pub const MODULE_TYPE: &str = "chataigne.module";
pub const MODULE_ENDPOINT_TYPE: &str = "chataigne.module_endpoint";
pub const PARAM_ARRAY_TYPE: &str = "chataigne.param_array";
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
        ValueTypeId::new(PARAM_ARRAY_TYPE),
        "Parameter Array",
        ValueStorageKind::Extension,
        || {
            RuntimeValue::Extension(ExtensionValue::new(
                ValueTypeId::new(PARAM_ARRAY_TYPE),
                Arc::<[u8]>::from([]),
            ))
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
    PropertyGetter,
    ConditionsManagerRef,
    InputsManagerRef,
    OutputsManagerRef,
    Routing,
}

impl ChataigneNodeKind {
    const ALL: [Self; 5] = [
        Self::PropertyGetter,
        Self::ConditionsManagerRef,
        Self::InputsManagerRef,
        Self::OutputsManagerRef,
        Self::Routing,
    ];

    fn type_id(self) -> &'static str {
        match self {
            Self::PropertyGetter => PROPERTY_GETTER_TYPE,
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
            ChataigneNodeKind::PropertyGetter => "Property",
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
            ChataigneNodeKind::PropertyGetter
            | ChataigneNodeKind::ConditionsManagerRef
            | ChataigneNodeKind::Routing => ExecutionKind::Pure,
        }
    }

    fn config_fields(&self) -> Vec<ANodeConfigFieldDecl> {
        match self.0 {
            ChataigneNodeKind::PropertyGetter => vec![
                ANodeConfigFieldDecl::new("property_id", "Property ID", RuntimeValue::String("".into()))
                    .with_description("Stable Formula property identifier."),
                ANodeConfigFieldDecl::new("value", "Value", RuntimeValue::Float(0.0))
                    .with_description("The property default or Processor override.")
                    .with_editor("runtime_value"),
            ],
            ChataigneNodeKind::ConditionsManagerRef
            | ChataigneNodeKind::InputsManagerRef
            | ChataigneNodeKind::OutputsManagerRef
            | ChataigneNodeKind::Routing => Vec::new(),
        }
    }

    fn signature(&self, _ctx: &SignatureCtx<'_>, instance: &ANodeInstance, _bindings: &TypeBindings) -> ANodeSignature {
        match self.0 {
            ChataigneNodeKind::PropertyGetter => {
                let value_type = instance
                    .config
                    .get("value")
                    .map_or_else(|| ValueTypeId::new("float"), RuntimeValue::value_type);
                ANodeSignature {
                    inputs: Vec::new(),
                    outputs: vec![OutputSocketDecl::new(
                        "value",
                        "Value",
                        TypeConstraint::Exact(value_type),
                    )],
                    ..ANodeSignature::default()
                }
            }
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
                    "parameters",
                    "Parameters",
                    TypeConstraint::Exact(ValueTypeId::new(PARAM_ARRAY_TYPE)),
                )],
                ..ANodeSignature::default()
            },
            ChataigneNodeKind::OutputsManagerRef => ANodeSignature {
                inputs: vec![
                    InputSocketDecl::new(
                        "parameters",
                        "Parameters",
                        TypeConstraint::Exact(ValueTypeId::new(PARAM_ARRAY_TYPE)),
                    ),
                    InputSocketDecl::new("trigger", "Trigger", TypeConstraint::Exact(ValueTypeId::new("trigger"))),
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
            ChataigneNodeKind::PropertyGetter => {
                return Ok(CompiledNodeOperation::Constant(
                    instance
                        .config
                        .get("value")
                        .cloned()
                        .unwrap_or(RuntimeValue::Float(0.0)),
                ));
            }
            ChataigneNodeKind::ConditionsManagerRef => Arc::new(ConditionsManagerRefEval),
            ChataigneNodeKind::InputsManagerRef => Arc::new(ParamArraySourceEval),
            ChataigneNodeKind::OutputsManagerRef => Arc::new(NoOutputEval),
            ChataigneNodeKind::Routing => Arc::new(RoutingEval),
        };
        Ok(CompiledNodeOperation::Custom(evaluator))
    }
}

#[derive(Debug)]
struct ConditionsManagerRefEval;

impl CompiledNodeEvaluator for ConditionsManagerRefEval {
    fn evaluate(&self, _evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        Ok(vec![
            RuntimeValue::Bool(false),
            RuntimeValue::Trigger(TriggerValue::default()),
            RuntimeValue::Trigger(TriggerValue::default()),
        ])
    }
}

#[derive(Debug)]
struct NoOutputEval;

impl CompiledNodeEvaluator for NoOutputEval {
    fn evaluate(&self, _evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        Ok(vec![])
    }
}

#[derive(Debug)]
struct ParamArraySourceEval;

impl CompiledNodeEvaluator for ParamArraySourceEval {
    fn evaluate(&self, _evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        Ok(vec![RuntimeValue::Extension(ExtensionValue::new(
            ValueTypeId::new(PARAM_ARRAY_TYPE),
            Arc::<[u8]>::from([]),
        ))])
    }
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
