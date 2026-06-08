use indexmap::IndexSet;

use golden_alchemist::{
    ANodeFieldPath, ANodeInstance, ANodeTypeId, AlchemistGraph, ExposedDeclId, ExposedParam, ExposedSurface,
    ParamUiHints, ValueTypeId, ValueTypeSpec,
};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProcessorModelId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessorCategory {
    InputCondition,
    Action,
    Mapping,
    Multiplex,
    SequenceLauncher,
    StateController,
    Conductor,
}

#[derive(Clone, Debug)]
pub struct ProcessorModel {
    pub id: ProcessorModelId,
    pub version: u32,
    pub label: String,
    pub category: ProcessorCategory,
    pub graph_template: AlchemistGraph,
    pub exposed_surface: ExposedSurface,
}

#[derive(Clone, Debug)]
pub struct ProcessorModelInstance {
    pub model_id: ProcessorModelId,
    pub model_version: u32,
    pub graph_instance: AlchemistGraph,
    pub overrides: IndexSet<ExposedDeclId>,
}

impl ProcessorModel {
    #[must_use]
    pub fn instantiate(&self) -> ProcessorModelInstance {
        ProcessorModelInstance {
            model_id: self.id.clone(),
            model_version: self.version,
            graph_instance: self.graph_template.clone(),
            overrides: IndexSet::new(),
        }
    }
}

#[must_use]
pub fn builtin_processor_models() -> Vec<ProcessorModel> {
    vec![
        model(
            "input_condition",
            "Input Condition",
            ProcessorCategory::InputCondition,
            &["chataigne.module_value_input", "compare", "edge"],
            &[("source", "bool"), ("expected", "bool")],
        ),
        model(
            "action",
            "Action",
            ProcessorCategory::Action,
            &["gate", "chataigne.command_builder", "chataigne.command_intent_output"],
            &[("target", "chataigne.command_target"), ("priority", "int")],
        ),
        model(
            "mapping",
            "Mapping",
            ProcessorCategory::Mapping,
            &[
                "chataigne.module_value_input",
                "map_range",
                "clamp",
                "chataigne.command_builder",
                "chataigne.command_intent_output",
            ],
            &[("source", "float"), ("target", "chataigne.command_target")],
        ),
        model(
            "multiplex",
            "Multiplex",
            ProcessorCategory::Multiplex,
            &["gate", "chataigne.command_intent_output"],
            &[("selector", "bool"), ("target", "chataigne.command_target")],
        ),
        model(
            "sequence_launcher",
            "Sequence Launcher",
            ProcessorCategory::SequenceLauncher,
            &["chataigne.sequence_intent_output"],
            &[("sequence", "chataigne.sequence")],
        ),
        model(
            "state_controller",
            "State Controller",
            ProcessorCategory::StateController,
            &["chataigne.state_transition_intent_output"],
            &[("state", "chataigne.state")],
        ),
        model(
            "conductor_v0",
            "Conductor",
            ProcessorCategory::Conductor,
            &["chataigne.command_intent_output"],
            &[("priority_domain", "int"), ("target_lock", "bool")],
        ),
    ]
}

fn model(
    id: &str,
    label: &str,
    category: ProcessorCategory,
    node_types: &[&str],
    exposed: &[(&str, &str)],
) -> ProcessorModel {
    let mut graph = AlchemistGraph::new();
    graph.metadata.label = label.into();
    let nodes: Vec<_> = node_types
        .iter()
        .map(|type_id| {
            graph
                .add_node(ANodeInstance::new(ANodeTypeId::new(*type_id), *type_id))
                .expect("new graph node IDs are unique")
        })
        .collect();
    for (index, (decl_id, value_type)) in exposed.iter().enumerate() {
        graph.exposed.params.push(ExposedParam {
            decl_id: ExposedDeclId::new(*decl_id),
            label: title(decl_id),
            description: None,
            target: ANodeFieldPath::new(nodes[index.min(nodes.len() - 1)], *decl_id),
            value_type: ValueTypeSpec::Exact(ValueTypeId::new(*value_type)),
            ui: ParamUiHints::default(),
        });
    }
    ProcessorModel {
        id: ProcessorModelId(id.into()),
        version: 1,
        label: label.into(),
        category,
        exposed_surface: graph.exposed.clone(),
        graph_template: graph,
    }
}

fn title(value: &str) -> String {
    value
        .split('_')
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
