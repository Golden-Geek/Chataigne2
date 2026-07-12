use crate::{
    ANodeDeclaration, ANodeInstance, ANodeSignature, ANodeTypeId, AlchemistGraph, AxisSet, CompileCtx,
    CompiledNodeOperation, ContextAxisId, ExecutionKind, FormulaPropertyDecl, FormulaPropertyId, FormulaPropertySchema,
    InputSocketDecl, InputSocketRef, NodeStateLayout, OutputSocketDecl, OutputSocketRef, ResolvedANodeSignature,
    RuntimeValue, SignatureCtx, StableRef, TypeBindings, TypeConstraint, TypeSolveCtx, ValueTypeId, ValueTypeRegistry,
    compile_graph, primitive_node_registry, solve_types,
};

fn node(type_id: &str) -> ANodeInstance {
    ANodeInstance::new(ANodeTypeId::new(type_id), type_id)
}

fn compile(graph: &AlchemistGraph) -> crate::CompileResult {
    compile_with_properties(graph, None)
}

fn compile_with_properties(graph: &AlchemistGraph, properties: Option<&FormulaPropertySchema>) -> crate::CompileResult {
    compile_graph(
        graph,
        &CompileCtx {
            value_types: &ValueTypeRegistry::with_primitives(),
            nodes: &primitive_node_registry(),
            properties,
        },
    )
}

fn compile_with_nodes(graph: &AlchemistGraph, nodes: &crate::ANodeRegistry) -> crate::CompileResult {
    compile_graph(
        graph,
        &CompileCtx {
            value_types: &ValueTypeRegistry::with_primitives(),
            nodes,
            properties: None,
        },
    )
}

fn property_schema(id: &str, value_type: &str, default_value: RuntimeValue) -> FormulaPropertySchema {
    let mut schema = FormulaPropertySchema::default();
    schema.insert(FormulaPropertyDecl {
        id: FormulaPropertyId::new(id),
        label: id.into(),
        description: None,
        value_type: ValueTypeId::new(value_type),
        default_value,
        ui: crate::PropertyUiHints::default(),
    });
    schema
}

fn property_node(id: &str) -> ANodeInstance {
    let mut node = node("property");
    node.config.set(
        "property_id",
        RuntimeValue::Ref(StableRef::new(ValueTypeId::new("property"), id)),
    );
    node
}

struct StateLayoutNodeDeclaration {
    type_id: &'static str,
    execution_kind: ExecutionKind,
    layout: NodeStateLayout,
}

impl StateLayoutNodeDeclaration {
    const fn new(type_id: &'static str, execution_kind: ExecutionKind, layout: NodeStateLayout) -> Self {
        Self {
            type_id,
            execution_kind,
            layout,
        }
    }
}

impl ANodeDeclaration for StateLayoutNodeDeclaration {
    fn type_id(&self) -> ANodeTypeId {
        ANodeTypeId::new(self.type_id)
    }

    fn label(&self) -> &'static str {
        self.type_id
    }

    fn category(&self) -> &'static str {
        "Test"
    }

    fn execution_kind(&self) -> ExecutionKind {
        self.execution_kind
    }

    fn signature(
        &self,
        _ctx: &SignatureCtx<'_>,
        _instance: &ANodeInstance,
        _bindings: &TypeBindings,
    ) -> ANodeSignature {
        ANodeSignature::default()
    }

    fn state_layout(&self, _instance: &ANodeInstance, _resolved: &ResolvedANodeSignature) -> NodeStateLayout {
        self.layout
    }

    fn compile_operation(
        &self,
        _instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, crate::Diagnostic> {
        Ok(CompiledNodeOperation::Constant(RuntimeValue::Unit))
    }
}

struct ContextAnalysisNodeDeclaration {
    type_id: &'static str,
    execution_kind: ExecutionKind,
    context_axis: Option<&'static str>,
    has_input: bool,
    has_output: bool,
    layout: NodeStateLayout,
}

impl ContextAnalysisNodeDeclaration {
    const fn new(
        type_id: &'static str,
        execution_kind: ExecutionKind,
        context_axis: Option<&'static str>,
        has_input: bool,
        has_output: bool,
        layout: NodeStateLayout,
    ) -> Self {
        Self {
            type_id,
            execution_kind,
            context_axis,
            has_input,
            has_output,
            layout,
        }
    }
}

impl ANodeDeclaration for ContextAnalysisNodeDeclaration {
    fn type_id(&self) -> ANodeTypeId {
        ANodeTypeId::new(self.type_id)
    }

    fn label(&self) -> &'static str {
        self.type_id
    }

    fn category(&self) -> &'static str {
        "Test"
    }

    fn execution_kind(&self) -> ExecutionKind {
        self.execution_kind
    }

    fn signature(
        &self,
        _ctx: &SignatureCtx<'_>,
        _instance: &ANodeInstance,
        _bindings: &TypeBindings,
    ) -> ANodeSignature {
        let bool_type = ValueTypeId::new("bool");
        ANodeSignature {
            inputs: self
                .has_input
                .then(|| {
                    vec![InputSocketDecl::new(
                        "in",
                        "In",
                        TypeConstraint::Exact(bool_type.clone()),
                    )]
                })
                .unwrap_or_default(),
            outputs: self
                .has_output
                .then(|| vec![OutputSocketDecl::new("out", "Out", TypeConstraint::Exact(bool_type))])
                .unwrap_or_default(),
            ..ANodeSignature::default()
        }
    }

    fn state_layout(&self, _instance: &ANodeInstance, _resolved: &ResolvedANodeSignature) -> NodeStateLayout {
        self.layout
    }

    fn context_axes(&self, _instance: &ANodeInstance, _resolved: &ResolvedANodeSignature) -> AxisSet {
        self.context_axis
            .map(|axis| {
                let mut axes = AxisSet::new();
                axes.insert(ContextAxisId::new(axis));
                axes
            })
            .unwrap_or_default()
    }

    fn compile_operation(
        &self,
        _instance: &ANodeInstance,
        _resolved: &ResolvedANodeSignature,
    ) -> Result<CompiledNodeOperation, crate::Diagnostic> {
        Ok(CompiledNodeOperation::Constant(RuntimeValue::Bool(true)))
    }
}

#[test]
fn compiler_builds_dense_schedule_and_memory_layout() {
    let mut graph = AlchemistGraph::new();
    let mut constant = node("constant");
    constant.config.set("value", RuntimeValue::Float(2.0));
    let constant = graph.add_node(constant).unwrap();
    let add = graph.add_node(node("math")).unwrap();
    let delay = graph.add_node(node("delay_one_tick")).unwrap();
    graph
        .connect(
            OutputSocketRef::new(constant, "value"),
            InputSocketRef::new(add, "value1"),
        )
        .unwrap();

    let result = compile(&graph);
    let compiled = result.compiled.unwrap();

    assert_eq!(compiled.exec_nodes.len(), 3);
    assert_eq!(compiled.topo_order.len(), 3);
    assert_eq!(compiled.exec_nodes[0].exec_id.index(), 0);
    assert_eq!(compiled.exec_nodes[1].exec_id.index(), 1);
    assert_eq!(compiled.debug_map.exec_to_authored, vec![constant, delay, add]);
    assert_eq!(compiled.state_layout.state_slot_count, 1);
}

#[test]
fn stateful_node_can_request_three_slots() {
    let mut nodes = primitive_node_registry();
    nodes
        .register(StateLayoutNodeDeclaration::new(
            "three_slot_state",
            ExecutionKind::Stateful,
            NodeStateLayout::RuntimeValues(3),
        ))
        .unwrap();
    let mut graph = AlchemistGraph::new();
    graph.add_node(node("three_slot_state")).unwrap();

    let result = compile_with_nodes(&graph, &nodes);
    let compiled = result.compiled.unwrap();

    assert_eq!(compiled.state_layout.state_slot_count, 3);
    assert_eq!(compiled.exec_nodes[0].state_range, 0..3);
}

#[test]
fn multiple_stateful_nodes_sum_state_slots() {
    let mut nodes = primitive_node_registry();
    nodes
        .register(StateLayoutNodeDeclaration::new(
            "two_slot_state",
            ExecutionKind::Stateful,
            NodeStateLayout::RuntimeValues(2),
        ))
        .unwrap();
    nodes
        .register(StateLayoutNodeDeclaration::new(
            "three_slot_state",
            ExecutionKind::Stateful,
            NodeStateLayout::RuntimeValues(3),
        ))
        .unwrap();
    let mut graph = AlchemistGraph::new();
    graph.add_node(node("two_slot_state")).unwrap();
    graph.add_node(node("three_slot_state")).unwrap();

    let result = compile_with_nodes(&graph, &nodes);
    let compiled = result.compiled.unwrap();

    assert_eq!(compiled.state_layout.state_slot_count, 5);
    assert_eq!(compiled.exec_nodes[0].state_range, 0..2);
    assert_eq!(compiled.exec_nodes[1].state_range, 2..5);
}

#[test]
fn stateless_node_has_empty_state_slice() {
    let mut nodes = primitive_node_registry();
    nodes
        .register(StateLayoutNodeDeclaration::new(
            "stateless_test",
            ExecutionKind::Pure,
            NodeStateLayout::Stateless,
        ))
        .unwrap();
    let mut graph = AlchemistGraph::new();
    graph.add_node(node("stateless_test")).unwrap();

    let result = compile_with_nodes(&graph, &nodes);
    let compiled = result.compiled.unwrap();

    assert_eq!(compiled.state_layout.state_slot_count, 0);
    assert_eq!(compiled.exec_nodes[0].state_range, 0..0);
}

#[test]
fn formula_analysis_propagates_context_axes_to_stateful_nodes() {
    let mut nodes = primitive_node_registry();
    nodes
        .register(ContextAnalysisNodeDeclaration::new(
            "context_source",
            ExecutionKind::Pure,
            Some("device"),
            false,
            true,
            NodeStateLayout::Stateless,
        ))
        .unwrap();
    nodes
        .register(ContextAnalysisNodeDeclaration::new(
            "state_consumer",
            ExecutionKind::Stateful,
            None,
            true,
            false,
            NodeStateLayout::RuntimeValues(1),
        ))
        .unwrap();
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(node("context_source")).unwrap();
    let state = graph.add_node(node("state_consumer")).unwrap();
    graph
        .connect(OutputSocketRef::new(source, "out"), InputSocketRef::new(state, "in"))
        .unwrap();

    let result = compile_with_nodes(&graph, &nodes);
    let compiled = result.compiled.unwrap();

    assert!(compiled.analysis.has_stateful_nodes);
    assert!(!compiled.analysis.has_effect_emitters);
    assert!(
        compiled
            .analysis
            .explicit_context_axes
            .contains(&ContextAxisId::new("device"))
    );
    assert!(compiled.analysis.state_axes.contains(&ContextAxisId::new("device")));
    assert!(compiled.analysis.effect_axes.is_empty());
}

#[test]
fn formula_analysis_propagates_context_axes_to_effect_emitters() {
    let mut nodes = primitive_node_registry();
    nodes
        .register(ContextAnalysisNodeDeclaration::new(
            "context_source",
            ExecutionKind::Pure,
            Some("device"),
            false,
            true,
            NodeStateLayout::Stateless,
        ))
        .unwrap();
    nodes
        .register(ContextAnalysisNodeDeclaration::new(
            "effect_consumer",
            ExecutionKind::EffectEmitter,
            None,
            true,
            false,
            NodeStateLayout::Stateless,
        ))
        .unwrap();
    let mut graph = AlchemistGraph::new();
    let source = graph.add_node(node("context_source")).unwrap();
    let effect = graph.add_node(node("effect_consumer")).unwrap();
    graph
        .connect(OutputSocketRef::new(source, "out"), InputSocketRef::new(effect, "in"))
        .unwrap();

    let result = compile_with_nodes(&graph, &nodes);
    let compiled = result.compiled.unwrap();

    assert!(!compiled.analysis.has_stateful_nodes);
    assert!(compiled.analysis.has_effect_emitters);
    assert!(
        compiled
            .analysis
            .explicit_context_axes
            .contains(&ContextAxisId::new("device"))
    );
    assert!(compiled.analysis.state_axes.is_empty());
    assert!(compiled.analysis.effect_axes.contains(&ContextAxisId::new("device")));
}

#[test]
fn cycle_without_delay_is_reported() {
    let mut graph = AlchemistGraph::new();
    let first = graph.add_node(node("math")).unwrap();
    let second = graph.add_node(node("math")).unwrap();
    graph
        .connect(
            OutputSocketRef::new(first, "result"),
            InputSocketRef::new(second, "value1"),
        )
        .unwrap();
    graph
        .connect(
            OutputSocketRef::new(second, "result"),
            InputSocketRef::new(first, "value1"),
        )
        .unwrap();

    let result = compile(&graph);

    assert!(result.compiled.is_none());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "cycle_without_delay")
    );
}

#[test]
fn delay_node_allows_feedback() {
    let mut graph = AlchemistGraph::new();
    let add = graph.add_node(node("math")).unwrap();
    let delay = graph.add_node(node("delay_one_tick")).unwrap();
    graph
        .connect(OutputSocketRef::new(add, "result"), InputSocketRef::new(delay, "value"))
        .unwrap();
    graph
        .connect(OutputSocketRef::new(delay, "value"), InputSocketRef::new(add, "value1"))
        .unwrap();

    let result = compile(&graph);

    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    assert_eq!(result.compiled.unwrap().topo_order.len(), 2);
}

#[test]
fn property_decl_rejects_invalid_default() {
    let graph = AlchemistGraph::new();
    let schema = property_schema("amount", "float", RuntimeValue::String("not a float".into()));

    let result = compile_with_properties(&graph, Some(&schema));

    assert!(result.compiled.is_none());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "invalid_property_default_type")
    );
}

#[test]
fn property_node_rejects_missing_property_id() {
    let mut graph = AlchemistGraph::new();
    graph.add_node(node("property")).unwrap();
    let schema = property_schema("amount", "float", RuntimeValue::Float(1.0));

    let result = compile_with_properties(&graph, Some(&schema));

    assert!(result.compiled.is_none());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_property_id")
    );
}

#[test]
fn property_node_type_comes_from_schema() {
    let mut graph = AlchemistGraph::new();
    let mut property = property_node("enabled");
    property.config.set("value", RuntimeValue::Float(123.0));
    let property = graph.add_node(property).unwrap();
    let schema = property_schema("enabled", "bool", RuntimeValue::Bool(true));
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();

    let result = solve_types(
        &graph,
        &TypeSolveCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: Some(&schema),
        },
    );

    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    assert_eq!(
        result.graph.nodes[&property].signature.outputs[&crate::SocketId::new("value")].value_type,
        Some(ValueTypeId::new("bool"))
    );
}
