use std::{collections::VecDeque, ops::Range, sync::Arc};

use indexmap::IndexMap;

use crate::{
    ANodeId, ANodeRegistry, AlchemistGraph, CompiledNodeEvaluator, Diagnostic, DiagnosticOrigin, DiagnosticSeverity,
    ExecNodeId, ExecutionKind, ExposedSurface, RuntimeValue, SocketId, TypeSolveCtx, ValueSlotId, ValueTypeRegistry,
    solve_types,
};

#[derive(Clone, Debug, PartialEq)]
pub enum InputValueSource {
    Slot(ValueSlotId),
    Constant(RuntimeValue),
    Unset,
}

#[derive(Clone, Debug)]
pub enum CompiledNodeOperation {
    Constant(RuntimeValue),
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
pub struct CompiledExecNode {
    pub exec_id: ExecNodeId,
    pub authored_id: ANodeId,
    pub execution_kind: ExecutionKind,
    pub operation: CompiledNodeOperation,
    pub inputs: Vec<InputValueSource>,
    pub outputs: Vec<ValueSlotId>,
    pub state_range: Range<usize>,
}

#[derive(Clone, Debug, Default)]
pub struct RuntimeStateLayout {
    pub ranges: Vec<Range<usize>>,
    pub state_slot_count: usize,
    pub value_slot_count: usize,
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

#[derive(Clone, Debug)]
pub struct CompiledAlchemistGraph {
    pub exec_nodes: Vec<CompiledExecNode>,
    pub topo_order: Vec<ExecNodeId>,
    pub state_layout: RuntimeStateLayout,
    pub output_routes: Vec<OutputRoute>,
    pub subscriptions: Vec<RuntimeSubscription>,
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

pub struct CompileCtx<'a> {
    pub value_types: &'a ValueTypeRegistry,
    pub nodes: &'a ANodeRegistry,
}

#[must_use]
pub fn compile_graph(graph: &AlchemistGraph, ctx: &CompileCtx<'_>) -> CompileResult {
    let solved = solve_types(
        graph,
        &TypeSolveCtx {
            value_types: ctx.value_types,
            nodes: ctx.nodes,
        },
    );
    let mut diagnostics = solved.diagnostics;
    validate_exposed_surface(&graph.exposed, graph, &mut diagnostics);
    if has_errors(&diagnostics) {
        return CompileResult {
            compiled: None,
            diagnostics,
        };
    }

    let authored_to_exec: IndexMap<ANodeId, ExecNodeId> = graph
        .nodes
        .keys()
        .enumerate()
        .map(|(index, node)| (*node, ExecNodeId::new(index as u32)))
        .collect();
    let mut value_slots = IndexMap::<(ANodeId, SocketId), ValueSlotId>::new();
    let mut next_value_slot = 0_u32;
    for (node_id, resolved) in &solved.graph.nodes {
        for socket in resolved.signature.outputs.keys() {
            value_slots.insert((*node_id, socket.clone()), ValueSlotId::new(next_value_slot));
            next_value_slot += 1;
        }
    }

    let topo_order = match topological_order(graph, ctx.nodes, &authored_to_exec) {
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

    let mut state_slot_count = 0_usize;
    let mut ranges = Vec::with_capacity(graph.nodes.len());
    let mut exec_nodes = Vec::with_capacity(graph.nodes.len());
    for (node_id, instance) in &graph.nodes {
        let exec_id = authored_to_exec[node_id];
        let resolved = &solved.graph.nodes[node_id];
        let state_size = usize::from(resolved.execution_kind == ExecutionKind::Stateful);
        let state_range = state_slot_count..state_slot_count + state_size;
        state_slot_count += state_size;
        ranges.push(state_range.clone());

        let inputs = resolved
            .signature
            .inputs
            .keys()
            .map(|socket| {
                graph
                    .edges
                    .iter()
                    .find(|edge| edge.to.node == *node_id && edge.to.socket == *socket)
                    .and_then(|edge| value_slots.get(&(edge.from.node, edge.from.socket.clone())).copied())
                    .map(InputValueSource::Slot)
                    .or_else(|| input_default(instance, socket, ctx).map(InputValueSource::Constant))
                    .unwrap_or(InputValueSource::Unset)
            })
            .collect();
        let outputs = resolved
            .signature
            .outputs
            .keys()
            .map(|socket| value_slots[&(*node_id, socket.clone())])
            .collect();
        let operation = match ctx
            .nodes
            .get(&instance.type_id)
            .expect("type solving guarantees a registered declaration")
            .compile_operation(instance, &resolved.signature)
        {
            Ok(operation) => operation,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        exec_nodes.push(CompiledExecNode {
            exec_id,
            authored_id: *node_id,
            execution_kind: resolved.execution_kind,
            operation,
            inputs,
            outputs,
            state_range,
        });
    }
    if has_errors(&diagnostics) {
        return CompileResult {
            compiled: None,
            diagnostics,
        };
    }

    let debug_map = DebugSourceMap {
        exec_to_authored: graph.nodes.keys().copied().collect(),
    };
    CompileResult {
        compiled: Some(Arc::new(CompiledAlchemistGraph {
            exec_nodes,
            topo_order,
            state_layout: RuntimeStateLayout {
                ranges,
                state_slot_count,
                value_slot_count: next_value_slot as usize,
            },
            output_routes: Vec::new(),
            subscriptions: Vec::new(),
            debug_map,
        })),
        diagnostics,
    }
}

fn input_default(instance: &crate::ANodeInstance, socket: &SocketId, ctx: &CompileCtx<'_>) -> Option<RuntimeValue> {
    let declaration = ctx.nodes.get(&instance.type_id)?;
    let signature = declaration.signature(
        &crate::SignatureCtx {
            value_types: ctx.value_types,
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

fn topological_order(
    graph: &AlchemistGraph,
    registry: &ANodeRegistry,
    authored_to_exec: &IndexMap<ANodeId, ExecNodeId>,
) -> Result<Vec<ExecNodeId>, Vec<ANodeId>> {
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
        order.push(authored_to_exec[&node]);
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
