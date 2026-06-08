use std::collections::HashMap;

use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::{
    AEdge, ANodeId, ANodeRegistry, AlchemistGraph, Diagnostic, DiagnosticOrigin, DiagnosticSeverity, ExecutionKind,
    FacetId, InputSocketDecl, OutputSocketDecl, SignatureCtx, SocketId, ValueTypeId, ValueTypeRegistry,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct TypeVar(SmolStr);

impl TypeVar {
    #[must_use]
    pub fn new(value: impl Into<SmolStr>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for TypeVar {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TypeBindingSource {
    Default,
    InferredFromConnection,
    ForcedByModel,
    ForcedByUser,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeBinding {
    pub value_type: ValueTypeId,
    pub source: TypeBindingSource,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeBindings {
    values: IndexMap<TypeVar, TypeBinding>,
}

impl TypeBindings {
    pub fn insert(
        &mut self,
        variable: impl Into<TypeVar>,
        value_type: ValueTypeId,
        source: TypeBindingSource,
    ) -> Option<TypeBinding> {
        self.values.insert(variable.into(), TypeBinding { value_type, source })
    }

    pub fn bind(
        &mut self,
        variable: TypeVar,
        value_type: ValueTypeId,
        source: TypeBindingSource,
    ) -> Result<bool, TypeBindingConflict> {
        let Some(existing) = self.values.get(&variable) else {
            self.values.insert(variable, TypeBinding { value_type, source });
            return Ok(true);
        };
        if existing.value_type == value_type {
            if source > existing.source {
                self.values.insert(variable, TypeBinding { value_type, source });
                return Ok(true);
            }
            return Ok(false);
        }
        if source > existing.source {
            self.values.insert(variable, TypeBinding { value_type, source });
            return Ok(true);
        }
        if source == existing.source {
            return Err(TypeBindingConflict {
                variable,
                existing: existing.clone(),
                incoming: TypeBinding { value_type, source },
            });
        }
        Ok(false)
    }

    pub fn merge_from(&mut self, other: &Self) -> Result<bool, TypeBindingConflict> {
        let mut changed = false;
        for (variable, binding) in other.iter() {
            changed |= self.bind(variable.clone(), binding.value_type.clone(), binding.source)?;
        }
        Ok(changed)
    }

    #[must_use]
    pub fn get(&self, variable: &TypeVar) -> Option<&TypeBinding> {
        self.values.get(variable)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&TypeVar, &TypeBinding)> {
        self.values.iter()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeBindingConflict {
    pub variable: TypeVar,
    pub existing: TypeBinding,
    pub incoming: TypeBinding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TypeConstraint {
    Any,
    Exact(ValueTypeId),
    Facet(FacetId),
    Primitive,
    NumericLike,
    Generic(TypeVar),
    OneOf(Vec<TypeConstraint>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedSocket {
    pub id: SocketId,
    pub value_type: Option<ValueTypeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedANodeSignature {
    pub inputs: IndexMap<SocketId, ResolvedSocket>,
    pub outputs: IndexMap<SocketId, ResolvedSocket>,
}

#[derive(Clone, Debug)]
pub struct ResolvedANode {
    pub authored_id: ANodeId,
    pub execution_kind: ExecutionKind,
    pub bindings: TypeBindings,
    pub signature: ResolvedANodeSignature,
}

#[derive(Clone, Debug, Default)]
pub struct ResolvedGraph {
    pub nodes: IndexMap<ANodeId, ResolvedANode>,
}

#[derive(Clone, Debug, Default)]
pub struct TypeSolveResult {
    pub graph: ResolvedGraph,
    pub diagnostics: Vec<Diagnostic>,
}

impl TypeSolveResult {
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

pub struct TypeSolveCtx<'a> {
    pub value_types: &'a ValueTypeRegistry,
    pub nodes: &'a ANodeRegistry,
}

struct WorkingNode {
    execution_kind: ExecutionKind,
    signature: crate::ANodeSignature,
    bindings: TypeBindings,
}

#[must_use]
pub fn solve_types(graph: &AlchemistGraph, ctx: &TypeSolveCtx<'_>) -> TypeSolveResult {
    let mut diagnostics = Vec::new();
    let mut working = IndexMap::<ANodeId, WorkingNode>::new();
    let signature_ctx = SignatureCtx {
        value_types: ctx.value_types,
    };

    for (node_id, instance) in &graph.nodes {
        let Some(declaration) = ctx.nodes.get(&instance.type_id) else {
            diagnostics.push(Diagnostic::error(
                "missing_node_declaration",
                format!("ANode declaration `{}` is not registered", instance.type_id),
                DiagnosticOrigin::Node(*node_id),
            ));
            continue;
        };
        let signature = declaration.signature(&signature_ctx, instance, &instance.type_bindings);
        let mut bindings = signature.default_bindings.clone();
        merge_bindings(&mut bindings, &instance.type_bindings, *node_id, &mut diagnostics);
        merge_bindings(
            &mut bindings,
            &instance.forced_type_bindings,
            *node_id,
            &mut diagnostics,
        );
        working.insert(
            *node_id,
            WorkingNode {
                execution_kind: declaration.execution_kind(),
                signature,
                bindings,
            },
        );
    }

    let mut socket_types = HashMap::<SocketKey, ValueTypeId>::new();
    for _ in 0..=working.len() {
        let mut changed = false;
        for edge in &graph.edges {
            changed |= infer_edge(edge, &mut working, &mut socket_types);
        }
        if !changed {
            break;
        }
    }

    for (node_id, node) in &working {
        validate_generic_bindings(*node_id, node, ctx.value_types, &mut diagnostics);
    }
    for edge in &graph.edges {
        validate_edge(edge, &working, &socket_types, ctx.value_types, &mut diagnostics);
    }

    let nodes = working
        .into_iter()
        .map(|(node_id, node)| {
            let signature = ResolvedANodeSignature {
                inputs: node
                    .signature
                    .inputs
                    .iter()
                    .map(|socket| {
                        let value_type = resolved_input_type(node_id, socket, &node.bindings, &socket_types);
                        (
                            socket.id.clone(),
                            ResolvedSocket {
                                id: socket.id.clone(),
                                value_type,
                            },
                        )
                    })
                    .collect(),
                outputs: node
                    .signature
                    .outputs
                    .iter()
                    .map(|socket| {
                        let value_type = resolved_output_type(node_id, socket, &node.bindings, &socket_types);
                        (
                            socket.id.clone(),
                            ResolvedSocket {
                                id: socket.id.clone(),
                                value_type,
                            },
                        )
                    })
                    .collect(),
            };
            (
                node_id,
                ResolvedANode {
                    authored_id: node_id,
                    execution_kind: node.execution_kind,
                    bindings: node.bindings,
                    signature,
                },
            )
        })
        .collect();

    TypeSolveResult {
        graph: ResolvedGraph { nodes },
        diagnostics,
    }
}

fn merge_bindings(target: &mut TypeBindings, source: &TypeBindings, node: ANodeId, diagnostics: &mut Vec<Diagnostic>) {
    if let Err(conflict) = target.merge_from(source) {
        diagnostics.push(Diagnostic::error(
            "conflicting_type_binding",
            format!(
                "type variable `{}` is bound to both `{}` and `{}`",
                conflict.variable.as_str(),
                conflict.existing.value_type,
                conflict.incoming.value_type
            ),
            DiagnosticOrigin::Node(node),
        ));
    }
}

fn infer_edge(
    edge: &AEdge,
    working: &mut IndexMap<ANodeId, WorkingNode>,
    socket_types: &mut HashMap<SocketKey, ValueTypeId>,
) -> bool {
    let Some(output_constraint) = output_constraint(working, edge) else {
        return false;
    };
    let Some(input_constraint) = input_constraint(working, edge) else {
        return false;
    };

    let source_type = resolve_constraint(
        &output_constraint,
        &working[&edge.from.node].bindings,
        socket_types.get(&SocketKey::output(edge)),
    );
    let target_type = resolve_constraint(
        &input_constraint,
        &working[&edge.to.node].bindings,
        socket_types.get(&SocketKey::input(edge)),
    );
    let mut changed = false;

    if let (Some(value_type), TypeConstraint::Generic(variable)) = (&source_type, &input_constraint)
        && let Some(node) = working.get_mut(&edge.to.node)
    {
        changed |= node
            .bindings
            .bind(
                variable.clone(),
                value_type.clone(),
                TypeBindingSource::InferredFromConnection,
            )
            .unwrap_or(false);
    }
    if let (Some(value_type), TypeConstraint::Generic(variable)) = (&target_type, &output_constraint)
        && let Some(node) = working.get_mut(&edge.from.node)
    {
        changed |= node
            .bindings
            .bind(
                variable.clone(),
                value_type.clone(),
                TypeBindingSource::InferredFromConnection,
            )
            .unwrap_or(false);
    }
    if let Some(value_type) = source_type.as_ref()
        && target_type.is_none()
        && !matches!(input_constraint, TypeConstraint::Generic(_))
    {
        changed |= socket_types
            .insert(SocketKey::input(edge), value_type.clone())
            .is_none();
    }
    if let Some(value_type) = target_type.as_ref()
        && source_type.is_none()
        && !matches!(output_constraint, TypeConstraint::Generic(_))
    {
        changed |= socket_types
            .insert(SocketKey::output(edge), value_type.clone())
            .is_none();
    }
    changed
}

fn validate_generic_bindings(
    node_id: ANodeId,
    node: &WorkingNode,
    registry: &ValueTypeRegistry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (variable, constraint) in &node.signature.generic_constraints {
        let Some(binding) = node.bindings.get(variable) else {
            diagnostics.push(Diagnostic::error(
                "unresolved_type_variable",
                format!("type variable `{}` could not be resolved", variable.as_str()),
                DiagnosticOrigin::Node(node_id),
            ));
            continue;
        };
        if !constraint_accepts(constraint, &binding.value_type, registry) {
            diagnostics.push(Diagnostic::error(
                "type_constraint_failed",
                format!(
                    "type `{}` does not satisfy constraint for `{}`",
                    binding.value_type,
                    variable.as_str()
                ),
                DiagnosticOrigin::Node(node_id),
            ));
        }
    }
}

fn validate_edge(
    edge: &AEdge,
    working: &IndexMap<ANodeId, WorkingNode>,
    socket_types: &HashMap<SocketKey, ValueTypeId>,
    registry: &ValueTypeRegistry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(output_constraint) = output_constraint(working, edge) else {
        diagnostics.push(socket_error(edge.from.node, &edge.from.socket, "missing_output_socket"));
        return;
    };
    let Some(input_constraint) = input_constraint(working, edge) else {
        diagnostics.push(socket_error(edge.to.node, &edge.to.socket, "missing_input_socket"));
        return;
    };
    let Some(source_type) = resolve_constraint(
        &output_constraint,
        &working[&edge.from.node].bindings,
        socket_types.get(&SocketKey::output(edge)),
    ) else {
        diagnostics.push(socket_error(
            edge.from.node,
            &edge.from.socket,
            "unresolved_output_type",
        ));
        return;
    };
    let target_type = resolve_constraint(
        &input_constraint,
        &working[&edge.to.node].bindings,
        socket_types.get(&SocketKey::input(edge)),
    );

    let compatible = match &input_constraint {
        TypeConstraint::Facet(_) | TypeConstraint::Any | TypeConstraint::Primitive | TypeConstraint::NumericLike => {
            constraint_accepts(&input_constraint, &source_type, registry)
        }
        TypeConstraint::OneOf(_) => constraint_accepts(&input_constraint, &source_type, registry),
        TypeConstraint::Exact(_) | TypeConstraint::Generic(_) => target_type
            .as_ref()
            .is_some_and(|target| &source_type == target || registry.can_convert_automatically(&source_type, target)),
    };
    if !compatible {
        diagnostics.push(Diagnostic::error(
            "type_mismatch",
            format!(
                "cannot connect `{source_type}` to `{}`",
                target_type.map_or_else(|| "socket constraint".into(), |value| value.to_string())
            ),
            DiagnosticOrigin::Socket {
                node: edge.to.node,
                socket: edge.to.socket.clone(),
            },
        ));
    }
}

fn constraint_accepts(constraint: &TypeConstraint, value_type: &ValueTypeId, registry: &ValueTypeRegistry) -> bool {
    match constraint {
        TypeConstraint::Any => true,
        TypeConstraint::Exact(expected) => {
            expected == value_type || registry.can_convert_automatically(value_type, expected)
        }
        TypeConstraint::Facet(facet) => registry.supports_facet(value_type, facet),
        TypeConstraint::Primitive => is_primitive(value_type),
        TypeConstraint::NumericLike => is_numeric(value_type),
        TypeConstraint::Generic(_) => true,
        TypeConstraint::OneOf(constraints) => constraints
            .iter()
            .any(|constraint| constraint_accepts(constraint, value_type, registry)),
    }
}

fn is_primitive(value_type: &ValueTypeId) -> bool {
    matches!(
        value_type.as_str(),
        "unit" | "bool" | "trigger" | "int" | "float" | "string" | "vec2" | "vec3" | "color" | "duration"
    )
}

fn is_numeric(value_type: &ValueTypeId) -> bool {
    matches!(value_type.as_str(), "int" | "float" | "vec2" | "vec3" | "color")
}

fn output_constraint(working: &IndexMap<ANodeId, WorkingNode>, edge: &AEdge) -> Option<TypeConstraint> {
    working
        .get(&edge.from.node)?
        .signature
        .outputs
        .iter()
        .find(|socket| socket.id == edge.from.socket)
        .map(|socket| socket.constraint.clone())
}

fn input_constraint(working: &IndexMap<ANodeId, WorkingNode>, edge: &AEdge) -> Option<TypeConstraint> {
    working
        .get(&edge.to.node)?
        .signature
        .inputs
        .iter()
        .find(|socket| socket.id == edge.to.socket)
        .map(|socket| socket.constraint.clone())
}

fn resolve_constraint(
    constraint: &TypeConstraint,
    bindings: &TypeBindings,
    socket_type: Option<&ValueTypeId>,
) -> Option<ValueTypeId> {
    socket_type.cloned().or_else(|| match constraint {
        TypeConstraint::Exact(value_type) => Some(value_type.clone()),
        TypeConstraint::Generic(variable) => bindings.get(variable).map(|binding| binding.value_type.clone()),
        TypeConstraint::Any
        | TypeConstraint::Facet(_)
        | TypeConstraint::Primitive
        | TypeConstraint::NumericLike
        | TypeConstraint::OneOf(_) => None,
    })
}

fn resolved_input_type(
    node: ANodeId,
    socket: &InputSocketDecl,
    bindings: &TypeBindings,
    socket_types: &HashMap<SocketKey, ValueTypeId>,
) -> Option<ValueTypeId> {
    resolve_constraint(
        &socket.constraint,
        bindings,
        socket_types.get(&SocketKey {
            node,
            socket: socket.id.clone(),
            direction: SocketDirection::Input,
        }),
    )
}

fn resolved_output_type(
    node: ANodeId,
    socket: &OutputSocketDecl,
    bindings: &TypeBindings,
    socket_types: &HashMap<SocketKey, ValueTypeId>,
) -> Option<ValueTypeId> {
    resolve_constraint(
        &socket.constraint,
        bindings,
        socket_types.get(&SocketKey {
            node,
            socket: socket.id.clone(),
            direction: SocketDirection::Output,
        }),
    )
}

fn socket_error(node: ANodeId, socket: &SocketId, code: &str) -> Diagnostic {
    Diagnostic::error(
        code,
        format!("socket `{socket}` could not be resolved"),
        DiagnosticOrigin::Socket {
            node,
            socket: socket.clone(),
        },
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SocketDirection {
    Input,
    Output,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SocketKey {
    node: ANodeId,
    socket: SocketId,
    direction: SocketDirection,
}

impl SocketKey {
    fn input(edge: &AEdge) -> Self {
        Self {
            node: edge.to.node,
            socket: edge.to.socket.clone(),
            direction: SocketDirection::Input,
        }
    }

    fn output(edge: &AEdge) -> Self {
        Self {
            node: edge.from.node,
            socket: edge.from.socket.clone(),
            direction: SocketDirection::Output,
        }
    }
}
