use std::any::Any;

use crate::{
    events::Event,
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::ProcessCtx,
};

use super::{
    default_parameter_value_for_node_type, EventPropagation, Node, NodeData, NodeId,
    UserContainerRules, UserCreatableItem, FOLDER_NODE_TYPE, USER_CONTEXT_ALLOWED_ITEM_KINDS,
    USER_CONTEXT_DEFAULT_LABEL, USER_CONTEXT_NODE_TYPE,
};

/// Internal scope node used to host user-authored lexical context entries.
///
/// Context entries are regular descendant parameter nodes. Symbols are derived from
/// parameter `decl_id` values during resolver indexing.
pub struct UserContextNode {
    node_data: NodeData,
}

impl UserContextNode {
    /// Creates a new user-context scope node.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        Self { node_data }
    }
}

impl Node for UserContextNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        USER_CONTEXT_NODE_TYPE
    }

    fn type_description(&self) -> Option<&str> {
        Some(
            "Internal scope node used to host user-authored lexical context entries.\n\nContext entries are regular descendant parameter nodes. Symbols are derived from parameter decl_id values during resolver indexing.",
        )
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&USER_CONTEXT_ALLOWED_ITEM_KINDS))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = vec![UserCreatableItem::new(FOLDER_NODE_TYPE, FOLDER_NODE_TYPE, "Folder")];
        items.extend([
            UserCreatableItem::new("trigger", "trigger", "Trigger"),
            UserCreatableItem::new("int", "int", "Int"),
            UserCreatableItem::new("float", "float", "Float"),
            UserCreatableItem::new("str", "str", "String"),
            UserCreatableItem::new("file", "file", "File"),
            UserCreatableItem::new("enum", "enum", "Enum"),
            UserCreatableItem::new("bool", "bool", "Bool"),
            UserCreatableItem::new("vec2", "vec2", "Vec2"),
            UserCreatableItem::new("vec3", "vec3", "Vec3"),
            UserCreatableItem::new("color", "color", "Color"),
            UserCreatableItem::new("reference", "reference", "Reference"),
        ]);
        items
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        let node_type = node_type.trim().to_ascii_lowercase();
        if node_type == FOLDER_NODE_TYPE {
            return Some(Box::new(Folder::new("Folder")));
        }

        if node_type == "param" || node_type == "parameter" {
            return Some(Box::new(Parameter::new(
                "Parameter",
                ParamValue::Float(0.0),
                ParameterChangeCheck::ValueChange,
            )));
        }

        let default_value = default_parameter_value_for_node_type(node_type.as_str())?;
        let default_label = match node_type.as_str() {
            "trigger" => "Trigger",
            "int" => "Int",
            "float" => "Float",
            "str" => "String",
            "file" => "File",
            "enum" => "Enum",
            "bool" => "Bool",
            "vec2" => "Vec2",
            "vec3" => "Vec3",
            "color" => "Color",
            "reference" => "Reference",
            _ => node_type.as_str(),
        };
        Some(Box::new(Parameter::new(
            default_label,
            default_value,
            ParameterChangeCheck::ValueChange,
        )))
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == USER_CONTEXT_NODE_TYPE).then(|| Self::new(USER_CONTEXT_DEFAULT_LABEL))
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}

pub(crate) fn parameter_child_exists(ctx: &ProcessCtx, parent: NodeId, decl_id: &str) -> bool {
    ctx.tree_snapshot()
        .and_then(|snapshot| snapshot.find_child(parent, decl_id))
        .is_some()
}

/// Internal Folder-like node used as empty organizational structure and root for user content without process or bubbling.
pub struct Folder {
    node_data: NodeData,
}

impl Folder {
    /// Creates a new folder node.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        Self { node_data }
    }
}

impl Node for Folder {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        FOLDER_NODE_TYPE
    }

    fn type_description(&self) -> Option<&str> {
        Some(
            "Internal folder-like node used as empty organizational structure and root for user content without process or bubbling.",
        )
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == FOLDER_NODE_TYPE).then(|| Self::new("Folder"))
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }
}
