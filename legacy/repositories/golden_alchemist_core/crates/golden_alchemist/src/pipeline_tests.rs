use crate::{
    AEdge, ANodeDeclaration, ANodeId, ANodeInstance, ANodeRoleCapability, ANodeSignature, ANodeTypeId, AlchemistGraph,
    AutoWirePolicy, ExecutionKind, InputSocketDecl, ManagedItemId, ManagedItemInstance, ManagedItemUiState,
    ManagedRegionDefinition, ManagedRegionId, ManagedRegionInstance, ManagedRegionKind, ManagedSocketRef,
    ManagedUiMode, OutputSocketDecl, OutputSocketRef, PipelineCardinality, PipelineLoweringCtx,
    PipelineLoweringDiagnosticKind, PipelineShape, PipelineShapeCheckItem, PrimitiveNodeDeclaration, PrimitiveNodeKind,
    RuntimeValue, SignatureCtx, SocketId, SurfaceItemKind, TypeBindings, TypeConstraint, ValueTypeId,
    ValueTypeRegistry, check_filter_pipeline_shapes, lower_filter_pipeline_region, primitive_node_registry,
    single_shape, value_set_shape,
};

fn ctx(value_types: &ValueTypeRegistry) -> SignatureCtx<'_> {
    SignatureCtx {
        value_types,
        properties: None,
    }
}

fn primitive_item<'a>(
    declaration: &'a PrimitiveNodeDeclaration,
    instance: &'a ANodeInstance,
) -> PipelineShapeCheckItem<'a> {
    PipelineShapeCheckItem { declaration, instance }
}

fn primitive_instance(kind: PrimitiveNodeKind) -> (PrimitiveNodeDeclaration, ANodeInstance) {
    let declaration = PrimitiveNodeDeclaration::new(kind);
    let instance = ANodeInstance::new(declaration.type_id(), declaration.label());
    (declaration, instance)
}

fn managed_item(kind: PrimitiveNodeKind) -> ManagedItemInstance {
    let declaration = PrimitiveNodeDeclaration::new(kind);
    ManagedItemInstance {
        id: ManagedItemId::new(),
        anode: ANodeInstance::new(declaration.type_id(), declaration.label()),
        enabled: true,
        ui_state: ManagedItemUiState::default(),
    }
}

fn boundary_graph() -> (AlchemistGraph, ANodeId, ANodeId) {
    let mut graph = AlchemistGraph::new();
    let input = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("boundary_input"), "Boundary Input"))
        .unwrap();
    let output = graph
        .add_node(ANodeInstance::new(
            ANodeTypeId::new("boundary_output"),
            "Boundary Output",
        ))
        .unwrap();
    (graph, input, output)
}

fn filter_region(input: ANodeId, output: ANodeId) -> ManagedRegionDefinition {
    ManagedRegionDefinition {
        id: ManagedRegionId::new("filters"),
        kind: ManagedRegionKind::FilterPipeline,
        label: "Filters".into(),
        input_socket: Some(ManagedSocketRef::new(input, "value")),
        output_socket: Some(ManagedSocketRef::new(output, "value")),
        accepted_roles: vec![SurfaceItemKind::Filter],
    }
}

fn filter_region_instance(items: Vec<ManagedItemInstance>) -> ManagedRegionInstance {
    ManagedRegionInstance {
        region_id: ManagedRegionId::new("filters"),
        items,
    }
}

#[test]
fn elementwise_filter_preserves_valueset_shape() {
    let value_types = ValueTypeRegistry::with_primitives();
    let (declaration, instance) = primitive_instance(PrimitiveNodeKind::Remap);

    let result = check_filter_pipeline_shapes(
        value_set_shape("float", Some(crate::ContextAxisId::new("input_lane"))),
        [primitive_item(&declaration, &instance)],
        &ctx(&value_types),
    );

    assert!(result.is_valid());
    assert_eq!(
        result.final_shape,
        PipelineShape::ValueSet {
            item_type: ValueTypeId::new("float"),
            axis: Some(crate::ContextAxisId::new("input_lane")),
        }
    );
    assert_eq!(result.steps[0].cardinality, PipelineCardinality::Elementwise);
}

#[test]
fn aggregate_filter_collapses_valueset_to_single() {
    let value_types = ValueTypeRegistry::with_primitives();
    let (declaration, instance) = primitive_instance(PrimitiveNodeKind::Math);

    let result = check_filter_pipeline_shapes(
        value_set_shape("float", Some(crate::ContextAxisId::new("input_lane"))),
        [primitive_item(&declaration, &instance)],
        &ctx(&value_types),
    );

    assert!(result.is_valid());
    assert_eq!(result.final_shape, single_shape("float"));
    assert_eq!(result.steps[0].cardinality, PipelineCardinality::Aggregate);
}

#[test]
fn reshape_filter_can_pack_valueset_items_to_vec3() {
    let value_types = ValueTypeRegistry::with_primitives();
    let declaration = ShapeOnlyDeclaration {
        type_name: "test_pack_vec3",
        label: "Test Pack Vec3",
        cardinality: PipelineCardinality::Reshape,
        output_type: "vec3",
    };
    let instance = ANodeInstance::new(declaration.type_id(), declaration.label());

    let result = check_filter_pipeline_shapes(
        value_set_shape("float", None),
        [PipelineShapeCheckItem {
            declaration: &declaration,
            instance: &instance,
        }],
        &ctx(&value_types),
    );

    assert!(result.is_valid());
    assert_eq!(result.final_shape, single_shape("vec3"));
    assert_eq!(result.steps[0].cardinality, PipelineCardinality::Reshape);
}

#[test]
fn checker_rejects_nodes_without_filter_capability() {
    let value_types = ValueTypeRegistry::with_primitives();
    let (declaration, instance) = primitive_instance(PrimitiveNodeKind::Constant);

    let result = check_filter_pipeline_shapes(
        single_shape("float"),
        [primitive_item(&declaration, &instance)],
        &ctx(&value_types),
    );

    assert!(!result.is_valid());
    assert_eq!(result.final_shape, single_shape("float"));
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].node_type, ANodeTypeId::new("constant"));
    assert!(result.diagnostics[0].message.contains("not filter-capable"));
}

#[test]
fn condition_gate_preserves_pipeline_shape() {
    let value_types = ValueTypeRegistry::with_primitives();
    let (declaration, instance) = primitive_instance(PrimitiveNodeKind::ConditionGate);
    let initial = value_set_shape("float", Some(crate::ContextAxisId::new("input_lane")));

    let result = check_filter_pipeline_shapes(
        initial.clone(),
        [primitive_item(&declaration, &instance)],
        &ctx(&value_types),
    );

    assert!(result.is_valid());
    assert_eq!(result.final_shape, initial);
    assert_eq!(result.steps[0].cardinality, PipelineCardinality::WholeSet);
}

#[test]
fn expand_filter_broadcasts_single_value_to_valueset() {
    let value_types = ValueTypeRegistry::with_primitives();
    let declaration = ShapeOnlyDeclaration {
        type_name: "test_broadcast",
        label: "Test Broadcast",
        cardinality: PipelineCardinality::Expand,
        output_type: "float",
    };
    let instance = ANodeInstance::new(declaration.type_id(), declaration.label());

    let result = check_filter_pipeline_shapes(
        single_shape("float"),
        [PipelineShapeCheckItem {
            declaration: &declaration,
            instance: &instance,
        }],
        &ctx(&value_types),
    );

    assert!(result.is_valid());
    assert_eq!(
        result.final_shape,
        PipelineShape::ValueSet {
            item_type: ValueTypeId::new("float"),
            axis: None,
        }
    );
    assert_eq!(result.steps[0].cardinality, PipelineCardinality::Expand);
}

#[test]
fn lowering_autowires_enabled_filter_items_into_graph() {
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let (graph, input, output) = boundary_graph();
    let region = filter_region(input, output);
    let instance = filter_region_instance(vec![
        managed_item(PrimitiveNodeKind::Remap),
        managed_item(PrimitiveNodeKind::ConditionGate),
    ]);
    let remap = instance.items[0].anode.id;
    let gate = instance.items[1].anode.id;

    let result = lower_filter_pipeline_region(
        &graph,
        &region,
        &instance,
        single_shape("float"),
        &PipelineLoweringCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );

    assert!(result.is_valid(), "{:?}", result.diagnostics);
    assert_eq!(result.inserted_nodes, vec![remap, gate]);
    assert_eq!(result.graph.nodes.len(), graph.nodes.len() + 2);
    assert_eq!(
        result.graph.edges,
        vec![
            AEdge {
                from: OutputSocketRef::new(input, "value"),
                to: crate::InputSocketRef::new(remap, "value"),
            },
            AEdge {
                from: OutputSocketRef::new(remap, "result"),
                to: crate::InputSocketRef::new(gate, "value"),
            },
            AEdge {
                from: OutputSocketRef::new(gate, "value"),
                to: crate::InputSocketRef::new(output, "value"),
            },
        ]
    );
}

#[test]
fn lowering_skips_disabled_filter_items() {
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let (graph, input, output) = boundary_graph();
    let region = filter_region(input, output);
    let mut disabled = managed_item(PrimitiveNodeKind::OneMinus);
    disabled.enabled = false;
    let disabled_node = disabled.anode.id;
    let instance = filter_region_instance(vec![managed_item(PrimitiveNodeKind::Remap), disabled]);
    let remap = instance.items[0].anode.id;

    let result = lower_filter_pipeline_region(
        &graph,
        &region,
        &instance,
        single_shape("float"),
        &PipelineLoweringCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );

    assert!(result.is_valid(), "{:?}", result.diagnostics);
    assert_eq!(result.inserted_nodes, vec![remap]);
    assert!(!result.graph.nodes.contains_key(&disabled_node));
    assert_eq!(result.graph.edges.len(), 2);
}

#[test]
fn lowering_rejects_non_filter_items_without_mutating_graph() {
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let (graph, input, output) = boundary_graph();
    let region = filter_region(input, output);
    let instance = filter_region_instance(vec![managed_item(PrimitiveNodeKind::Constant)]);

    let result = lower_filter_pipeline_region(
        &graph,
        &region,
        &instance,
        single_shape("float"),
        &PipelineLoweringCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );

    assert!(!result.is_valid());
    assert_eq!(result.graph, graph);
    assert_eq!(result.shape.diagnostics.len(), 1);
    assert!(result.shape.diagnostics[0].message.contains("not filter-capable"));
}

#[test]
fn lowering_requires_linear_autowire_sockets() {
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let (graph, input, output) = boundary_graph();
    let region = filter_region(input, output);
    let instance = filter_region_instance(vec![managed_item(PrimitiveNodeKind::Math)]);

    let result = lower_filter_pipeline_region(
        &graph,
        &region,
        &instance,
        single_shape("float"),
        &PipelineLoweringCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );

    assert!(!result.is_valid());
    assert_eq!(result.graph, graph);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].kind,
        PipelineLoweringDiagnosticKind::MissingLinearAutowire
    );
    assert!(result.diagnostics[0].message.contains("linear autowire sockets"));
}

#[test]
fn lowering_rejects_valueset_elementwise_until_lane_strategy_exists() {
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let (graph, input, output) = boundary_graph();
    let region = filter_region(input, output);
    let instance = filter_region_instance(vec![managed_item(PrimitiveNodeKind::Remap)]);

    let result = lower_filter_pipeline_region(
        &graph,
        &region,
        &instance,
        value_set_shape("float", Some(crate::ContextAxisId::new("input_lane"))),
        &PipelineLoweringCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );

    assert!(!result.is_valid());
    assert_eq!(result.graph, graph);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].kind,
        PipelineLoweringDiagnosticKind::UnsupportedValueSetElementwise
    );
    assert!(result.diagnostics[0].message.contains("lane-aware MapEach support"));
}

#[test]
fn lowering_allows_whole_valueset_filters() {
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();
    let (graph, input, output) = boundary_graph();
    let region = filter_region(input, output);
    let instance = filter_region_instance(vec![managed_item(PrimitiveNodeKind::ConditionGate)]);
    let gate = instance.items[0].anode.id;

    let result = lower_filter_pipeline_region(
        &graph,
        &region,
        &instance,
        value_set_shape("float", Some(crate::ContextAxisId::new("input_lane"))),
        &PipelineLoweringCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );

    assert!(result.is_valid(), "{:?}", result.diagnostics);
    assert_eq!(result.inserted_nodes, vec![gate]);
    assert_eq!(
        result.graph.edges,
        vec![
            AEdge {
                from: OutputSocketRef::new(input, "value"),
                to: crate::InputSocketRef::new(gate, "value"),
            },
            AEdge {
                from: OutputSocketRef::new(gate, "value"),
                to: crate::InputSocketRef::new(output, "value"),
            },
        ]
    );
}

struct ShapeOnlyDeclaration {
    type_name: &'static str,
    label: &'static str,
    cardinality: PipelineCardinality,
    output_type: &'static str,
}

impl ANodeDeclaration for ShapeOnlyDeclaration {
    fn type_id(&self) -> ANodeTypeId {
        ANodeTypeId::new(self.type_name)
    }

    fn label(&self) -> &'static str {
        self.label
    }

    fn category(&self) -> &'static str {
        "Test"
    }

    fn execution_kind(&self) -> ExecutionKind {
        ExecutionKind::Pure
    }

    fn role_capabilities(&self) -> Vec<ANodeRoleCapability> {
        vec![ANodeRoleCapability {
            role: SurfaceItemKind::Filter,
            primary_input: Some(SocketId::new("value")),
            primary_output: Some(SocketId::new("value")),
            autowire: AutoWirePolicy::UnaryTransform {
                input: SocketId::new("value"),
                output: SocketId::new("value"),
            },
            cardinality: self.cardinality,
            ui_mode: ManagedUiMode::CompactRow,
        }]
    }

    fn signature(
        &self,
        _ctx: &SignatureCtx<'_>,
        _instance: &ANodeInstance,
        _bindings: &TypeBindings,
    ) -> ANodeSignature {
        ANodeSignature {
            inputs: vec![
                InputSocketDecl::new("value", "Value", TypeConstraint::Any).with_default(RuntimeValue::Float(0.0)),
            ],
            outputs: vec![OutputSocketDecl::new(
                "value",
                "Value",
                TypeConstraint::Exact(ValueTypeId::new(self.output_type)),
            )],
            ..ANodeSignature::default()
        }
    }
}
