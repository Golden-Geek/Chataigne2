use golden_alchemist::{
    ANodeFieldPath, ANodeId, ANodeInstance, ANodeTypeId, AlchemistFormula, AlchemistGraph, FormulaContextContract,
    FormulaFamily, FormulaId, FormulaSurface, ParamUiHints, SurfaceItem, SurfaceItemId, SurfaceItemKind,
    SurfaceSection, SurfaceSectionId, SurfaceSource, ValueTypeId, ValueTypeSpec,
};

#[must_use]
pub fn builtin_formulas() -> Vec<AlchemistFormula> {
    vec![action_formula(), mapping_formula()]
}

fn action_formula() -> AlchemistFormula {
    let (graph, nodes) = graph(
        "Action",
        &["gate", "chataigne.command_builder", "chataigne.command_intent_output"],
    );
    let surface = FormulaSurface {
        sections: vec![
            section(
                "conditions",
                "Conditions",
                vec![item(
                    "condition",
                    "Condition",
                    SurfaceItemKind::Input,
                    "bool",
                    nodes[0],
                    "condition",
                )],
            ),
            section(
                "consequences_true",
                "Consequences when true",
                vec![item(
                    "target",
                    "Target",
                    SurfaceItemKind::Output,
                    "chataigne.command_target",
                    nodes[1],
                    "target",
                )],
            ),
            section("consequences_false", "Consequences when false", Vec::new()),
            section(
                "options",
                "Options",
                vec![item(
                    "priority",
                    "Priority",
                    SurfaceItemKind::Parameter,
                    "int",
                    nodes[1],
                    "priority",
                )],
            ),
        ],
    };
    formula("action", "Action", FormulaFamily::Action, graph, surface)
}

fn mapping_formula() -> AlchemistFormula {
    let (graph, nodes) = graph(
        "Mapping",
        &[
            "chataigne.module_value_input",
            "map_range",
            "clamp",
            "chataigne.command_builder",
            "chataigne.command_intent_output",
        ],
    );
    let surface = FormulaSurface {
        sections: vec![
            section(
                "input",
                "Input",
                vec![item(
                    "source",
                    "Source",
                    SurfaceItemKind::Input,
                    "float",
                    nodes[0],
                    "source",
                )],
            ),
            section(
                "filters",
                "Filters",
                vec![
                    item(
                        "input_min",
                        "Input minimum",
                        SurfaceItemKind::Parameter,
                        "float",
                        nodes[1],
                        "input_min",
                    ),
                    item(
                        "input_max",
                        "Input maximum",
                        SurfaceItemKind::Parameter,
                        "float",
                        nodes[1],
                        "input_max",
                    ),
                ],
            ),
            section(
                "outputs",
                "Outputs",
                vec![item(
                    "target",
                    "Target",
                    SurfaceItemKind::Output,
                    "chataigne.command_target",
                    nodes[3],
                    "target",
                )],
            ),
            section("options", "Options", Vec::new()),
        ],
    };
    formula("mapping", "Mapping", FormulaFamily::Mapping, graph, surface)
}

fn graph(label: &str, node_types: &[&str]) -> (AlchemistGraph, Vec<ANodeId>) {
    let mut graph = AlchemistGraph::new();
    graph.metadata.label = label.into();
    let nodes = node_types
        .iter()
        .map(|type_id| {
            graph
                .add_node(ANodeInstance::new(ANodeTypeId::new(*type_id), title(type_id)))
                .expect("new graph node IDs are unique")
        })
        .collect();
    (graph, nodes)
}

fn formula(
    id: &str,
    label: &str,
    family: FormulaFamily,
    graph: AlchemistGraph,
    surface: FormulaSurface,
) -> AlchemistFormula {
    AlchemistFormula {
        id: FormulaId::new(id),
        version: 1,
        label: label.into(),
        family,
        graph,
        surface,
        context_contract: FormulaContextContract {
            accepts_additional_dimensions: true,
            ..FormulaContextContract::default()
        },
        migrations: Vec::new(),
    }
}

fn section(id: &str, label: &str, items: Vec<SurfaceItem>) -> SurfaceSection {
    SurfaceSection {
        id: SurfaceSectionId::new(id),
        label: label.into(),
        items,
        source: SurfaceSource::Formula,
    }
}

fn item(id: &str, label: &str, kind: SurfaceItemKind, value_type: &str, node: ANodeId, field: &str) -> SurfaceItem {
    SurfaceItem {
        id: SurfaceItemId::new(id),
        label: label.into(),
        description: None,
        kind,
        value_type: Some(ValueTypeSpec::Exact(ValueTypeId::new(value_type))),
        ui: ParamUiHints::default(),
        binding: Some(ANodeFieldPath::new(node, field)),
    }
}

fn title(value: &str) -> String {
    value
        .split(['_', '.'])
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        })
        .collect::<Vec<String>>()
        .join(" ")
}
