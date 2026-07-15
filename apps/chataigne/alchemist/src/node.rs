use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::{
    ANodeInstance, ANodeTypeId, AxisSet, CompiledNodeOperation, Diagnostic, DiagnosticOrigin, FormulaPropertySchema,
    ResolvedANodeSignature, RuntimeValue, SocketId, SurfaceItemKind, TypeBindings, TypeConstraint, TypeVar,
    ValueStorageKind, ValueTypeId, ValueTypeRegistry,
};

pub const PROCESS_ON_INPUT_CHANGE_ONLY_CONFIG: &str = "process_on_input_change_only";
pub const SEND_ON_OUTPUT_CHANGE_ONLY_CONFIG: &str = "send_on_output_change_only";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ExecutionKind {
    Pure,
    Stateful,
    EventSource,
    EffectEmitter,
    Subgraph,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NodeStateLayout {
    Stateless,
    RuntimeValues(usize),
}

impl NodeStateLayout {
    #[must_use]
    pub const fn slot_count(&self) -> usize {
        match self {
            Self::Stateless => 0,
            Self::RuntimeValues(count) => *count,
        }
    }
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
    pub properties: Option<&'a FormulaPropertySchema>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum AutoWirePolicy {
    None,
    UnaryTransform {
        input: SocketId,
        output: SocketId,
    },
    Source {
        output: SocketId,
    },
    Sink {
        input: SocketId,
        trigger: Option<SocketId>,
    },
    Gate {
        input: SocketId,
        condition: SocketId,
        output: SocketId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PipelineCardinality {
    Elementwise,
    Aggregate,
    Reshape,
    Expand,
    WholeSet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ManagedUiMode {
    FullGraph,
    CompactRow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ANodeRoleCapability {
    pub role: SurfaceItemKind,
    pub primary_input: Option<SocketId>,
    pub primary_output: Option<SocketId>,
    pub autowire: AutoWirePolicy,
    pub cardinality: PipelineCardinality,
    pub ui_mode: ManagedUiMode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ANodeConfigFieldDecl {
    pub id: SmolStr,
    pub label: String,
    pub description: Option<String>,
    pub editor: Option<String>,
    pub enum_options: Vec<(SmolStr, String)>,
    pub type_variable: Option<TypeVar>,
    pub type_options: Vec<ValueTypeId>,
    pub default_value: RuntimeValue,
}

impl ANodeConfigFieldDecl {
    #[must_use]
    pub fn new(id: impl Into<SmolStr>, label: impl Into<String>, default_value: RuntimeValue) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
            editor: None,
            enum_options: Vec::new(),
            type_variable: None,
            type_options: Vec::new(),
            default_value,
        }
    }

    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    #[must_use]
    pub fn with_editor(mut self, editor: impl Into<String>) -> Self {
        self.editor = Some(editor.into());
        self
    }

    #[must_use]
    pub fn with_enum_options(
        mut self,
        options: impl IntoIterator<Item = (impl Into<SmolStr>, impl Into<String>)>,
    ) -> Self {
        self.enum_options = options
            .into_iter()
            .map(|(id, label)| (id.into(), label.into()))
            .collect();
        self
    }

    #[must_use]
    pub fn with_type_variable(mut self, variable: impl Into<TypeVar>) -> Self {
        self.type_variable = Some(variable.into());
        self
    }

    #[must_use]
    pub fn with_type_options(mut self, options: impl IntoIterator<Item = ValueTypeId>) -> Self {
        self.type_options = options.into_iter().collect();
        self
    }

    #[must_use]
    pub fn resolved_type_options(&self, signature: &ANodeSignature, registry: &ValueTypeRegistry) -> Vec<ValueTypeId> {
        if !self.type_options.is_empty() {
            return self.type_options.clone();
        }
        let constraint = self
            .type_variable
            .as_ref()
            .and_then(|variable| signature.generic_constraints.get(variable));
        registry
            .iter()
            .filter(|descriptor| !matches!(descriptor.storage, ValueStorageKind::Extension))
            .filter(|descriptor| {
                constraint.is_none_or(|constraint| constraint.accepts_value_type(&descriptor.id, registry))
            })
            .map(|descriptor| descriptor.id.clone())
            .collect()
    }
}

pub trait ANodeDeclaration: Send + Sync {
    fn type_id(&self) -> ANodeTypeId;
    fn label(&self) -> &'static str;
    fn category(&self) -> &'static str;
    fn execution_kind(&self) -> ExecutionKind;
    fn breaks_dependency_cycle(&self) -> bool {
        false
    }
    fn config_fields(&self) -> Vec<ANodeConfigFieldDecl> {
        Vec::new()
    }
    fn config_fields_for(&self, _instance: &ANodeInstance) -> Vec<ANodeConfigFieldDecl> {
        self.config_fields()
    }
    fn role_capabilities(&self) -> Vec<ANodeRoleCapability> {
        Vec::new()
    }
    fn supports_role(&self, role: SurfaceItemKind) -> bool {
        self.role_capabilities()
            .iter()
            .any(|capability| capability.role == role)
    }
    fn default_process_on_input_change_only(&self) -> bool {
        true
    }
    fn process_on_input_change_only_configurable(&self) -> bool {
        true
    }
    fn default_send_on_output_change_only(&self) -> bool {
        true
    }
    fn process_on_input_change_only(&self, instance: &ANodeInstance) -> bool {
        match instance.config.get(PROCESS_ON_INPUT_CHANGE_ONLY_CONFIG) {
            Some(RuntimeValue::Bool(value)) => *value,
            _ => self.default_process_on_input_change_only(),
        }
    }
    fn send_on_output_change_only(&self, instance: &ANodeInstance) -> bool {
        match instance.config.get(SEND_ON_OUTPUT_CHANGE_ONLY_CONFIG) {
            Some(RuntimeValue::Bool(value)) => *value,
            _ => self.default_send_on_output_change_only(),
        }
    }
    fn signature(&self, ctx: &SignatureCtx<'_>, instance: &ANodeInstance, bindings: &TypeBindings) -> ANodeSignature;
    fn state_layout(&self, _instance: &ANodeInstance, _resolved: &ResolvedANodeSignature) -> NodeStateLayout {
        match self.execution_kind() {
            ExecutionKind::Stateful => NodeStateLayout::RuntimeValues(1),
            _ => NodeStateLayout::Stateless,
        }
    }
    fn context_axes(&self, _instance: &ANodeInstance, _resolved: &ResolvedANodeSignature) -> AxisSet {
        AxisSet::new()
    }
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
