use crate::test_support::TestGraph;

use crate::{
    AEdge, ANodeDeclaration, ANodeFieldPath, ANodeInstance, ANodeTypeId, AlchemistFormula, AlchemistGraphDomain,
    FormulaContextContract, FormulaId, FormulaMaterializationError, FormulaPropertySchema, FormulaSurface,
    InputSocketRef, ManagedItemId, ManagedItemInstance, ManagedItemUiState, ManagedRegionDefinition, ManagedRegionId,
    ManagedRegionInstance, ManagedRegionInstances, ManagedRegionKind, ManagedSocketRef, OutputSocketRef, ParamUiHints,
    PipelineLoweringCtx, PipelineLoweringDiagnosticKind, PrimitiveNodeDeclaration, PrimitiveNodeKind, RuntimeValue,
    SurfaceItem, SurfaceItemId, SurfaceItemKind, SurfaceSection, SurfaceSectionId, SurfaceSource, ValueTypeId,
    ValueTypeRegistry, ValueTypeSpec, primitive_node_registry, single_shape, value_set_shape,
};

#[test]
fn formula_instance_references_shared_definition_and_materializes_overrides() {
    let mut graph = TestGraph::new();
    let mut constant = ANodeInstance::new(ANodeTypeId::new("constant"), "Constant");
    constant.config.set("value", RuntimeValue::Float(1.0));
    let node = graph.add_node(constant).unwrap();
    let mut second = ANodeInstance::new(ANodeTypeId::new("property"), "Amount");
    second.config.set("value", RuntimeValue::Float(1.0));
    let second_node = graph.add_node(second).unwrap();
    let surface = FormulaSurface {
        sections: vec![SurfaceSection {
            id: SurfaceSectionId::new("value"),
            label: "Value".into(),
            items: vec![SurfaceItem {
                id: SurfaceItemId::new("amount"),
                label: "Amount".into(),
                description: None,
                path: vec!["Controls".into()],
                kind: SurfaceItemKind::Parameter,
                value_type: Some(ValueTypeSpec::Exact(ValueTypeId::new("float"))),
                ui: ParamUiHints::default(),
                bindings: vec![
                    ANodeFieldPath::new(node, "value"),
                    ANodeFieldPath::new(second_node, "value"),
                ],
            }],
            source: SurfaceSource::Formula,
        }],
        managed_regions: Vec::new(),
    };
    let formula = AlchemistFormula {
        id: FormulaId::new("test"),
        version: 3,
        label: "Test".into(),
        description: None,
        tags: Vec::new(),
        graph: graph.to_document(),
        properties: FormulaPropertySchema::default(),
        surface,
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    };

    let mut instance = formula.instantiate();
    instance
        .overrides
        .values
        .insert(SurfaceItemId::new("amount"), RuntimeValue::Float(2.5));
    let materialized = formula
        .materialize(&instance, &AlchemistGraphDomain::with_primitives())
        .unwrap();

    assert_eq!(instance.formula_ref.id, formula.id);
    assert_eq!(instance.formula_ref.version, 3);
    assert!(
        instance
            .surface_bindings
            .bindings
            .contains_key(&SurfaceItemId::new("amount"))
    );
    assert_eq!(
        materialized
            .node(AlchemistGraphDomain::node_id(node))
            .unwrap()
            .data
            .config
            .get("value"),
        Some(&RuntimeValue::Float(2.5))
    );
    assert_eq!(
        materialized
            .node(AlchemistGraphDomain::node_id(second_node))
            .unwrap()
            .data
            .config
            .get("value"),
        Some(&RuntimeValue::Float(2.5))
    );
    assert_eq!(
        formula
            .graph
            .node(AlchemistGraphDomain::node_id(node))
            .unwrap()
            .data
            .config
            .get("value"),
        Some(&RuntimeValue::Float(1.0)),
        "materializing one Processor instance must not mutate the shared Formula"
    );
    assert_eq!(
        materialized.revision().sequence,
        formula.graph.revision().sequence + 1,
        "authoring overrides must be committed as one revisioned graph transaction"
    );
}

#[test]
fn formula_authoring_document_roundtrips_through_versioned_graph_envelope() {
    let mut graph = TestGraph::new();
    graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("constant"), "Constant"))
        .unwrap();
    let formula = formula_with_graph_and_surface(graph, FormulaSurface::default());

    let encoded = serde_json::to_value(&formula).unwrap();

    assert_eq!(
        encoded["graph"]["domain_id"],
        serde_json::Value::String(AlchemistGraphDomain::DOMAIN_ID.into())
    );
    let decoded: AlchemistFormula = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, formula);
}

#[test]
fn managed_region_kind_roundtrips_through_json() {
    let encoded = serde_json::to_string(&ManagedRegionKind::FilterPipeline).unwrap();
    assert_eq!(encoded, "\"filter_pipeline\"");

    let decoded: ManagedRegionKind = serde_json::from_str("\"command_set\"").unwrap();
    assert_eq!(decoded, ManagedRegionKind::CommandSet);
}

#[test]
fn empty_managed_regions_are_instantiated_from_surface() {
    let surface = FormulaSurface {
        sections: Vec::new(),
        managed_regions: vec![
            region(
                "inputs",
                ManagedRegionKind::InputSet,
                "Inputs",
                vec![SurfaceItemKind::Input],
            ),
            region(
                "filters",
                ManagedRegionKind::FilterPipeline,
                "Filters",
                vec![SurfaceItemKind::Filter],
            ),
        ],
    };

    let instances = ManagedRegionInstances::empty_for(&surface);

    assert_eq!(instances.regions.len(), 2);
    assert_eq!(
        instances
            .regions
            .get(&ManagedRegionId::new("inputs"))
            .map(|region| region.items.len()),
        Some(0)
    );
    instances
        .validate_against(&surface)
        .expect("empty regions should be valid");
}

#[test]
fn invalid_managed_region_reference_reports_diagnostic() {
    let surface = FormulaSurface {
        sections: Vec::new(),
        managed_regions: vec![region(
            "inputs",
            ManagedRegionKind::InputSet,
            "Inputs",
            vec![SurfaceItemKind::Input],
        )],
    };
    let mut instances = ManagedRegionInstances::empty_for(&surface);
    instances.regions.insert(
        ManagedRegionId::new("missing"),
        ManagedRegionInstance {
            region_id: ManagedRegionId::new("missing"),
            items: Vec::new(),
        },
    );

    let error = instances
        .validate_against(&surface)
        .expect_err("unknown region should be rejected");

    assert!(error.to_string().contains("unknown region `missing`"));
}

#[test]
fn materialize_with_filter_pipelines_lowers_managed_filter_items() {
    let mut graph = TestGraph::new();
    let input = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("constant"), "Boundary Input"))
        .unwrap();
    let output = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("debug_value"), "Boundary Output"))
        .unwrap();
    let surface = FormulaSurface {
        sections: Vec::new(),
        managed_regions: vec![filter_region(input, output)],
    };
    let formula = formula_with_graph_and_surface(graph, surface);
    let mut instance = formula.instantiate();
    let remap = managed_filter_item(PrimitiveNodeKind::Remap);
    let remap_id = remap.anode.id;
    instance
        .managed_regions
        .regions
        .get_mut(&ManagedRegionId::new("filters"))
        .unwrap()
        .items
        .push(remap);
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();

    let materialized = formula
        .materialize_with_filter_pipelines(
            &instance,
            &PipelineLoweringCtx {
                value_types: &value_types,
                nodes: &nodes,
                properties: None,
            },
            &[(ManagedRegionId::new("filters"), single_shape("float"))],
        )
        .unwrap();

    assert!(materialized.node(AlchemistGraphDomain::node_id(remap_id)).is_some());
    assert_eq!(
        AlchemistGraphDomain::with_primitives()
            .semantic_edges(&materialized)
            .unwrap(),
        vec![
            AEdge {
                from: OutputSocketRef::new(input, "value"),
                to: InputSocketRef::new(remap_id, "value"),
            },
            AEdge {
                from: OutputSocketRef::new(remap_id, "result"),
                to: InputSocketRef::new(output, "value"),
            },
        ]
    );
}

#[test]
fn materialize_with_filter_pipelines_requires_initial_shape() {
    let mut graph = TestGraph::new();
    let input = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("boundary_input"), "Boundary Input"))
        .unwrap();
    let output = graph
        .add_node(ANodeInstance::new(
            ANodeTypeId::new("boundary_output"),
            "Boundary Output",
        ))
        .unwrap();
    let surface = FormulaSurface {
        sections: Vec::new(),
        managed_regions: vec![filter_region(input, output)],
    };
    let formula = formula_with_graph_and_surface(graph, surface);
    let instance = formula.instantiate();
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();

    let error = formula
        .materialize_with_filter_pipelines(
            &instance,
            &PipelineLoweringCtx {
                value_types: &value_types,
                nodes: &nodes,
                properties: None,
            },
            &[],
        )
        .expect_err("filter pipeline materialization needs an explicit starting shape");

    assert!(matches!(
        error,
        FormulaMaterializationError::MissingManagedRegionInitialShape { .. }
    ));
}

#[test]
fn materialize_with_filter_pipelines_rejects_valueset_elementwise_without_lane_strategy() {
    let mut graph = TestGraph::new();
    let input = graph
        .add_node(ANodeInstance::new(ANodeTypeId::new("boundary_input"), "Boundary Input"))
        .unwrap();
    let output = graph
        .add_node(ANodeInstance::new(
            ANodeTypeId::new("boundary_output"),
            "Boundary Output",
        ))
        .unwrap();
    let surface = FormulaSurface {
        sections: Vec::new(),
        managed_regions: vec![filter_region(input, output)],
    };
    let formula = formula_with_graph_and_surface(graph, surface);
    let mut instance = formula.instantiate();
    instance
        .managed_regions
        .regions
        .get_mut(&ManagedRegionId::new("filters"))
        .unwrap()
        .items
        .push(managed_filter_item(PrimitiveNodeKind::Remap));
    let value_types = ValueTypeRegistry::with_primitives();
    let nodes = primitive_node_registry();

    let error = formula
        .materialize_with_filter_pipelines(
            &instance,
            &PipelineLoweringCtx {
                value_types: &value_types,
                nodes: &nodes,
                properties: None,
            },
            &[(
                ManagedRegionId::new("filters"),
                value_set_shape("float", Some(crate::ContextAxisId::new("input_lane"))),
            )],
        )
        .expect_err("ValueSet elementwise lowering needs explicit lane support");

    match error {
        FormulaMaterializationError::ManagedRegionLoweringFailed {
            lowering_diagnostics,
            shape_diagnostics,
            ..
        } => {
            assert_eq!(shape_diagnostics.len(), 0);
            assert_eq!(lowering_diagnostics.len(), 1);
            assert_eq!(
                lowering_diagnostics[0].kind,
                PipelineLoweringDiagnosticKind::UnsupportedValueSetElementwise
            );
        }
        other => panic!("expected managed lowering failure, got {other:?}"),
    }
}

fn formula_with_graph_and_surface(graph: TestGraph, surface: FormulaSurface) -> AlchemistFormula {
    AlchemistFormula {
        id: FormulaId::new("test"),
        version: 1,
        label: "Test".into(),
        description: None,
        tags: Vec::new(),
        graph: graph.to_document(),
        properties: FormulaPropertySchema::default(),
        surface,
        context_contract: FormulaContextContract::default(),
        migrations: Vec::new(),
    }
}

fn filter_region(input: crate::ANodeId, output: crate::ANodeId) -> ManagedRegionDefinition {
    ManagedRegionDefinition {
        id: ManagedRegionId::new("filters"),
        kind: ManagedRegionKind::FilterPipeline,
        label: "Filters".into(),
        input_socket: Some(ManagedSocketRef::new(input, "value")),
        output_socket: Some(ManagedSocketRef::new(output, "value")),
        accepted_roles: vec![SurfaceItemKind::Filter],
    }
}

fn managed_filter_item(kind: PrimitiveNodeKind) -> ManagedItemInstance {
    let declaration = PrimitiveNodeDeclaration::new(kind);
    ManagedItemInstance {
        id: ManagedItemId::new(),
        anode: ANodeInstance::new(declaration.type_id(), declaration.label()),
        enabled: true,
        ui_state: ManagedItemUiState::default(),
    }
}

fn region(
    id: &str,
    kind: ManagedRegionKind,
    label: &str,
    accepted_roles: Vec<SurfaceItemKind>,
) -> ManagedRegionDefinition {
    ManagedRegionDefinition {
        id: ManagedRegionId::new(id),
        kind,
        label: label.into(),
        input_socket: None,
        output_socket: None,
        accepted_roles,
    }
}
