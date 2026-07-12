use std::{
    collections::{VecDeque, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
};

use indexmap::IndexMap;

use crate::{
    ANodeId, ANodeRegistry, AlchemistFormula, AlchemistGraph, AlchemistGraphId, AxisSet, CompiledNodeEvaluator,
    Diagnostic, DiagnosticOrigin, DiagnosticSeverity, ExecNodeId, ExecutionKind, ExposedSurface, FormulaId,
    FormulaPropertyId, FormulaPropertySchema, FormulaPropertySlotId, FormulaRef, ResolvedANodeSignature, RuntimeValue,
    SocketId, StableRef, TypeSolveCtx, ValueComponent, ValueSlotId, ValueTypeId, ValueTypeRegistry,
    component_value_type, solve_types,
};

pub const FORMULA_INPUT_VALUE_TYPE: &str = "alchemist.formula_input";

#[must_use]
pub fn formula_input_value_ref(graph_id: AlchemistGraphId, node_id: ANodeId, socket: &SocketId) -> StableRef {
    StableRef::new(
        ValueTypeId::new(FORMULA_INPUT_VALUE_TYPE),
        format!("{graph_id}:{node_id}:{}", socket.as_str()),
    )
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputValueSource {
    Slot(ValueSlotId),
    Converted {
        source: Box<InputValueSource>,
        target_type: ValueTypeId,
    },
    Component {
        source: Box<InputValueSource>,
        component: ValueComponent,
    },
    Composite {
        target_type: ValueTypeId,
        base: Box<InputValueSource>,
        components: Vec<(ValueComponent, InputValueSource)>,
    },
    RuntimeInput {
        reference: StableRef,
        fallback: Box<InputValueSource>,
    },
    Constant(RuntimeValue),
    Unset,
}

#[derive(Clone, Debug)]
pub enum CompiledNodeOperation {
    Disabled { outputs: Vec<DisabledOutput> },
    Constant(RuntimeValue),
    ReadProperty(FormulaPropertySlotId),
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
    Custom(Arc<dyn CompiledNodeEvaluator>),
}

#[derive(Clone, Debug)]
pub struct DisabledOutput {
    pub input_index: Option<usize>,
    pub default_value: RuntimeValue,
}

#[derive(Clone, Debug)]
pub struct CompiledExecNode {
    pub exec_id: ExecNodeId,
    pub authored_id: ANodeId,
    pub execution_kind: ExecutionKind,
    pub operation: CompiledNodeOperation,
    pub inputs: Vec<InputValueSource>,
    pub outputs: Vec<ValueSlotId>,
    pub output_sockets: Vec<SocketId>,
    pub output_types: Vec<Option<ValueTypeId>>,
    pub state_range: Range<usize>,
    pub process_on_input_change_only: bool,
    pub send_on_output_change_only: bool,
    pub log_enabled: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeStateLayout {
    pub ranges: Vec<Range<usize>>,
    pub state_slot_count: usize,
    pub value_slot_count: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledFormulaProperty {
    pub id: FormulaPropertyId,
    pub slot: FormulaPropertySlotId,
    pub value_type: ValueTypeId,
    pub default_value: RuntimeValue,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompiledFormulaPropertySchema {
    pub properties: IndexMap<FormulaPropertyId, CompiledFormulaProperty>,
}

impl CompiledFormulaPropertySchema {
    #[must_use]
    pub fn get(&self, id: &FormulaPropertyId) -> Option<&CompiledFormulaProperty> {
        self.properties.get(id)
    }

    #[must_use]
    pub fn get_slot(&self, slot: FormulaPropertySlotId) -> Option<&CompiledFormulaProperty> {
        self.properties.values().find(|property| property.slot == slot)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&FormulaPropertyId, &CompiledFormulaProperty)> {
        self.properties.iter()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSubscription {
    pub exec_node: ExecNodeId,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputRoute {
    pub exec_node: ExecNodeId,
    pub socket_index: usize,
}

#[derive(Clone, Debug, Default)]
pub struct DebugSourceMap {
    pub exec_to_authored: Vec<ANodeId>,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeDependencyGraph {
    pub slot_dependents: Vec<Vec<ExecNodeId>>,
    pub external_input_nodes: Vec<ExecNodeId>,
    pub always_process_nodes: Vec<ExecNodeId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormulaAnalysis {
    pub has_stateful_nodes: bool,
    pub has_effect_emitters: bool,
    pub has_always_process_nodes: bool,
    pub has_input_gated_nodes: bool,
    pub explicit_context_axes: AxisSet,
    pub state_axes: AxisSet,
    pub effect_axes: AxisSet,
    pub state_slot_count: usize,
    pub value_slot_count: usize,
}

#[derive(Clone, Debug)]
pub struct CompiledAlchemistGraph {
    pub exec_nodes: Vec<CompiledExecNode>,
    pub topo_order: Vec<ExecNodeId>,
    pub properties: CompiledFormulaPropertySchema,
    pub analysis: FormulaAnalysis,
    pub state_layout: RuntimeStateLayout,
    pub output_routes: Vec<OutputRoute>,
    pub subscriptions: Vec<RuntimeSubscription>,
    pub dependencies: RuntimeDependencyGraph,
    pub debug_map: DebugSourceMap,
}

#[derive(Clone, Debug, Default)]
pub struct CompileResult {
    pub compiled: Option<Arc<CompiledAlchemistGraph>>,
    pub diagnostics: Vec<Diagnostic>,
}

impl CompileResult {
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }
}

#[derive(Clone, Debug)]
pub struct CompiledAlchemistFormula {
    pub formula_ref: FormulaRef,
    pub graph: Arc<CompiledAlchemistGraph>,
    pub properties: CompiledFormulaPropertySchema,
    pub analysis: FormulaAnalysis,
    pub diagnostics: Vec<Diagnostic>,
}

impl CompiledAlchemistFormula {
    #[must_use]
    pub fn new(formula_ref: FormulaRef, graph: Arc<CompiledAlchemistGraph>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            formula_ref,
            properties: graph.properties.clone(),
            analysis: graph.analysis.clone(),
            graph,
            diagnostics,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FormulaCompileKey {
    pub formula_id: FormulaId,
    pub formula_version: u32,
    pub graph_revision: u64,
    pub property_schema_hash: u64,
    pub node_registry_hash: u64,
    pub value_type_registry_hash: u64,
}

impl FormulaCompileKey {
    #[must_use]
    pub fn from_formula(
        formula: &AlchemistFormula,
        graph_revision: u64,
        node_registry_hash: u64,
        value_type_registry_hash: u64,
    ) -> Self {
        Self {
            formula_id: formula.id.clone(),
            formula_version: formula.version,
            graph_revision,
            property_schema_hash: property_schema_hash(&formula.properties),
            node_registry_hash,
            value_type_registry_hash,
        }
    }
}

fn property_schema_hash(schema: &FormulaPropertySchema) -> u64 {
    let mut hasher = DefaultHasher::new();
    for (id, declaration) in schema.iter() {
        id.hash(&mut hasher);
        declaration.label.hash(&mut hasher);
        declaration.description.hash(&mut hasher);
        declaration.value_type.hash(&mut hasher);
        format!("{:?}", declaration.default_value).hash(&mut hasher);
    }
    hasher.finish()
}

pub struct CompileCtx<'a> {
    pub value_types: &'a ValueTypeRegistry,
    pub nodes: &'a ANodeRegistry,
    pub properties: Option<&'a FormulaPropertySchema>,
}

#[must_use]
pub fn compile_graph(graph: &AlchemistGraph, ctx: &CompileCtx<'_>) -> CompileResult {
    let mut diagnostics = Vec::new();
    let compiled_properties = compile_property_schema(ctx.properties, ctx.value_types, &mut diagnostics);
    let solved = solve_types(
        graph,
        &TypeSolveCtx {
            value_types: ctx.value_types,
            nodes: ctx.nodes,
            properties: ctx.properties,
        },
    );
    diagnostics.extend(solved.diagnostics);
    validate_exposed_surface(&graph.exposed, graph, &mut diagnostics);
    if has_errors(&diagnostics) {
        return CompileResult {
            compiled: None,
            diagnostics,
        };
    }

    let mut value_slots = IndexMap::<(ANodeId, SocketId), ValueSlotId>::new();
    let mut next_value_slot = 0_u32;
    for (node_id, resolved) in &solved.graph.nodes {
        for socket in resolved.signature.outputs.keys() {
            value_slots.insert((*node_id, socket.clone()), ValueSlotId::new(next_value_slot));
            next_value_slot += 1;
        }
    }

    let topo_nodes = match topological_order(graph, ctx.nodes) {
        Ok(order) => order,
        Err(cycle_nodes) => {
            diagnostics.push(Diagnostic::error(
                "cycle_without_delay",
                format!("graph cycle involves {} node(s)", cycle_nodes.len()),
                DiagnosticOrigin::Graph,
            ));
            return CompileResult {
                compiled: None,
                diagnostics,
            };
        }
    };
    let authored_to_exec: IndexMap<ANodeId, ExecNodeId> = topo_nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (*node, ExecNodeId::new(index as u32)))
        .collect();
    let topo_order = topo_nodes.iter().map(|node| authored_to_exec[node]).collect::<Vec<_>>();

    let mut state_slot_count = 0_usize;
    let mut ranges = vec![0..0; graph.nodes.len()];
    let mut exec_nodes = (0..graph.nodes.len()).map(|_| None).collect::<Vec<_>>();
    let mut direct_context_axes = vec![AxisSet::new(); graph.nodes.len()];
    for (node_id, instance) in &graph.nodes {
        let exec_id = authored_to_exec[node_id];
        let resolved = &solved.graph.nodes[node_id];
        let declaration = ctx
            .nodes
            .get(&instance.type_id)
            .expect("type solving guarantees a registered declaration");
        let state_size = if instance.enabled {
            declaration.state_layout(instance, &resolved.signature).slot_count()
        } else {
            0
        };
        if instance.enabled {
            direct_context_axes[exec_id.index()] = declaration.context_axes(instance, &resolved.signature);
        }
        let state_range = state_slot_count..state_slot_count + state_size;
        state_slot_count += state_size;
        ranges[exec_id.index()] = state_range.clone();

        let inputs = resolved
            .signature
            .inputs
            .iter()
            .map(|(socket, resolved_socket)| {
                input_source(
                    graph,
                    &solved.graph,
                    &value_slots,
                    instance,
                    *node_id,
                    socket,
                    resolved_socket.value_type.as_ref(),
                    ctx,
                )
            })
            .collect();
        let mut outputs = Vec::with_capacity(resolved.signature.outputs.len());
        let mut output_sockets = Vec::with_capacity(resolved.signature.outputs.len());
        let mut output_types = Vec::with_capacity(resolved.signature.outputs.len());
        for (socket, resolved_socket) in &resolved.signature.outputs {
            outputs.push(value_slots[&(*node_id, socket.clone())]);
            output_sockets.push(socket.clone());
            output_types.push(resolved_socket.value_type.clone());
        }
        let operation = if instance.enabled {
            match compile_node_operation(instance, &resolved.signature, ctx, &compiled_properties) {
                Ok(operation) => operation,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            }
        } else {
            disabled_operation(&resolved.signature, ctx.value_types)
        };
        exec_nodes[exec_id.index()] = Some(CompiledExecNode {
            exec_id,
            authored_id: *node_id,
            execution_kind: resolved.execution_kind,
            operation,
            inputs,
            outputs,
            output_sockets,
            output_types,
            state_range,
            process_on_input_change_only: declaration.process_on_input_change_only(instance),
            send_on_output_change_only: declaration.send_on_output_change_only(instance),
            log_enabled: instance.enabled && matches!(instance.config.get("log"), Some(RuntimeValue::Bool(true))),
        });
    }
    if has_errors(&diagnostics) {
        return CompileResult {
            compiled: None,
            diagnostics,
        };
    }

    let exec_nodes = exec_nodes
        .into_iter()
        .map(|node| node.expect("type solving and compilation produce one node per exec id"))
        .collect::<Vec<_>>();
    let debug_map = DebugSourceMap {
        exec_to_authored: exec_nodes.iter().map(|node| node.authored_id).collect(),
    };
    let analysis = analyze_formula(
        &exec_nodes,
        &topo_order,
        state_slot_count,
        next_value_slot as usize,
        &direct_context_axes,
    );
    let dependencies = runtime_dependencies(&exec_nodes, next_value_slot as usize);
    CompileResult {
        compiled: Some(Arc::new(CompiledAlchemistGraph {
            exec_nodes,
            topo_order,
            properties: compiled_properties,
            analysis,
            state_layout: RuntimeStateLayout {
                ranges,
                state_slot_count,
                value_slot_count: next_value_slot as usize,
            },
            output_routes: Vec::new(),
            subscriptions: Vec::new(),
            dependencies,
            debug_map,
        })),
        diagnostics,
    }
}

fn runtime_dependencies(exec_nodes: &[CompiledExecNode], value_slot_count: usize) -> RuntimeDependencyGraph {
    let mut slot_dependents = vec![Vec::new(); value_slot_count];
    let mut external_input_nodes = Vec::new();
    let mut always_process_nodes = Vec::new();

    for node in exec_nodes {
        if !node.process_on_input_change_only {
            always_process_nodes.push(node.exec_id);
        }
        let mut has_external_input = matches!(
            node.operation,
            CompiledNodeOperation::ReadProperty(_) | CompiledNodeOperation::Custom(_)
        );
        for source in &node.inputs {
            collect_runtime_dependencies(source, node.exec_id, &mut slot_dependents, &mut has_external_input);
        }
        if has_external_input {
            external_input_nodes.push(node.exec_id);
        }
    }

    RuntimeDependencyGraph {
        slot_dependents,
        external_input_nodes,
        always_process_nodes,
    }
}

fn collect_runtime_dependencies(
    source: &InputValueSource,
    dependent: ExecNodeId,
    slot_dependents: &mut [Vec<ExecNodeId>],
    has_external_input: &mut bool,
) {
    match source {
        InputValueSource::Slot(slot) => {
            if let Some(dependents) = slot_dependents.get_mut(slot.index()) {
                dependents.push(dependent);
            }
        }
        InputValueSource::Converted { source, .. } | InputValueSource::Component { source, .. } => {
            collect_runtime_dependencies(source, dependent, slot_dependents, has_external_input);
        }
        InputValueSource::Composite { base, components, .. } => {
            collect_runtime_dependencies(base, dependent, slot_dependents, has_external_input);
            for (_, source) in components {
                collect_runtime_dependencies(source, dependent, slot_dependents, has_external_input);
            }
        }
        InputValueSource::RuntimeInput { fallback, .. } => {
            *has_external_input = true;
            collect_runtime_dependencies(fallback, dependent, slot_dependents, has_external_input);
        }
        InputValueSource::Constant(_) | InputValueSource::Unset => {}
    }
}

fn analyze_formula(
    exec_nodes: &[CompiledExecNode],
    topo_order: &[ExecNodeId],
    state_slot_count: usize,
    value_slot_count: usize,
    direct_context_axes: &[AxisSet],
) -> FormulaAnalysis {
    let mut analysis = FormulaAnalysis {
        has_stateful_nodes: state_slot_count > 0,
        state_slot_count,
        value_slot_count,
        ..FormulaAnalysis::default()
    };
    let mut slot_axes = vec![AxisSet::new(); value_slot_count];

    for exec_id in topo_order {
        let node = &exec_nodes[exec_id.index()];
        let active = !matches!(node.operation, CompiledNodeOperation::Disabled { .. });
        let mut node_axes = direct_context_axes.get(exec_id.index()).cloned().unwrap_or_default();
        for source in &node.inputs {
            extend_axes_from_input_source(&mut node_axes, source, &slot_axes);
        }
        if active {
            if node.process_on_input_change_only {
                analysis.has_input_gated_nodes = true;
            } else {
                analysis.has_always_process_nodes = true;
            }
            analysis.explicit_context_axes.extend(
                direct_context_axes
                    .get(exec_id.index())
                    .into_iter()
                    .flat_map(|axes| axes.iter().cloned()),
            );
            if !node.state_range.is_empty() {
                analysis.state_axes.extend(node_axes.iter().cloned());
            }
            if matches!(node.execution_kind, ExecutionKind::EffectEmitter) {
                analysis.has_effect_emitters = true;
                analysis.effect_axes.extend(node_axes.iter().cloned());
            }
        }
        for slot in &node.outputs {
            if let Some(axes) = slot_axes.get_mut(slot.index()) {
                axes.extend(node_axes.iter().cloned());
            }
        }
    }

    analysis
}

fn extend_axes_from_input_source(target: &mut AxisSet, source: &InputValueSource, slot_axes: &[AxisSet]) {
    match source {
        InputValueSource::Slot(slot) => {
            if let Some(axes) = slot_axes.get(slot.index()) {
                target.extend(axes.iter().cloned());
            }
        }
        InputValueSource::Converted { source, .. } | InputValueSource::Component { source, .. } => {
            extend_axes_from_input_source(target, source, slot_axes);
        }
        InputValueSource::Composite { base, components, .. } => {
            extend_axes_from_input_source(target, base, slot_axes);
            for (_, source) in components {
                extend_axes_from_input_source(target, source, slot_axes);
            }
        }
        InputValueSource::RuntimeInput { fallback, .. } => {
            extend_axes_from_input_source(target, fallback, slot_axes);
        }
        InputValueSource::Constant(_) | InputValueSource::Unset => {}
    }
}

fn compile_property_schema(
    schema: Option<&FormulaPropertySchema>,
    value_types: &ValueTypeRegistry,
    diagnostics: &mut Vec<Diagnostic>,
) -> CompiledFormulaPropertySchema {
    let Some(schema) = schema else {
        return CompiledFormulaPropertySchema::default();
    };
    let mut properties = IndexMap::with_capacity(schema.properties.len());
    for (index, (id, declaration)) in schema.iter().enumerate() {
        if id != &declaration.id {
            diagnostics.push(Diagnostic::error(
                "property_schema_id_mismatch",
                format!(
                    "property schema key `{id}` does not match declaration id `{}`",
                    declaration.id
                ),
                DiagnosticOrigin::Graph,
            ));
        }
        if !value_types.contains(&declaration.value_type) {
            diagnostics.push(Diagnostic::error(
                "unknown_property_value_type",
                format!(
                    "property `{id}` declares unknown value type `{}`",
                    declaration.value_type
                ),
                DiagnosticOrigin::Graph,
            ));
        }
        let actual = declaration.default_value.value_type();
        if actual != declaration.value_type {
            diagnostics.push(Diagnostic::error(
                "invalid_property_default_type",
                format!(
                    "property `{id}` default has type `{actual}`, expected `{}`",
                    declaration.value_type
                ),
                DiagnosticOrigin::Graph,
            ));
        }
        properties.insert(
            id.clone(),
            CompiledFormulaProperty {
                id: id.clone(),
                slot: FormulaPropertySlotId::new(index as u32),
                value_type: declaration.value_type.clone(),
                default_value: declaration.default_value.clone(),
            },
        );
    }
    CompiledFormulaPropertySchema { properties }
}

fn compile_node_operation(
    instance: &crate::ANodeInstance,
    resolved: &ResolvedANodeSignature,
    ctx: &CompileCtx<'_>,
    properties: &CompiledFormulaPropertySchema,
) -> Result<CompiledNodeOperation, Diagnostic> {
    if instance.type_id.as_str() == "property" {
        return compile_property_operation(instance, properties);
    }
    ctx.nodes
        .get(&instance.type_id)
        .expect("type solving guarantees a registered declaration")
        .compile_operation(instance, resolved)
}

fn compile_property_operation(
    instance: &crate::ANodeInstance,
    properties: &CompiledFormulaPropertySchema,
) -> Result<CompiledNodeOperation, Diagnostic> {
    let Some(property_id) = property_id_from_config(instance) else {
        return Err(Diagnostic::error(
            "missing_property_id",
            "property node is missing a stable property_id",
            DiagnosticOrigin::Node(instance.id),
        ));
    };
    let Some(property) = properties.get(&property_id) else {
        return Err(Diagnostic::error(
            "missing_property_declaration",
            format!("property node references missing property `{property_id}`"),
            DiagnosticOrigin::Node(instance.id),
        ));
    };
    Ok(CompiledNodeOperation::ReadProperty(property.slot))
}

fn property_id_from_config(instance: &crate::ANodeInstance) -> Option<FormulaPropertyId> {
    let RuntimeValue::Ref(value) = instance.config.get("property_id")? else {
        return None;
    };
    (!value.stable_id.is_empty()).then(|| FormulaPropertyId::new(value.stable_id.as_ref()))
}

fn disabled_operation(signature: &ResolvedANodeSignature, value_types: &ValueTypeRegistry) -> CompiledNodeOperation {
    let bypass_input = disabled_bypass_input(signature);
    let outputs = signature
        .outputs
        .values()
        .map(|output| DisabledOutput {
            input_index: bypass_input,
            default_value: output
                .value_type
                .as_ref()
                .and_then(|value_type| value_types.default_value(value_type))
                .unwrap_or(RuntimeValue::Unit),
        })
        .collect();
    CompiledNodeOperation::Disabled { outputs }
}

fn disabled_bypass_input(signature: &ResolvedANodeSignature) -> Option<usize> {
    if signature.inputs.len() != 1 || signature.outputs.len() != 1 {
        return None;
    }
    let input_type = signature.inputs.values().next()?.value_type.as_ref()?;
    let output_type = signature.outputs.values().next()?.value_type.as_ref()?;
    (input_type == output_type).then_some(0)
}

fn input_source(
    graph: &AlchemistGraph,
    solved: &crate::ResolvedGraph,
    value_slots: &IndexMap<(ANodeId, SocketId), ValueSlotId>,
    instance: &crate::ANodeInstance,
    node_id: ANodeId,
    socket: &SocketId,
    target_type: Option<&ValueTypeId>,
    ctx: &CompileCtx<'_>,
) -> InputValueSource {
    let base_edge = graph
        .edges
        .iter()
        .find(|edge| edge.to.node == node_id && edge.to.socket == *socket);
    let component_edges = graph.edges.iter().filter_map(|edge| {
        if edge.to.node != node_id {
            return None;
        }
        let (base, component) = split_socket_component(&edge.to.socket);
        (base == *socket)
            .then_some(component?)
            .map(|component| (component, edge))
    });

    let base = base_edge
        .and_then(|edge| edge_input_source(edge, target_type, solved, value_slots))
        .or_else(|| {
            let fallback = input_default(instance, socket, ctx)
                .map(InputValueSource::Constant)
                .or_else(|| {
                    target_type.and_then(|value_type| {
                        ctx.value_types
                            .default_value(value_type)
                            .map(InputValueSource::Constant)
                    })
                })
                .unwrap_or(InputValueSource::Unset);
            Some(InputValueSource::RuntimeInput {
                reference: formula_input_value_ref(graph.id, node_id, socket),
                fallback: Box::new(fallback),
            })
        });

    let components = component_edges
        .filter_map(|(component, edge)| {
            let component_type = target_type.and_then(|target| component_value_type(target, component))?;
            edge_input_source(edge, Some(&component_type), solved, value_slots).map(|source| (component, source))
        })
        .collect::<Vec<_>>();

    if components.is_empty() {
        return base.unwrap_or(InputValueSource::Unset);
    }

    let Some(target_type) = target_type else {
        return base.unwrap_or(InputValueSource::Unset);
    };
    let base = base
        .or_else(|| {
            ctx.value_types
                .default_value(target_type)
                .map(InputValueSource::Constant)
        })
        .unwrap_or(InputValueSource::Unset);
    InputValueSource::Composite {
        target_type: target_type.clone(),
        base: Box::new(base),
        components,
    }
}

fn edge_input_source(
    edge: &crate::AEdge,
    target_type: Option<&ValueTypeId>,
    solved: &crate::ResolvedGraph,
    value_slots: &IndexMap<(ANodeId, SocketId), ValueSlotId>,
) -> Option<InputValueSource> {
    let (source_socket, source_component) = split_socket_component(&edge.from.socket);
    let slot = value_slots.get(&(edge.from.node, source_socket.clone())).copied()?;
    let mut source = InputValueSource::Slot(slot);
    let mut source_type = solved
        .nodes
        .get(&edge.from.node)?
        .signature
        .outputs
        .get(&source_socket)?
        .value_type
        .clone();
    if let Some(component) = source_component {
        source = InputValueSource::Component {
            source: Box::new(source),
            component,
        };
        source_type = source_type.and_then(|value_type| component_value_type(&value_type, component));
    }
    if let (Some(source_type), Some(target_type)) = (source_type, target_type)
        && source_type != *target_type
    {
        source = InputValueSource::Converted {
            source: Box::new(source),
            target_type: target_type.clone(),
        };
    }
    Some(source)
}

fn input_default(instance: &crate::ANodeInstance, socket: &SocketId, ctx: &CompileCtx<'_>) -> Option<RuntimeValue> {
    if let Some(value) = instance.input_defaults.get(socket) {
        return Some(value.clone());
    }
    let declaration = ctx.nodes.get(&instance.type_id)?;
    let signature = declaration.signature(
        &crate::SignatureCtx {
            value_types: ctx.value_types,
            properties: ctx.properties,
        },
        instance,
        &instance.type_bindings,
    );
    signature
        .inputs
        .into_iter()
        .find(|input| input.id == *socket)?
        .default_value
}

fn topological_order(graph: &AlchemistGraph, registry: &ANodeRegistry) -> Result<Vec<ANodeId>, Vec<ANodeId>> {
    let mut indegree = IndexMap::<ANodeId, usize>::new();
    let mut outgoing = IndexMap::<ANodeId, Vec<ANodeId>>::new();
    for node in graph.nodes.keys() {
        indegree.insert(*node, 0);
        outgoing.insert(*node, Vec::new());
    }
    for edge in &graph.edges {
        let breaks_cycle = graph
            .nodes
            .get(&edge.from.node)
            .and_then(|node| registry.get(&node.type_id))
            .is_some_and(|declaration| declaration.breaks_dependency_cycle());
        if breaks_cycle {
            continue;
        }
        if let Some(value) = indegree.get_mut(&edge.to.node) {
            *value += 1;
        }
        if let Some(targets) = outgoing.get_mut(&edge.from.node) {
            targets.push(edge.to.node);
        }
    }

    let mut ready: VecDeque<ANodeId> = indegree
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some(*node))
        .collect();
    let mut order = Vec::with_capacity(graph.nodes.len());
    while let Some(node) = ready.pop_front() {
        order.push(node);
        for target in &outgoing[&node] {
            let degree = indegree.get_mut(target).expect("target node must exist");
            *degree -= 1;
            if *degree == 0 {
                ready.push_back(*target);
            }
        }
    }
    if order.len() == graph.nodes.len() {
        Ok(order)
    } else {
        Err(indegree
            .into_iter()
            .filter_map(|(node, degree)| (degree > 0).then_some(node))
            .collect())
    }
}

fn validate_exposed_surface(exposed: &ExposedSurface, graph: &AlchemistGraph, diagnostics: &mut Vec<Diagnostic>) {
    let targets = exposed
        .params
        .iter()
        .map(|declaration| (&declaration.decl_id, declaration.target.node))
        .chain(
            exposed
                .inputs
                .iter()
                .map(|declaration| (&declaration.decl_id, declaration.target.node)),
        )
        .chain(
            exposed
                .outputs
                .iter()
                .map(|declaration| (&declaration.decl_id, declaration.source.node)),
        )
        .chain(
            exposed
                .actions
                .iter()
                .map(|declaration| (&declaration.decl_id, declaration.target.node)),
        );
    for (declaration, node) in targets {
        if !graph.nodes.contains_key(&node) {
            diagnostics.push(Diagnostic::error(
                "missing_exposed_target",
                format!("exposed declaration `{declaration}` targets a missing node"),
                DiagnosticOrigin::Graph,
            ));
        }
    }
}

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn split_socket_component(socket: &SocketId) -> (SocketId, Option<ValueComponent>) {
    let Some((base, component)) = socket.as_str().rsplit_once('.') else {
        return (socket.clone(), None);
    };
    let Some(component) = ValueComponent::parse(component) else {
        return (socket.clone(), None);
    };
    (SocketId::new(base), Some(component))
}
