use golden_core::{
    item, node,
    node::{Node, NodeUserPermissions, UserContainerRules, UserCreatableItem},
    process_ctx::ProcessCtx,
};

pub(crate) const PROCESSOR_ITEM_KIND: &str = "state_processor";
pub(crate) const PROCESSOR_FOLDER_ITEM_KIND: &str = "state_processor_folder";
pub(crate) const PROCESSOR_FOLDER_NODE_TYPE: &str = "state_processor_folder";

fn processor_items() -> Vec<UserCreatableItem> {
    crate::app::declared_user_creatable_items(PROCESSOR_ITEM_KIND)
}

fn processor_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[PROCESSOR_ITEM_KIND, PROCESSOR_FOLDER_ITEM_KIND])
}

fn processor_container_accepts(item_type: &str, item_kind: &str) -> bool {
    (item_kind == PROCESSOR_ITEM_KIND
        && crate::app::declared_user_item_type_matches(item_type, PROCESSOR_ITEM_KIND))
        || (item_type == PROCESSOR_FOLDER_NODE_TYPE && item_kind == PROCESSOR_FOLDER_ITEM_KIND)
}

fn processor_creatable_items() -> Vec<UserCreatableItem> {
    let mut items = processor_items();
    items.push(UserCreatableItem::new(
        PROCESSOR_FOLDER_NODE_TYPE,
        PROCESSOR_FOLDER_ITEM_KIND,
        "Folder",
    ));
    items
}

fn create_processor_item(node_type: &str) -> Option<Box<dyn Node>> {
    crate::app::create_declared_user_item(node_type, PROCESSOR_ITEM_KIND).or_else(|| {
        (node_type == PROCESSOR_FOLDER_NODE_TYPE)
            .then(|| Box::new(StateProcessorFolder::new()) as Box<dyn Node>)
    })
}

fn initialize_processor_item(node: &mut dyn Node) {
    node.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
}

#[node(
    "state_processor_manager",
    label = "Processors",
    presentation = golden_core::node::PresentationHint {
        show_in_nested_inspector: false,
        ..Default::default()
    }
)]
pub struct StateProcessorManager {}

#[node("state_processor_manager", from_struct)]
impl Node for StateProcessorManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(processor_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        processor_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        processor_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_processor_item(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }
}

#[node("state_processor_folder", label = "Folder")]
pub struct StateProcessorFolder {}

#[node("state_processor_folder", from_struct)]
impl Node for StateProcessorFolder {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(processor_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        processor_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        processor_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_processor_item(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

const ACTION_GRAPH_TEMPLATE: &str = r#"{
  "version": 1,
  "nodes": [
    {
      "id": "condition",
      "typeId": "condition",
      "label": "Condition",
      "x": 2.0,
      "y": 4.0,
      "inputs": [],
      "outputs": [{ "id": "result", "label": "Result", "valueType": "bool" }]
    },
    {
      "id": "edge",
      "typeId": "edge_detector",
      "label": "Edge",
      "x": 18.0,
      "y": 4.0,
      "inputs": [{ "id": "value", "label": "Value", "valueType": "bool" }],
      "outputs": [
        { "id": "true", "label": "True", "valueType": "trigger" },
        { "id": "false", "label": "False", "valueType": "trigger" }
      ]
    },
    {
      "id": "true_consequence",
      "typeId": "consequence",
      "label": "True Consequence",
      "x": 35.0,
      "y": 0.0,
      "inputs": [{ "id": "trigger", "label": "Trigger", "valueType": "trigger" }],
      "outputs": []
    },
    {
      "id": "false_consequence",
      "typeId": "consequence",
      "label": "False Consequence",
      "x": 35.0,
      "y": 10.0,
      "inputs": [{ "id": "trigger", "label": "Trigger", "valueType": "trigger" }],
      "outputs": []
    }
  ],
  "edges": [
    {
      "id": "condition-to-edge",
      "from": { "nodeId": "condition", "socketId": "result" },
      "to": { "nodeId": "edge", "socketId": "value" }
    },
    {
      "id": "edge-to-true",
      "from": { "nodeId": "edge", "socketId": "true" },
      "to": { "nodeId": "true_consequence", "socketId": "trigger" }
    },
    {
      "id": "edge-to-false",
      "from": { "nodeId": "edge", "socketId": "false" },
      "to": { "nodeId": "false_consequence", "socketId": "trigger" }
    }
  ]
}"#;

const MAPPING_GRAPH_TEMPLATE: &str = r#"{
  "version": 1,
  "nodes": [
    {
      "id": "input",
      "typeId": "input",
      "label": "Input",
      "x": 2.0,
      "y": 4.0,
      "inputs": [],
      "outputs": [{ "id": "value", "label": "Value", "valueType": "number" }]
    },
    {
      "id": "range",
      "typeId": "map_range",
      "label": "Map Range",
      "x": 18.0,
      "y": 4.0,
      "inputs": [{ "id": "value", "label": "Value", "valueType": "number" }],
      "outputs": [{ "id": "value", "label": "Value", "valueType": "number" }]
    },
    {
      "id": "smoothing",
      "typeId": "smoothing",
      "label": "Smoothing",
      "x": 34.0,
      "y": 4.0,
      "inputs": [{ "id": "value", "label": "Value", "valueType": "number" }],
      "outputs": [{ "id": "value", "label": "Value", "valueType": "number" }]
    },
    {
      "id": "output",
      "typeId": "output",
      "label": "Output",
      "x": 50.0,
      "y": 4.0,
      "inputs": [{ "id": "value", "label": "Value", "valueType": "number" }],
      "outputs": []
    }
  ],
  "edges": [
    {
      "id": "input-to-range",
      "from": { "nodeId": "input", "socketId": "value" },
      "to": { "nodeId": "range", "socketId": "value" }
    },
    {
      "id": "range-to-smoothing",
      "from": { "nodeId": "range", "socketId": "value" },
      "to": { "nodeId": "smoothing", "socketId": "value" }
    },
    {
      "id": "smoothing-to-output",
      "from": { "nodeId": "smoothing", "socketId": "value" },
      "to": { "nodeId": "output", "socketId": "value" }
    }
  ]
}"#;

fn action_graph_json() -> String {
    ACTION_GRAPH_TEMPLATE.to_owned()
}

fn mapping_graph_json() -> String {
    MAPPING_GRAPH_TEMPLATE.to_owned()
}

#[node("state_processor_action", label = "Action")]
#[children(
    enabled: bool = true (
        label = "Enabled",
        description = "Whether this action formula is evaluated."
    );
    condition: bool = false (
        label = "Condition",
        description = "The condition value consumed by the action formula."
    );
    true_command: String = String::new() (
        label = "True Command",
        description = "Command invoked when the condition enters the configured true edge."
    );
    false_command: String = String::new() (
        label = "False Command",
        description = "Command invoked when the condition enters the configured false edge."
    );
    edge_mode: String = "Both".to_owned() (
        label = "Edge Mode",
        description = "Selects which condition edges may invoke consequences."
    );
    cooldown_ms: f64 = 0.0 (
        label = "Cooldown (ms)",
        description = "Minimum interval between consequence invocations."
    );
    authored_graph: String = action_graph_json() (
        label = "Authored Graph",
        description = "Versioned Alchemist graph document for this action.",
        show_in_inspector_content = false
    );
)]
pub struct ActionStateProcessor {}

#[item(
    "state_processor",
    node = "state_processor_action",
    from_struct,
    menu_path = ["Built-in"]
)]
impl Node for ActionStateProcessor {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("state_processor_mapping", label = "Mapping")]
#[children(
    enabled: bool = true (
        label = "Enabled",
        description = "Whether this mapping formula is evaluated."
    );
    input_value: f64 = 0.0 (
        label = "Input Value",
        description = "The value consumed by the mapping formula."
    );
    input_min: f64 = 0.0 (
        label = "Input Minimum",
        description = "Lower bound of the input range."
    );
    input_max: f64 = 127.0 (
        label = "Input Maximum",
        description = "Upper bound of the input range."
    );
    output_min: f64 = 0.0 (
        label = "Output Minimum",
        description = "Lower bound of the output range."
    );
    output_max: f64 = 1.0 (
        label = "Output Maximum",
        description = "Upper bound of the output range."
    );
    smoothing_ms: f64 = 100.0 (
        label = "Smoothing (ms)",
        description = "Smoothing duration applied to the mapped output."
    );
    output_target: String = String::new() (
        label = "Output Target",
        description = "Parameter or command target receiving the mapped value."
    );
    authored_graph: String = mapping_graph_json() (
        label = "Authored Graph",
        description = "Versioned Alchemist graph document for this mapping.",
        show_in_inspector_content = false
    );
)]
pub struct MappingStateProcessor {}

#[item(
    "state_processor",
    node = "state_processor_mapping",
    from_struct,
    menu_path = ["Built-in"]
)]
impl Node for MappingStateProcessor {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("state_processor_custom", label = "Custom Processor")]
pub struct CustomStateProcessor {}

#[item("state_processor", node = "state_processor_custom", from_struct)]
impl Node for CustomStateProcessor {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod state_machine_processor_tests;
