//! Chataigne-specific value types and nodes for `golden_alchemist`.

use std::{fmt::Debug, sync::Arc};

use golden_alchemist::{
    ANodeDeclaration, ANodeInstance, ANodeRegistry, ANodeSignature, ANodeTypeId, CompiledNodeEvaluator,
    CompiledNodeOperation, Diagnostic, ExecutionKind, ExtensionValue, FacetId, InputSocketDecl, NodeEvaluation,
    OutputSocketDecl, RegistryError, ResolvedANodeSignature, RuntimeIntent, RuntimeValue, SignatureCtx, StableRef,
    TypeBindings, TypeConstraint, ValueStorageKind, ValueTypeDescriptor, ValueTypeId, ValueTypeRegistry,
};
use serde::{Deserialize, Serialize};

pub use golden_alchemist as alchemist;

pub const MODULE_TYPE: &str = "chataigne.module";
pub const MODULE_ENDPOINT_TYPE: &str = "chataigne.module_endpoint";
pub const COMMAND_TARGET_TYPE: &str = "chataigne.command_target";
pub const COMMAND_DRAFT_TYPE: &str = "chataigne.command_draft";
pub const SEQUENCE_TYPE: &str = "chataigne.sequence";
pub const STATE_TYPE: &str = "chataigne.state";
pub const PROCESSOR_TYPE: &str = "chataigne.processor";
pub const DASHBOARD_TARGET_TYPE: &str = "chataigne.dashboard_target";

pub fn register_value_types(registry: &mut ValueTypeRegistry) -> Result<(), RegistryError> {
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
    ModuleValueInput,
    CommandBuilder,
    CommandIntentOutput,
    SequenceIntentOutput,
    StateTransitionIntentOutput,
    StateActive,
}

impl ChataigneNodeKind {
    const ALL: [Self; 6] = [
        Self::ModuleValueInput,
        Self::CommandBuilder,
        Self::CommandIntentOutput,
        Self::SequenceIntentOutput,
        Self::StateTransitionIntentOutput,
        Self::StateActive,
    ];

    fn type_id(self) -> &'static str {
        match self {
            Self::ModuleValueInput => "chataigne.module_value_input",
            Self::CommandBuilder => "chataigne.command_builder",
            Self::CommandIntentOutput => "chataigne.command_intent_output",
            Self::SequenceIntentOutput => "chataigne.sequence_intent_output",
            Self::StateTransitionIntentOutput => "chataigne.state_transition_intent_output",
            Self::StateActive => "chataigne.state_active",
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
            ChataigneNodeKind::ModuleValueInput => "Module Value Input",
            ChataigneNodeKind::CommandBuilder => "Command Builder",
            ChataigneNodeKind::CommandIntentOutput => "Command Intent Output",
            ChataigneNodeKind::SequenceIntentOutput => "Sequence Intent Output",
            ChataigneNodeKind::StateTransitionIntentOutput => "State Transition Intent Output",
            ChataigneNodeKind::StateActive => "State Active",
        }
    }

    fn category(&self) -> &'static str {
        "Chataigne"
    }

    fn execution_kind(&self) -> ExecutionKind {
        match self.0 {
            ChataigneNodeKind::ModuleValueInput | ChataigneNodeKind::StateActive => ExecutionKind::EventSource,
            ChataigneNodeKind::CommandBuilder => ExecutionKind::Pure,
            ChataigneNodeKind::CommandIntentOutput
            | ChataigneNodeKind::SequenceIntentOutput
            | ChataigneNodeKind::StateTransitionIntentOutput => ExecutionKind::EffectEmitter,
        }
    }

    fn signature(&self, _ctx: &SignatureCtx<'_>, instance: &ANodeInstance, _bindings: &TypeBindings) -> ANodeSignature {
        match self.0 {
            ChataigneNodeKind::ModuleValueInput => ANodeSignature {
                outputs: vec![OutputSocketDecl::new(
                    "value",
                    "Value",
                    TypeConstraint::Exact(configured_value_type(instance)),
                )],
                ..ANodeSignature::default()
            },
            ChataigneNodeKind::CommandBuilder => ANodeSignature {
                inputs: vec![
                    InputSocketDecl::new(
                        "target",
                        "Target",
                        TypeConstraint::Facet(FacetId::new("command_target")),
                    ),
                    InputSocketDecl::new("payload", "Payload", TypeConstraint::Any),
                ],
                outputs: vec![OutputSocketDecl::new(
                    "command",
                    "Command",
                    TypeConstraint::Exact(ValueTypeId::new(COMMAND_DRAFT_TYPE)),
                )],
                ..ANodeSignature::default()
            },
            ChataigneNodeKind::CommandIntentOutput => ANodeSignature {
                inputs: vec![
                    InputSocketDecl::new("trigger", "Trigger", TypeConstraint::Exact(ValueTypeId::new("trigger"))),
                    InputSocketDecl::new(
                        "command",
                        "Command",
                        TypeConstraint::Exact(ValueTypeId::new(COMMAND_DRAFT_TYPE)),
                    ),
                ],
                ..ANodeSignature::default()
            },
            ChataigneNodeKind::SequenceIntentOutput => intent_signature(SEQUENCE_TYPE),
            ChataigneNodeKind::StateTransitionIntentOutput => intent_signature(STATE_TYPE),
            ChataigneNodeKind::StateActive => ANodeSignature {
                inputs: vec![InputSocketDecl::new(
                    "state",
                    "State",
                    TypeConstraint::Exact(ValueTypeId::new(STATE_TYPE)),
                )],
                outputs: vec![OutputSocketDecl::new(
                    "active",
                    "Active",
                    TypeConstraint::Exact(ValueTypeId::new("bool")),
                )],
                ..ANodeSignature::default()
            },
        }
    }

    fn compile_operation(
        &self,
        instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, Diagnostic> {
        let evaluator: Arc<dyn CompiledNodeEvaluator> = match self.0 {
            ChataigneNodeKind::ModuleValueInput => Arc::new(ReferenceInput {
                reference: config_ref(instance, "source", MODULE_ENDPOINT_TYPE),
            }),
            ChataigneNodeKind::CommandBuilder => Arc::new(CommandBuilder),
            ChataigneNodeKind::CommandIntentOutput => Arc::new(CommandOutput),
            ChataigneNodeKind::SequenceIntentOutput => Arc::new(ReferenceIntentOutput {
                kind: "chataigne.sequence",
            }),
            ChataigneNodeKind::StateTransitionIntentOutput => Arc::new(ReferenceIntentOutput {
                kind: "chataigne.state_transition",
            }),
            ChataigneNodeKind::StateActive => Arc::new(StateActive),
        };
        Ok(CompiledNodeOperation::Custom(evaluator))
    }
}

fn intent_signature(reference_type: &str) -> ANodeSignature {
    ANodeSignature {
        inputs: vec![
            InputSocketDecl::new("trigger", "Trigger", TypeConstraint::Exact(ValueTypeId::new("trigger"))),
            InputSocketDecl::new(
                "target",
                "Target",
                TypeConstraint::Exact(ValueTypeId::new(reference_type)),
            ),
        ],
        ..ANodeSignature::default()
    }
}

fn configured_value_type(instance: &ANodeInstance) -> ValueTypeId {
    match instance.config.get("value_type") {
        Some(RuntimeValue::String(value)) => ValueTypeId::new(value.as_ref()),
        _ => ValueTypeId::new("bool"),
    }
}

fn config_ref(instance: &ANodeInstance, field: &str, fallback_type: &str) -> StableRef {
    match instance.config.get(field) {
        Some(RuntimeValue::Ref(reference)) => reference.clone(),
        _ => StableRef::new(ValueTypeId::new(fallback_type), ""),
    }
}

#[derive(Debug)]
struct ReferenceInput {
    reference: StableRef,
}

impl CompiledNodeEvaluator for ReferenceInput {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        evaluation
            .ctx
            .inputs
            .get(&self.reference)
            .cloned()
            .map(|value| vec![value])
            .ok_or_else(|| format!("input `{}` is unavailable", self.reference.stable_id))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CommandDraft {
    target: StableRef,
    payload: RuntimeValue,
}

#[derive(Debug)]
struct CommandBuilder;

impl CompiledNodeEvaluator for CommandBuilder {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [RuntimeValue::Ref(target), payload] = evaluation.inputs else {
            return Err("Command Builder expects target and payload inputs".into());
        };
        let bytes = serde_json::to_vec(&CommandDraft {
            target: target.clone(),
            payload: payload.clone(),
        })
        .map_err(|error| error.to_string())?;
        Ok(vec![RuntimeValue::Extension(ExtensionValue::new(
            ValueTypeId::new(COMMAND_DRAFT_TYPE),
            bytes,
        ))])
    }
}

#[derive(Debug)]
struct CommandOutput;

impl CompiledNodeEvaluator for CommandOutput {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [RuntimeValue::Trigger(trigger), RuntimeValue::Extension(command)] = evaluation.inputs else {
            return Err("Command Intent Output expects trigger and command inputs".into());
        };
        if trigger.fired {
            let draft: CommandDraft = serde_json::from_slice(&command.payload).map_err(|error| error.to_string())?;
            evaluation.intents.push(RuntimeIntent {
                kind: Arc::from("chataigne.command"),
                target: Some(draft.target),
                payload: draft.payload,
                logical_tick: evaluation.ctx.logical_tick,
            });
        }
        Ok(Vec::new())
    }
}

#[derive(Debug)]
struct ReferenceIntentOutput {
    kind: &'static str,
}

impl CompiledNodeEvaluator for ReferenceIntentOutput {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [RuntimeValue::Trigger(trigger), RuntimeValue::Ref(target)] = evaluation.inputs else {
            return Err("intent output expects trigger and reference inputs".into());
        };
        if trigger.fired {
            evaluation.intents.push(RuntimeIntent {
                kind: Arc::from(self.kind),
                target: Some(target.clone()),
                payload: RuntimeValue::Unit,
                logical_tick: evaluation.ctx.logical_tick,
            });
        }
        Ok(Vec::new())
    }
}

#[derive(Debug)]
struct StateActive;

impl CompiledNodeEvaluator for StateActive {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<Vec<RuntimeValue>, String> {
        let [RuntimeValue::Ref(state)] = evaluation.inputs else {
            return Err("State Active expects a state reference".into());
        };
        let active = matches!(evaluation.ctx.inputs.get(state), Some(RuntimeValue::Bool(true)));
        Ok(vec![RuntimeValue::Bool(active)])
    }
}

#[cfg(test)]
mod tests;
