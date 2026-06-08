use crate::{
    ANodeDeclaration, ANodeInstance, ANodeRegistry, ANodeSignature, ANodeTypeId, CompiledNodeOperation, Diagnostic,
    ExecutionKind, InputSocketDecl, OutputSocketDecl, RegistryError, ResolvedANodeSignature, RuntimeValue,
    SignatureCtx, TypeBindingSource, TypeBindings, TypeConstraint, TypeVar, ValueTypeId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveNodeKind {
    Constant,
    Add,
    Compare,
    BoolAnd,
    BoolOr,
    BoolNot,
    Edge,
    Gate,
    MapRange,
    Clamp,
    DelayOneTick,
    DebugLog,
}

impl PrimitiveNodeKind {
    const ALL: [Self; 12] = [
        Self::Constant,
        Self::Add,
        Self::Compare,
        Self::BoolAnd,
        Self::BoolOr,
        Self::BoolNot,
        Self::Edge,
        Self::Gate,
        Self::MapRange,
        Self::Clamp,
        Self::DelayOneTick,
        Self::DebugLog,
    ];

    #[must_use]
    pub const fn type_name(self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::Add => "add",
            Self::Compare => "compare",
            Self::BoolAnd => "bool_and",
            Self::BoolOr => "bool_or",
            Self::BoolNot => "bool_not",
            Self::Edge => "edge",
            Self::Gate => "gate",
            Self::MapRange => "map_range",
            Self::Clamp => "clamp",
            Self::DelayOneTick => "delay_one_tick",
            Self::DebugLog => "debug_log",
        }
    }
}

pub struct PrimitiveNodeDeclaration {
    kind: PrimitiveNodeKind,
}

impl PrimitiveNodeDeclaration {
    #[must_use]
    pub const fn new(kind: PrimitiveNodeKind) -> Self {
        Self { kind }
    }

    #[must_use]
    pub const fn kind(&self) -> PrimitiveNodeKind {
        self.kind
    }
}

impl ANodeDeclaration for PrimitiveNodeDeclaration {
    fn type_id(&self) -> ANodeTypeId {
        ANodeTypeId::new(self.kind.type_name())
    }

    fn label(&self) -> &'static str {
        match self.kind {
            PrimitiveNodeKind::Constant => "Constant",
            PrimitiveNodeKind::Add => "Add",
            PrimitiveNodeKind::Compare => "Compare",
            PrimitiveNodeKind::BoolAnd => "Boolean And",
            PrimitiveNodeKind::BoolOr => "Boolean Or",
            PrimitiveNodeKind::BoolNot => "Boolean Not",
            PrimitiveNodeKind::Edge => "Edge",
            PrimitiveNodeKind::Gate => "Gate",
            PrimitiveNodeKind::MapRange => "Map Range",
            PrimitiveNodeKind::Clamp => "Clamp",
            PrimitiveNodeKind::DelayOneTick => "Delay One Tick",
            PrimitiveNodeKind::DebugLog => "Debug Log",
        }
    }

    fn category(&self) -> &'static str {
        match self.kind {
            PrimitiveNodeKind::Constant => "Values",
            PrimitiveNodeKind::Add | PrimitiveNodeKind::MapRange | PrimitiveNodeKind::Clamp => "Math",
            PrimitiveNodeKind::Compare => "Logic",
            PrimitiveNodeKind::BoolAnd
            | PrimitiveNodeKind::BoolOr
            | PrimitiveNodeKind::BoolNot
            | PrimitiveNodeKind::Edge
            | PrimitiveNodeKind::Gate
            | PrimitiveNodeKind::DelayOneTick => "Flow",
            PrimitiveNodeKind::DebugLog => "Debug",
        }
    }

    fn execution_kind(&self) -> ExecutionKind {
        match self.kind {
            PrimitiveNodeKind::Edge | PrimitiveNodeKind::DelayOneTick => ExecutionKind::Stateful,
            PrimitiveNodeKind::DebugLog => ExecutionKind::EffectEmitter,
            _ => ExecutionKind::Pure,
        }
    }

    fn breaks_dependency_cycle(&self) -> bool {
        self.kind == PrimitiveNodeKind::DelayOneTick
    }

    fn signature(&self, _ctx: &SignatureCtx<'_>, instance: &ANodeInstance, _bindings: &TypeBindings) -> ANodeSignature {
        match self.kind {
            PrimitiveNodeKind::Constant => constant_signature(instance),
            PrimitiveNodeKind::Add => generic_binary_signature("a", "b", "result"),
            PrimitiveNodeKind::Compare => compare_signature(),
            PrimitiveNodeKind::BoolAnd | PrimitiveNodeKind::BoolOr => boolean_binary_signature(),
            PrimitiveNodeKind::BoolNot => ANodeSignature {
                inputs: vec![InputSocketDecl::new("value", "Value", exact("bool"))],
                outputs: vec![OutputSocketDecl::new("result", "Result", exact("bool"))],
                ..ANodeSignature::default()
            },
            PrimitiveNodeKind::Edge => ANodeSignature {
                inputs: vec![InputSocketDecl::new("value", "Value", exact("bool"))],
                outputs: vec![OutputSocketDecl::new("trigger", "Trigger", exact("trigger"))],
                ..ANodeSignature::default()
            },
            PrimitiveNodeKind::Gate => ANodeSignature {
                inputs: vec![
                    InputSocketDecl::new("trigger", "Trigger", exact("trigger")),
                    InputSocketDecl::new("open", "Open", exact("bool")).with_default(RuntimeValue::Bool(true)),
                ],
                outputs: vec![OutputSocketDecl::new("trigger", "Trigger", exact("trigger"))],
                ..ANodeSignature::default()
            },
            PrimitiveNodeKind::MapRange => {
                float_signature(&["value", "in_min", "in_max", "out_min", "out_max"], "result")
            }
            PrimitiveNodeKind::Clamp => float_signature(&["value", "minimum", "maximum"], "result"),
            PrimitiveNodeKind::DelayOneTick => passthrough_signature(),
            PrimitiveNodeKind::DebugLog => ANodeSignature {
                inputs: vec![InputSocketDecl::new("value", "Value", TypeConstraint::Any)],
                outputs: Vec::new(),
                ..ANodeSignature::default()
            },
        }
    }

    fn compile_operation(
        &self,
        instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, Diagnostic> {
        Ok(match self.kind {
            PrimitiveNodeKind::Constant => CompiledNodeOperation::Constant(
                instance
                    .config
                    .get("value")
                    .cloned()
                    .unwrap_or(RuntimeValue::Float(0.0)),
            ),
            PrimitiveNodeKind::Add => CompiledNodeOperation::Add,
            PrimitiveNodeKind::Compare => CompiledNodeOperation::Compare,
            PrimitiveNodeKind::BoolAnd => CompiledNodeOperation::BoolAnd,
            PrimitiveNodeKind::BoolOr => CompiledNodeOperation::BoolOr,
            PrimitiveNodeKind::BoolNot => CompiledNodeOperation::BoolNot,
            PrimitiveNodeKind::Edge => CompiledNodeOperation::Edge,
            PrimitiveNodeKind::Gate => CompiledNodeOperation::Gate,
            PrimitiveNodeKind::MapRange => CompiledNodeOperation::MapRange,
            PrimitiveNodeKind::Clamp => CompiledNodeOperation::Clamp,
            PrimitiveNodeKind::DelayOneTick => CompiledNodeOperation::DelayOneTick,
            PrimitiveNodeKind::DebugLog => CompiledNodeOperation::DebugLog,
        })
    }
}

pub fn register_primitive_nodes(registry: &mut ANodeRegistry) -> Result<(), RegistryError> {
    for kind in PrimitiveNodeKind::ALL {
        registry.register(PrimitiveNodeDeclaration::new(kind))?;
    }
    Ok(())
}

#[must_use]
pub fn primitive_node_registry() -> ANodeRegistry {
    let mut registry = ANodeRegistry::default();
    register_primitive_nodes(&mut registry).expect("primitive ANode IDs must be unique");
    registry
}

fn exact(id: &str) -> TypeConstraint {
    TypeConstraint::Exact(ValueTypeId::new(id))
}

fn constant_signature(instance: &ANodeInstance) -> ANodeSignature {
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

fn generic_binary_signature(first: &str, second: &str, output: &str) -> ANodeSignature {
    let variable = TypeVar::new("TNumeric");
    let mut default_bindings = TypeBindings::default();
    let mut generic_constraints = indexmap::IndexMap::new();
    default_bindings.insert(variable.clone(), ValueTypeId::new("float"), TypeBindingSource::Default);
    generic_constraints.insert(variable.clone(), TypeConstraint::NumericLike);
    ANodeSignature {
        inputs: vec![
            InputSocketDecl::new(first, title(first), TypeConstraint::Generic(variable.clone())),
            InputSocketDecl::new(second, title(second), TypeConstraint::Generic(variable.clone())),
        ],
        outputs: vec![OutputSocketDecl::new(
            output,
            title(output),
            TypeConstraint::Generic(variable),
        )],
        default_bindings,
        generic_constraints,
    }
}

fn compare_signature() -> ANodeSignature {
    let variable = TypeVar::new("TValue");
    ANodeSignature {
        inputs: vec![
            InputSocketDecl::new("left", "Left", TypeConstraint::Generic(variable.clone())),
            InputSocketDecl::new("right", "Right", TypeConstraint::Generic(variable)),
        ],
        outputs: vec![OutputSocketDecl::new("result", "Result", exact("bool"))],
        ..ANodeSignature::default()
    }
}

fn boolean_binary_signature() -> ANodeSignature {
    ANodeSignature {
        inputs: vec![
            InputSocketDecl::new("a", "A", exact("bool")),
            InputSocketDecl::new("b", "B", exact("bool")),
        ],
        outputs: vec![OutputSocketDecl::new("result", "Result", exact("bool"))],
        ..ANodeSignature::default()
    }
}

fn float_signature(inputs: &[&str], output: &str) -> ANodeSignature {
    ANodeSignature {
        inputs: inputs
            .iter()
            .map(|id| InputSocketDecl::new(*id, title(id), exact("float")))
            .collect(),
        outputs: vec![OutputSocketDecl::new(output, title(output), exact("float"))],
        ..ANodeSignature::default()
    }
}

fn passthrough_signature() -> ANodeSignature {
    let variable = TypeVar::new("TValue");
    let mut default_bindings = TypeBindings::default();
    default_bindings.insert(variable.clone(), ValueTypeId::new("float"), TypeBindingSource::Default);
    ANodeSignature {
        inputs: vec![InputSocketDecl::new(
            "value",
            "Value",
            TypeConstraint::Generic(variable.clone()),
        )],
        outputs: vec![OutputSocketDecl::new(
            "value",
            "Value",
            TypeConstraint::Generic(variable),
        )],
        default_bindings,
        ..ANodeSignature::default()
    }
}

fn title(id: &str) -> String {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
