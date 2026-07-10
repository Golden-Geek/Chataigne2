use crate::{
    ANodeDeclaration, ANodeId, ANodeInstance, ANodeRegistry, ANodeRoleCapability, ANodeTypeId, AlchemistGraph,
    AutoWirePolicy, ContextAxisId, FormulaPropertySchema, GraphEditError, InputSocketRef, ManagedRegionDefinition,
    ManagedRegionInstance, ManagedRegionKind, OutputSocketRef, PipelineCardinality, SignatureCtx, SurfaceItemKind,
    TypeBindings, TypeConstraint, ValueTypeId, ValueTypeRegistry,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PipelineShape {
    Single {
        value_type: ValueTypeId,
    },
    ValueSet {
        item_type: ValueTypeId,
        axis: Option<ContextAxisId>,
    },
    Trigger,
    CommandIntent,
    Unknown,
}

#[derive(Clone, Copy)]
pub struct PipelineShapeCheckItem<'a> {
    pub declaration: &'a dyn ANodeDeclaration,
    pub instance: &'a ANodeInstance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineShapeStep {
    pub item_index: usize,
    pub node_type: ANodeTypeId,
    pub input: PipelineShape,
    pub output: PipelineShape,
    pub cardinality: PipelineCardinality,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineShapeDiagnostic {
    pub item_index: usize,
    pub node_type: ANodeTypeId,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineShapeResult {
    pub final_shape: PipelineShape,
    pub steps: Vec<PipelineShapeStep>,
    pub diagnostics: Vec<PipelineShapeDiagnostic>,
}

impl PipelineShapeResult {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

pub fn check_filter_pipeline_shapes<'a>(
    initial_shape: PipelineShape,
    items: impl IntoIterator<Item = PipelineShapeCheckItem<'a>>,
    ctx: &SignatureCtx<'_>,
) -> PipelineShapeResult {
    let mut current_shape = initial_shape;
    let mut steps = Vec::new();
    let mut diagnostics = Vec::new();

    for (item_index, item) in items.into_iter().enumerate() {
        let node_type = item.declaration.type_id();
        let Some(capability) = item
            .declaration
            .role_capabilities()
            .into_iter()
            .find(|capability| capability.role == SurfaceItemKind::Filter)
        else {
            diagnostics.push(PipelineShapeDiagnostic {
                item_index,
                node_type,
                message: "ANode is not filter-capable".to_string(),
            });
            continue;
        };

        match transition_shape(&current_shape, item, &capability, ctx) {
            Ok(output) => {
                steps.push(PipelineShapeStep {
                    item_index,
                    node_type,
                    input: current_shape,
                    output: output.clone(),
                    cardinality: capability.cardinality,
                });
                current_shape = output;
            }
            Err(message) => diagnostics.push(PipelineShapeDiagnostic {
                item_index,
                node_type,
                message,
            }),
        }
    }

    PipelineShapeResult {
        final_shape: current_shape,
        steps,
        diagnostics,
    }
}

pub struct PipelineLoweringCtx<'a> {
    pub value_types: &'a ValueTypeRegistry,
    pub nodes: &'a ANodeRegistry,
    pub properties: Option<&'a FormulaPropertySchema>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PipelineLoweringDiagnostic {
    pub kind: PipelineLoweringDiagnosticKind,
    pub item_index: Option<usize>,
    pub node_type: Option<ANodeTypeId>,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineLoweringDiagnosticKind {
    InvalidRegionKind,
    RegionInstanceMismatch,
    RegionRoleMismatch,
    MissingBoundarySockets,
    MissingDeclaration,
    NotFilterCapable,
    MissingLinearAutowire,
    UnsupportedValueSetElementwise,
    UnsupportedValueSetAggregate,
    UnsupportedValueSetReshape,
    UnsupportedValueSetExpand,
    GraphEdit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FilterPipelineLoweringResult {
    pub graph: AlchemistGraph,
    pub shape: PipelineShapeResult,
    pub inserted_nodes: Vec<ANodeId>,
    pub diagnostics: Vec<PipelineLoweringDiagnostic>,
}

impl FilterPipelineLoweringResult {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.shape.is_valid() && self.diagnostics.is_empty()
    }
}

struct ResolvedPipelineItem<'a> {
    item_index: usize,
    instance: &'a ANodeInstance,
    declaration: &'a dyn ANodeDeclaration,
    capability: Option<ANodeRoleCapability>,
}

pub fn lower_filter_pipeline_region(
    graph: &AlchemistGraph,
    definition: &ManagedRegionDefinition,
    instance: &ManagedRegionInstance,
    initial_shape: PipelineShape,
    ctx: &PipelineLoweringCtx<'_>,
) -> FilterPipelineLoweringResult {
    let mut diagnostics = validate_filter_pipeline_region(definition, instance);
    let signature_ctx = SignatureCtx {
        value_types: ctx.value_types,
        properties: ctx.properties,
    };
    let resolved_items = resolve_filter_pipeline_items(instance, ctx.nodes, &mut diagnostics);
    let shape = check_filter_pipeline_shapes(
        initial_shape,
        resolved_items.iter().map(|item| PipelineShapeCheckItem {
            declaration: item.declaration,
            instance: item.instance,
        }),
        &signature_ctx,
    );

    if !diagnostics.is_empty() || !shape.is_valid() {
        return FilterPipelineLoweringResult {
            graph: graph.clone(),
            shape,
            inserted_nodes: Vec::new(),
            diagnostics,
        };
    }
    diagnostics.extend(unsupported_shape_lowering_diagnostics(&shape));
    if !diagnostics.is_empty() {
        return FilterPipelineLoweringResult {
            graph: graph.clone(),
            shape,
            inserted_nodes: Vec::new(),
            diagnostics,
        };
    }

    let mut draft = graph.clone();
    let mut inserted_nodes = Vec::new();
    let mut edit_diagnostics = Vec::new();

    if let (Some(input_socket), Some(output_socket)) = (&definition.input_socket, &definition.output_socket) {
        let mut previous_output = OutputSocketRef::new(input_socket.node, input_socket.socket.clone());

        for item in &resolved_items {
            let Some(capability) = &item.capability else {
                edit_diagnostics.push(PipelineLoweringDiagnostic {
                    kind: PipelineLoweringDiagnosticKind::NotFilterCapable,
                    item_index: Some(item.item_index),
                    node_type: Some(item.instance.type_id.clone()),
                    message: "ANode is not filter-capable".to_string(),
                });
                continue;
            };
            let Some((input, output)) = linear_filter_sockets(capability) else {
                edit_diagnostics.push(PipelineLoweringDiagnostic {
                    kind: PipelineLoweringDiagnosticKind::MissingLinearAutowire,
                    item_index: Some(item.item_index),
                    node_type: Some(item.instance.type_id.clone()),
                    message: "filter item does not declare linear autowire sockets".to_string(),
                });
                continue;
            };

            let node = item.instance.clone();
            let node_id = node.id;
            if let Err(error) = draft.add_node(node) {
                edit_diagnostics.push(graph_edit_diagnostic(
                    Some(item.item_index),
                    Some(item.instance.type_id.clone()),
                    error,
                ));
                continue;
            }
            inserted_nodes.push(node_id);

            if let Err(error) = draft.connect(previous_output.clone(), InputSocketRef::new(node_id, input.clone())) {
                edit_diagnostics.push(graph_edit_diagnostic(
                    Some(item.item_index),
                    Some(item.instance.type_id.clone()),
                    error,
                ));
                break;
            }
            previous_output = OutputSocketRef::new(node_id, output.clone());
        }

        if edit_diagnostics.is_empty() {
            if let Err(error) = draft.connect(
                previous_output,
                InputSocketRef::new(output_socket.node, output_socket.socket.clone()),
            ) {
                edit_diagnostics.push(graph_edit_diagnostic(None, None, error));
            }
        }
    }

    if edit_diagnostics.is_empty() {
        FilterPipelineLoweringResult {
            graph: draft,
            shape,
            inserted_nodes,
            diagnostics,
        }
    } else {
        FilterPipelineLoweringResult {
            graph: graph.clone(),
            shape,
            inserted_nodes: Vec::new(),
            diagnostics: edit_diagnostics,
        }
    }
}

fn unsupported_shape_lowering_diagnostics(shape: &PipelineShapeResult) -> Vec<PipelineLoweringDiagnostic> {
    shape
        .steps
        .iter()
        .filter(|step| {
            step.cardinality != PipelineCardinality::WholeSet
                && (matches!(step.input, PipelineShape::ValueSet { .. })
                    || matches!(step.output, PipelineShape::ValueSet { .. }))
        })
        .map(|step| PipelineLoweringDiagnostic {
            kind: match step.cardinality {
                PipelineCardinality::Elementwise => PipelineLoweringDiagnosticKind::UnsupportedValueSetElementwise,
                PipelineCardinality::Aggregate => PipelineLoweringDiagnosticKind::UnsupportedValueSetAggregate,
                PipelineCardinality::Reshape => PipelineLoweringDiagnosticKind::UnsupportedValueSetReshape,
                PipelineCardinality::Expand => PipelineLoweringDiagnosticKind::UnsupportedValueSetExpand,
                PipelineCardinality::WholeSet => unreachable!("whole-set steps are filtered out"),
            },
            item_index: Some(step.item_index),
            node_type: Some(step.node_type.clone()),
            message: match step.cardinality {
                PipelineCardinality::Elementwise => {
                    "elementwise ValueSet lowering requires lane-aware MapEach support".to_string()
                }
                PipelineCardinality::Aggregate => {
                    "aggregate ValueSet lowering requires an explicit reduction strategy".to_string()
                }
                PipelineCardinality::Reshape => {
                    "reshape ValueSet lowering requires an explicit projection strategy".to_string()
                }
                PipelineCardinality::Expand => {
                    "expand lowering to ValueSet requires an explicit broadcast strategy".to_string()
                }
                PipelineCardinality::WholeSet => unreachable!("whole-set steps are filtered out"),
            },
        })
        .collect()
}

fn validate_filter_pipeline_region(
    definition: &ManagedRegionDefinition,
    instance: &ManagedRegionInstance,
) -> Vec<PipelineLoweringDiagnostic> {
    let mut diagnostics = Vec::new();

    if definition.kind != ManagedRegionKind::FilterPipeline {
        diagnostics.push(PipelineLoweringDiagnostic {
            kind: PipelineLoweringDiagnosticKind::InvalidRegionKind,
            item_index: None,
            node_type: None,
            message: "managed region is not a filter pipeline".to_string(),
        });
    }
    if definition.id != instance.region_id {
        diagnostics.push(PipelineLoweringDiagnostic {
            kind: PipelineLoweringDiagnosticKind::RegionInstanceMismatch,
            item_index: None,
            node_type: None,
            message: format!(
                "managed region instance `{}` does not match definition `{}`",
                instance.region_id, definition.id
            ),
        });
    }
    if !definition.accepted_roles.contains(&SurfaceItemKind::Filter) {
        diagnostics.push(PipelineLoweringDiagnostic {
            kind: PipelineLoweringDiagnosticKind::RegionRoleMismatch,
            item_index: None,
            node_type: None,
            message: "managed region does not accept filter items".to_string(),
        });
    }
    if definition.input_socket.is_none() || definition.output_socket.is_none() {
        diagnostics.push(PipelineLoweringDiagnostic {
            kind: PipelineLoweringDiagnosticKind::MissingBoundarySockets,
            item_index: None,
            node_type: None,
            message: "filter pipeline region must declare input and output boundary sockets".to_string(),
        });
    }

    diagnostics
}

fn resolve_filter_pipeline_items<'a>(
    instance: &'a ManagedRegionInstance,
    nodes: &'a ANodeRegistry,
    diagnostics: &mut Vec<PipelineLoweringDiagnostic>,
) -> Vec<ResolvedPipelineItem<'a>> {
    let mut resolved = Vec::new();

    for (item_index, item) in instance.items.iter().enumerate() {
        if !item.enabled {
            continue;
        }
        let Some(declaration) = nodes.get(&item.anode.type_id) else {
            diagnostics.push(PipelineLoweringDiagnostic {
                kind: PipelineLoweringDiagnosticKind::MissingDeclaration,
                item_index: Some(item_index),
                node_type: Some(item.anode.type_id.clone()),
                message: "ANode declaration is not registered".to_string(),
            });
            continue;
        };
        let capability = declaration
            .role_capabilities()
            .into_iter()
            .find(|capability| capability.role == SurfaceItemKind::Filter);

        resolved.push(ResolvedPipelineItem {
            item_index,
            instance: &item.anode,
            declaration: declaration.as_ref(),
            capability,
        });
    }

    resolved
}

fn linear_filter_sockets(capability: &ANodeRoleCapability) -> Option<(crate::SocketId, crate::SocketId)> {
    match &capability.autowire {
        AutoWirePolicy::UnaryTransform { input, output } => Some((input.clone(), output.clone())),
        AutoWirePolicy::Gate { input, output, .. } => Some((input.clone(), output.clone())),
        AutoWirePolicy::None => capability.primary_input.clone().zip(capability.primary_output.clone()),
        AutoWirePolicy::Source { .. } | AutoWirePolicy::Sink { .. } => None,
    }
}

fn graph_edit_diagnostic(
    item_index: Option<usize>,
    node_type: Option<ANodeTypeId>,
    error: GraphEditError,
) -> PipelineLoweringDiagnostic {
    PipelineLoweringDiagnostic {
        kind: PipelineLoweringDiagnosticKind::GraphEdit,
        item_index,
        node_type,
        message: error.to_string(),
    }
}

fn transition_shape(
    input: &PipelineShape,
    item: PipelineShapeCheckItem<'_>,
    capability: &ANodeRoleCapability,
    ctx: &SignatureCtx<'_>,
) -> Result<PipelineShape, String> {
    match capability.cardinality {
        PipelineCardinality::WholeSet => Ok(input.clone()),
        PipelineCardinality::Elementwise => elementwise_shape(input, item, capability, ctx),
        PipelineCardinality::Aggregate => aggregate_shape(input, item, capability, ctx),
        PipelineCardinality::Reshape => reshape_shape(input, item, capability, ctx),
        PipelineCardinality::Expand => expand_shape(input, item, capability, ctx),
    }
}

fn elementwise_shape(
    input: &PipelineShape,
    item: PipelineShapeCheckItem<'_>,
    capability: &ANodeRoleCapability,
    ctx: &SignatureCtx<'_>,
) -> Result<PipelineShape, String> {
    match input {
        PipelineShape::Single { value_type } => Ok(PipelineShape::Single {
            value_type: output_value_type(item, capability, ctx, Some(value_type))
                .unwrap_or_else(|| value_type.clone()),
        }),
        PipelineShape::ValueSet { item_type, axis } => Ok(PipelineShape::ValueSet {
            item_type: output_value_type(item, capability, ctx, Some(item_type)).unwrap_or_else(|| item_type.clone()),
            axis: axis.clone(),
        }),
        PipelineShape::Unknown => Ok(PipelineShape::Unknown),
        PipelineShape::Trigger | PipelineShape::CommandIntent => {
            Err("elementwise filters require single values or ValueSet items".to_string())
        }
    }
}

fn aggregate_shape(
    input: &PipelineShape,
    item: PipelineShapeCheckItem<'_>,
    capability: &ANodeRoleCapability,
    ctx: &SignatureCtx<'_>,
) -> Result<PipelineShape, String> {
    match input {
        PipelineShape::Single { value_type }
        | PipelineShape::ValueSet {
            item_type: value_type, ..
        } => Ok(PipelineShape::Single {
            value_type: output_value_type(item, capability, ctx, Some(value_type))
                .unwrap_or_else(|| value_type.clone()),
        }),
        PipelineShape::Unknown => Ok(PipelineShape::Unknown),
        PipelineShape::Trigger | PipelineShape::CommandIntent => {
            Err("aggregate filters require single values or ValueSet items".to_string())
        }
    }
}

fn reshape_shape(
    input: &PipelineShape,
    item: PipelineShapeCheckItem<'_>,
    capability: &ANodeRoleCapability,
    ctx: &SignatureCtx<'_>,
) -> Result<PipelineShape, String> {
    match input {
        PipelineShape::Single { value_type }
        | PipelineShape::ValueSet {
            item_type: value_type, ..
        } => output_value_type(item, capability, ctx, Some(value_type))
            .map(|value_type| PipelineShape::Single { value_type })
            .ok_or_else(|| "reshape filters must declare a resolvable primary output shape".to_string()),
        PipelineShape::Unknown => Ok(PipelineShape::Unknown),
        PipelineShape::Trigger | PipelineShape::CommandIntent => {
            Err("reshape filters require single values or ValueSet items".to_string())
        }
    }
}

fn expand_shape(
    input: &PipelineShape,
    item: PipelineShapeCheckItem<'_>,
    capability: &ANodeRoleCapability,
    ctx: &SignatureCtx<'_>,
) -> Result<PipelineShape, String> {
    match input {
        PipelineShape::Single { value_type } => Ok(PipelineShape::ValueSet {
            item_type: output_value_type(item, capability, ctx, Some(value_type)).unwrap_or_else(|| value_type.clone()),
            axis: None,
        }),
        PipelineShape::ValueSet { .. } => Err("expand filters require an explicit single-value input".to_string()),
        PipelineShape::Unknown => Ok(PipelineShape::Unknown),
        PipelineShape::Trigger | PipelineShape::CommandIntent => {
            Err("expand filters require a single value input".to_string())
        }
    }
}

fn output_value_type(
    item: PipelineShapeCheckItem<'_>,
    capability: &ANodeRoleCapability,
    ctx: &SignatureCtx<'_>,
    fallback: Option<&ValueTypeId>,
) -> Option<ValueTypeId> {
    let output_id = capability.primary_output.as_ref()?;
    let signature = item
        .declaration
        .signature(ctx, item.instance, &item.instance.type_bindings);
    let output = signature.outputs.iter().find(|output| &output.id == output_id)?;

    let mut bindings = signature.default_bindings.clone();
    let _ = bindings.merge_from(&item.instance.type_bindings);
    resolve_constraint(&output.constraint, &bindings, fallback)
}

fn resolve_constraint(
    constraint: &TypeConstraint,
    bindings: &TypeBindings,
    fallback: Option<&ValueTypeId>,
) -> Option<ValueTypeId> {
    match constraint {
        TypeConstraint::Exact(value_type) => Some(value_type.clone()),
        TypeConstraint::Generic(variable) => bindings
            .get(variable)
            .map(|binding| binding.value_type.clone())
            .or_else(|| fallback.cloned()),
        TypeConstraint::OneOf(options) => options
            .iter()
            .find_map(|option| resolve_constraint(option, bindings, fallback))
            .or_else(|| fallback.cloned()),
        TypeConstraint::Any | TypeConstraint::Facet(_) | TypeConstraint::Primitive | TypeConstraint::NumericLike => {
            fallback.cloned()
        }
    }
}

pub fn single_shape(value_type: impl Into<ValueTypeId>) -> PipelineShape {
    PipelineShape::Single {
        value_type: value_type.into(),
    }
}

pub fn value_set_shape(item_type: impl Into<ValueTypeId>, axis: Option<ContextAxisId>) -> PipelineShape {
    PipelineShape::ValueSet {
        item_type: item_type.into(),
        axis,
    }
}
