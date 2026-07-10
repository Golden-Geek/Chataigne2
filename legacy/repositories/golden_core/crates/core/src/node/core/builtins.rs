use std::any::Any;

use crate::{
    edit::{Edit, NodeTree},
    events::{Event, EventFrame},
    parameter::{ParamValue, Parameter, ParameterChangeCheck, ReferenceTargetKind},
    process_ctx::ProcessCtx,
};

use super::{
    EventPropagation, FOLDER_NODE_TYPE, Node, NodeCreationContext, NodeData, NodeId, NodeUserPermissions,
    PARAMETER_NODE_TYPES, USER_CONTEXT_ALLOWED_ITEM_KINDS, USER_CONTEXT_DEFAULT_LABEL, USER_CONTEXT_FOLDER_NODE_TYPE,
    USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID, USER_CONTEXT_MULTIPLEX_DEFAULT_LABEL, USER_CONTEXT_MULTIPLEX_ITEM_KIND,
    USER_CONTEXT_MULTIPLEX_LIST_ITEM_KIND, USER_CONTEXT_MULTIPLEX_LIST_NODE_TYPE_PREFIX,
    USER_CONTEXT_MULTIPLEX_NODE_TYPE, USER_CONTEXT_NODE_TYPE, UserContainerRules, UserCreatableItem,
    default_parameter_value_for_node_type,
};

/// Internal scope node used to host user-authored lexical context entries.
///
/// Context entries are regular descendant parameter nodes. Symbols are derived from
/// parameter `decl_id` values during resolver indexing.
pub struct UserContextNode {
    node_data: NodeData,
    multiplex_enabled: bool,
}

impl UserContextNode {
    /// Creates a new user-context scope node.
    pub fn new(label: impl Into<String>) -> Self {
        Self::new_with_multiplex(label, false)
    }

    /// Creates a new user-context scope node with optional multiplex authoring.
    pub fn new_with_multiplex(label: impl Into<String>, multiplex_enabled: bool) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        Self {
            node_data,
            multiplex_enabled,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
struct UserContextNodeProjectData {
    #[serde(default)]
    multiplex_enabled: bool,
}

fn user_context_container_rules() -> UserContainerRules {
    UserContainerRules::new(&USER_CONTEXT_ALLOWED_ITEM_KINDS)
}

fn user_context_container_accepts_item(item_type: &str, item_kind: &str, multiplex_enabled: bool) -> bool {
    if item_kind == FOLDER_NODE_TYPE {
        return item_type == USER_CONTEXT_FOLDER_NODE_TYPE;
    }
    if item_kind == USER_CONTEXT_MULTIPLEX_ITEM_KIND {
        return multiplex_enabled && item_type == USER_CONTEXT_MULTIPLEX_NODE_TYPE;
    }
    if item_type == USER_CONTEXT_MULTIPLEX_NODE_TYPE || item_kind == USER_CONTEXT_MULTIPLEX_LIST_ITEM_KIND {
        return false;
    }
    if PARAMETER_NODE_TYPES.contains(&item_kind) {
        return item_type == item_kind;
    }
    user_context_container_rules().accepts(item_kind)
}

fn user_context_creatable_items(multiplex_enabled: bool) -> Vec<UserCreatableItem> {
    let mut items = vec![UserCreatableItem::new(
        USER_CONTEXT_FOLDER_NODE_TYPE,
        FOLDER_NODE_TYPE,
        "Folder",
    )];
    if multiplex_enabled {
        items.push(
            UserCreatableItem::new(
                USER_CONTEXT_MULTIPLEX_NODE_TYPE,
                USER_CONTEXT_MULTIPLEX_ITEM_KIND,
                USER_CONTEXT_MULTIPLEX_DEFAULT_LABEL,
            )
            .with_select_when_created(false),
        );
    }
    items.extend([
        UserCreatableItem::new("trigger", "trigger", "Trigger").with_select_when_created(false),
        UserCreatableItem::new("int", "int", "Int").with_select_when_created(false),
        UserCreatableItem::new("float", "float", "Float").with_select_when_created(false),
        UserCreatableItem::new("str", "str", "String").with_select_when_created(false),
        UserCreatableItem::new("file", "file", "File").with_select_when_created(false),
        UserCreatableItem::new("enum", "enum", "Enum").with_select_when_created(false),
        UserCreatableItem::new("bool", "bool", "Bool").with_select_when_created(false),
        UserCreatableItem::new("vec2", "vec2", "Vec2").with_select_when_created(false),
        UserCreatableItem::new("vec3", "vec3", "Vec3").with_select_when_created(false),
        UserCreatableItem::new("color", "color", "Color").with_select_when_created(false),
        UserCreatableItem::new("reference", "reference", "Reference").with_select_when_created(false),
    ]);
    items
}

fn create_user_context_item(node_type: &str, multiplex_enabled: bool) -> Option<Box<dyn Node>> {
    let node_type = node_type.trim().to_ascii_lowercase();
    if node_type == USER_CONTEXT_FOLDER_NODE_TYPE {
        return Some(Box::new(UserContextFolder::new_with_multiplex(
            "Folder",
            multiplex_enabled,
        )));
    }

    if multiplex_enabled && node_type == USER_CONTEXT_MULTIPLEX_NODE_TYPE {
        return Some(Box::new(UserContextMultiplexNode::new(
            USER_CONTEXT_MULTIPLEX_DEFAULT_LABEL,
        )));
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

fn create_user_context_item_tree(node_type: &str, multiplex_enabled: bool) -> Option<NodeTree> {
    let node_type = node_type.trim().to_ascii_lowercase();
    if multiplex_enabled && node_type == USER_CONTEXT_MULTIPLEX_NODE_TYPE {
        return Some(user_context_multiplex_node_tree(USER_CONTEXT_MULTIPLEX_DEFAULT_LABEL));
    }
    create_user_context_item(node_type.as_str(), multiplex_enabled).map(NodeTree::boxed)
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
        Some(user_context_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        user_context_container_accepts_item(item_type, item_kind, self.multiplex_enabled)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        user_context_creatable_items(self.multiplex_enabled)
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_user_context_item(node_type, self.multiplex_enabled)
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        create_user_context_item_tree(node_type, self.multiplex_enabled)
    }

    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(UserContextNodeProjectData {
            multiplex_enabled: self.multiplex_enabled,
        })
        .map_err(|err| err.to_string())
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        if data.is_null() {
            return Ok(());
        }
        let parsed: UserContextNodeProjectData = serde_json::from_value(data.clone()).map_err(|err| err.to_string())?;
        self.multiplex_enabled = parsed.multiplex_enabled;
        Ok(())
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == USER_CONTEXT_NODE_TYPE).then(|| Self::new(USER_CONTEXT_DEFAULT_LABEL))
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }

    fn inbox_requires_tree_snapshot(&self, _events: &EventFrame) -> bool {
        false
    }
}

/// Folder node used inside a user-context scope.
///
/// It keeps context-entry organization typed so generic folders do not become
/// implicit user-context containers.
pub struct UserContextFolder {
    node_data: NodeData,
    multiplex_enabled: bool,
}

impl UserContextFolder {
    /// Creates a new user-context folder node.
    pub fn new(label: impl Into<String>) -> Self {
        Self::new_with_multiplex(label, false)
    }

    /// Creates a new user-context folder node with optional multiplex authoring.
    pub fn new_with_multiplex(label: impl Into<String>, multiplex_enabled: bool) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        Self {
            node_data,
            multiplex_enabled,
        }
    }
}

impl Node for UserContextFolder {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        USER_CONTEXT_FOLDER_NODE_TYPE
    }

    fn user_item_kind(&self) -> &str {
        FOLDER_NODE_TYPE
    }

    fn type_description(&self) -> Option<&str> {
        Some("Folder node used inside a user-context scope.")
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(user_context_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        user_context_container_accepts_item(item_type, item_kind, self.multiplex_enabled)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        user_context_creatable_items(self.multiplex_enabled)
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_user_context_item(node_type, self.multiplex_enabled)
    }

    fn create_user_item_tree(&self, node_type: &str) -> Option<NodeTree> {
        create_user_context_item_tree(node_type, self.multiplex_enabled)
    }

    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(UserContextNodeProjectData {
            multiplex_enabled: self.multiplex_enabled,
        })
        .map_err(|err| err.to_string())
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        if data.is_null() {
            return Ok(());
        }
        let parsed: UserContextNodeProjectData = serde_json::from_value(data.clone()).map_err(|err| err.to_string())?;
        self.multiplex_enabled = parsed.multiplex_enabled;
        Ok(())
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == USER_CONTEXT_FOLDER_NODE_TYPE).then(|| Self::new("Folder"))
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }

    fn inbox_requires_tree_snapshot(&self, _events: &EventFrame) -> bool {
        false
    }
}

/// Multiplex axis node inside a user-context scope.
pub struct UserContextMultiplexNode {
    node_data: NodeData,
}

impl UserContextMultiplexNode {
    /// Creates a new multiplex node.
    pub fn new(label: impl Into<String>) -> Self {
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        Self { node_data }
    }
}

fn user_context_multiplex_node_tree(label: impl Into<String>) -> NodeTree {
    NodeTree::new(UserContextMultiplexNode::new(label))
        .with_child(NodeTree::new(user_context_multiplex_count_parameter()))
}

fn user_context_multiplex_count_parameter() -> Parameter {
    let mut count = Parameter::new("Count", ParamValue::Int(0), ParameterChangeCheck::ValueChange);
    count.node_data_mut().meta.decl_id.0 = USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID.to_string();
    count
}

fn multiplex_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[USER_CONTEXT_MULTIPLEX_LIST_ITEM_KIND])
}

fn multiplex_creatable_items() -> Vec<UserCreatableItem> {
    PARAMETER_NODE_TYPES
        .into_iter()
        .map(|value_type| {
            UserCreatableItem::new(
                user_context_multiplex_list_node_type(value_type),
                USER_CONTEXT_MULTIPLEX_LIST_ITEM_KIND,
                format!("{} List", user_context_parameter_type_label(value_type)),
            )
            .with_select_when_created(false)
        })
        .collect()
}

impl Node for UserContextMultiplexNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        USER_CONTEXT_MULTIPLEX_NODE_TYPE
    }

    fn user_item_kind(&self) -> &str {
        USER_CONTEXT_MULTIPLEX_ITEM_KIND
    }

    fn type_description(&self) -> Option<&str> {
        Some("Multiplex axis node used inside a user-authored context scope.")
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(multiplex_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_kind == USER_CONTEXT_MULTIPLEX_LIST_ITEM_KIND
            && user_context_multiplex_list_value_type(item_type).is_some()
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        multiplex_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        let value_type = user_context_multiplex_list_value_type(node_type)?;
        Some(Box::new(UserContextMultiplexListNode::new(
            format!("{} List", user_context_parameter_type_label(value_type)),
            value_type,
        )))
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == USER_CONTEXT_MULTIPLEX_NODE_TYPE).then(|| Self::new(USER_CONTEXT_MULTIPLEX_DEFAULT_LABEL))
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if ctx.tree_snapshot().is_some_and(|snapshot| {
            snapshot
                .find_child_by_decl_id(self.id(), USER_CONTEXT_MULTIPLEX_COUNT_DECL_ID)
                .is_some()
        }) {
            return;
        }
        ctx.edits.push(Edit::AddNode {
            parent: self.id(),
            prev_sibling: None,
            node: Box::new(user_context_multiplex_count_parameter()),
        });
    }

    fn needs_update(&self) -> bool {
        false
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        false
    }
}

/// Typed multiplex list node. Its direct parameter children are lane entries.
pub struct UserContextMultiplexListNode {
    node_data: NodeData,
    node_type: String,
    value_type: String,
}

impl UserContextMultiplexListNode {
    /// Creates a new typed multiplex list node.
    pub fn new(label: impl Into<String>, value_type: impl Into<String>) -> Self {
        let value_type = value_type.into();
        let mut node_data = NodeData::new(label.into());
        node_data.meta.can_be_disabled = false;
        node_data.meta.decl_id.0 = value_type.clone();
        let node_type = user_context_multiplex_list_node_type(value_type.as_str());
        Self {
            node_data,
            node_type,
            value_type,
        }
    }

    /// Returns the list parameter value type id.
    pub fn value_type(&self) -> &str {
        &self.value_type
    }
}

impl Node for UserContextMultiplexListNode {
    fn node_data(&self) -> &NodeData {
        &self.node_data
    }

    fn node_data_mut(&mut self) -> &mut NodeData {
        &mut self.node_data
    }

    fn get_type(&self) -> &str {
        &self.node_type
    }

    fn user_item_kind(&self) -> &str {
        USER_CONTEXT_MULTIPLEX_LIST_ITEM_KIND
    }

    fn type_description(&self) -> Option<&str> {
        Some("Typed multiplex list. Direct parameter children are lane entries.")
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn project_encode_data(&self) -> Result<serde_json::Value, String> {
        serde_json::to_value(&self.value_type).map_err(|err| err.to_string())
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        if data.is_null() {
            return Ok(());
        }
        let value_type = serde_json::from_value::<String>(data.clone()).map_err(|err| err.to_string())?;
        if user_context_multiplex_list_node_type(value_type.as_str()) != self.get_type() {
            return Err("multiplex list node type/data mismatch".to_string());
        }
        self.node_type = user_context_multiplex_list_node_type(value_type.as_str());
        self.value_type = value_type;
        Ok(())
    }

    fn project_create(node_type: &str) -> Option<Self> {
        let value_type = user_context_multiplex_list_value_type(node_type)?;
        Some(Self::new(
            format!("{} List", user_context_parameter_type_label(value_type)),
            value_type,
        ))
    }

    fn event_propagation(&self, _: &Event, _: u32) -> EventPropagation {
        EventPropagation::PassOn
    }

    fn inbox_requires_tree_snapshot(&self, _events: &EventFrame) -> bool {
        false
    }
}

pub(crate) fn user_context_multiplex_entry_parameter(value_type: &str) -> Option<Parameter> {
    let default_value = default_parameter_value_for_node_type(value_type)?;
    let is_reference = matches!(default_value, ParamValue::Reference(_));
    let mut parameter = Parameter::new(
        user_context_parameter_type_label(value_type),
        default_value,
        ParameterChangeCheck::ValueChange,
    );
    parameter.node_data_mut().meta.user_permissions = NodeUserPermissions {
        can_edit_name: true,
        ..NodeUserPermissions::none()
    };
    if is_reference {
        parameter.constraints.reference.target_kind = ReferenceTargetKind::ParameterOnly;
        parameter.constraints.reference.allow_projections = true;
    }
    Some(parameter)
}

/// Returns the typed multiplex list node type for one parameter type id.
pub fn user_context_multiplex_list_node_type(value_type: &str) -> String {
    format!(
        "{USER_CONTEXT_MULTIPLEX_LIST_NODE_TYPE_PREFIX}{}",
        value_type.trim().to_ascii_lowercase()
    )
}

/// Returns the parameter type id encoded in a typed multiplex list node type.
pub fn user_context_multiplex_list_value_type(node_type: &str) -> Option<&str> {
    let value_type = node_type
        .trim()
        .strip_prefix(USER_CONTEXT_MULTIPLEX_LIST_NODE_TYPE_PREFIX)?;
    PARAMETER_NODE_TYPES.contains(&value_type).then_some(value_type)
}

fn user_context_parameter_type_label(value_type: &str) -> &'static str {
    match value_type {
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
        _ => "Entry",
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

    fn inbox_requires_tree_snapshot(&self, _events: &EventFrame) -> bool {
        false
    }

    // Folders have no lifecycle hooks, so inserting one never needs a whole-tree snapshot.
    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod builtins_tests;
