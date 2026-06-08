use indexmap::IndexMap;

use crate::{
    ANodeInstance, ANodeTypeId, CompiledNodeOperation, Diagnostic, DiagnosticOrigin, ResolvedANodeSignature,
    RuntimeValue, SocketId, TypeBindings, TypeConstraint, TypeVar, ValueTypeRegistry,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExecutionKind {
    Pure,
    Stateful,
    EventSource,
    EffectEmitter,
    Subgraph,
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct InputSocketDecl {
    pub id: SocketId,
    pub label: String,
    pub constraint: TypeConstraint,
    pub default_value: Option<RuntimeValue>,
}

impl InputSocketDecl {
    #[must_use]
    pub fn new(id: impl Into<SocketId>, label: impl Into<String>, constraint: TypeConstraint) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            constraint,
            default_value: None,
        }
    }

    #[must_use]
    pub fn with_default(mut self, value: RuntimeValue) -> Self {
        self.default_value = Some(value);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OutputSocketDecl {
    pub id: SocketId,
    pub label: String,
    pub constraint: TypeConstraint,
}

impl OutputSocketDecl {
    #[must_use]
    pub fn new(id: impl Into<SocketId>, label: impl Into<String>, constraint: TypeConstraint) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            constraint,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ANodeSignature {
    pub inputs: Vec<InputSocketDecl>,
    pub outputs: Vec<OutputSocketDecl>,
    pub default_bindings: TypeBindings,
    pub generic_constraints: IndexMap<TypeVar, TypeConstraint>,
}

pub struct SignatureCtx<'a> {
    pub value_types: &'a ValueTypeRegistry,
}

pub trait ANodeDeclaration: Send + Sync {
    fn type_id(&self) -> ANodeTypeId;
    fn label(&self) -> &'static str;
    fn category(&self) -> &'static str;
    fn execution_kind(&self) -> ExecutionKind;
    fn breaks_dependency_cycle(&self) -> bool {
        false
    }
    fn signature(&self, ctx: &SignatureCtx<'_>, instance: &ANodeInstance, bindings: &TypeBindings) -> ANodeSignature;
    fn compile_operation(
        &self,
        instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, Diagnostic> {
        Err(Diagnostic::error(
            "node_compile_not_implemented",
            format!("ANode `{}` does not provide a compiled operation", self.type_id()),
            DiagnosticOrigin::Node(instance.id),
        ))
    }
}
