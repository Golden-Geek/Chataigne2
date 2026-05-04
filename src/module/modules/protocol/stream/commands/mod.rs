use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId, UserContainerRules, UserCreatableItem, FOLDER_NODE_TYPE},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::streaming::commands::{
    bytes_request, hex_string_request, string_request, values_json_request, values_request, LINE_ENDING_NONE,
    STREAMING_SEND_BYTES_COMMAND_NODE_TYPE, STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE,
    STREAMING_SEND_STRING_COMMAND_NODE_TYPE, STREAMING_SEND_VALUES_AS_JSON_COMMAND_NODE_TYPE,
    STREAMING_SEND_VALUES_COMMAND_NODE_TYPE, StreamingSendRequest,
};

const STREAMING_COMMAND_VALUE_ITEM_KINDS: &[&str] = &[
    FOLDER_NODE_TYPE,
    "trigger",
    "int",
    "float",
    "str",
    "file",
    "enum",
    "bool",
    "css_value",
    "vec2",
    "vec3",
    "color",
    "reference",
];

macro_rules! command_node_impl {
    ($context:literal) => {
        fn child_event_interest_depth(&self, event: &Event) -> u32 {
            match event.kind {
                EventKind::ParamChanged { .. } => u32::MAX,
                _ => 0,
            }
        }

        fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
            let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
                return;
            };
            let snapshot = snapshot_arc.as_ref();
            if !crate::app::module_command::module_command_triggered(snapshot, self.id(), param) {
                return;
            }

            if let Err(error) = self.request_payload(snapshot).and_then(|payload| {
                crate::app::module_command::emit_module_command_request(
                    ctx,
                    snapshot,
                    self.id(),
                    self.get_type(),
                    &payload,
                )
            }) {
                golden_core::logerror!(format!("Failed to trigger {}: {error}", $context));
            }
        }
    };
}

#[node("streaming_command_values", label = "Values")]
pub struct StreamingCommandValues {}

#[node("streaming_command_values", from_struct)]
impl Node for StreamingCommandValues {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(STREAMING_COMMAND_VALUE_ITEM_KINDS))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(Self::NODE_TYPE, FOLDER_NODE_TYPE, "Folder")
                .with_select_when_created(false),
            UserCreatableItem::new("trigger", "trigger", "Trigger").with_select_when_created(false),
            UserCreatableItem::new("int", "int", "Int").with_select_when_created(false),
            UserCreatableItem::new("float", "float", "Float").with_select_when_created(false),
            UserCreatableItem::new("str", "str", "String").with_select_when_created(false),
            UserCreatableItem::new("file", "file", "File").with_select_when_created(false),
            UserCreatableItem::new("enum", "enum", "Enum").with_select_when_created(false),
            UserCreatableItem::new("bool", "bool", "Bool").with_select_when_created(false),
            UserCreatableItem::new("css_value", "css_value", "CSS Value").with_select_when_created(false),
            UserCreatableItem::new("vec2", "vec2", "Vec2").with_select_when_created(false),
            UserCreatableItem::new("vec3", "vec3", "Vec3").with_select_when_created(false),
            UserCreatableItem::new("color", "color", "Color").with_select_when_created(false),
            UserCreatableItem::new("reference", "reference", "Reference").with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        let normalized_node_type = node_type.trim().to_ascii_lowercase();
        if normalized_node_type == Self::NODE_TYPE || normalized_node_type == FOLDER_NODE_TYPE {
            return Some(Box::new(create_values_folder("Folder")));
        }

        let normalized = match normalized_node_type.as_str() {
            "string" => "str",
            other => other,
        };
        let default_value = default_value_for_node_type(normalized)?;
        Some(Box::new(create_value_parameter(
            value_label_for_node_type(normalized),
            default_value,
        )))
    }
}

#[node("streaming_send_string_command", label = "Send String")]
#[children(
    text: String = String::new() (
        label = "Text",
        description = "UTF-8 text to send. Escape sequences such as \\n, \\r, \\t, and \\xNN are supported.",
        widget = "textarea"
    );
    line_ending: golden_core::parameter::Enum = LINE_ENDING_NONE (
        label = "Line Ending",
        description = "Optional line ending appended to the sent text.",
        enum_options = ["None", "NL (\\n) (default)", "CR (\\r)", "CRLF (\\r\\n)"]
    );
)]
pub struct StreamingSendStringCommand {
    base: crate::app::ModuleCommandBase,
}

impl StreamingSendStringCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<StreamingSendRequest, String> {
        let text = command_string_param(snapshot, self.id(), "text").unwrap_or_default();
        let line_ending = command_enum_param(snapshot, self.id(), "line_ending")
            .unwrap_or_else(|| LINE_ENDING_NONE.to_string());
        Ok(string_request(text.as_str(), line_ending.as_str()))
    }
}

#[golden_core::item("module_command", node = "streaming_send_string_command", via = base, from_struct)]
impl Node for StreamingSendStringCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == STREAMING_SEND_STRING_COMMAND_NODE_TYPE).then(Self::create)
    }

    command_node_impl!("streaming string command");
}

#[node("streaming_send_bytes_command", label = "Send Bytes")]
#[children(
    bytes: String = String::new() (
        label = "Bytes",
        description = "Bytes to send as decimal values or 0x-prefixed hex values separated by whitespace, commas, or semicolons."
    );
)]
pub struct StreamingSendBytesCommand {
    base: crate::app::ModuleCommandBase,
}

impl StreamingSendBytesCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<StreamingSendRequest, String> {
        let bytes = command_string_param(snapshot, self.id(), "bytes").unwrap_or_default();
        bytes_request(bytes.as_str())
    }
}

#[golden_core::item("module_command", node = "streaming_send_bytes_command", via = base, from_struct)]
impl Node for StreamingSendBytesCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == STREAMING_SEND_BYTES_COMMAND_NODE_TYPE).then(Self::create)
    }

    command_node_impl!("streaming bytes command");
}

#[node("streaming_send_hex_string_command", label = "Send Hex String")]
#[children(
    hex: String = String::new() (
        label = "Hex",
        description = "Hexadecimal bytes to send. Whitespace, commas, and semicolons are ignored.",
        widget = "textarea"
    );
)]
pub struct StreamingSendHexStringCommand {
    base: crate::app::ModuleCommandBase,
}

impl StreamingSendHexStringCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<StreamingSendRequest, String> {
        let hex = command_string_param(snapshot, self.id(), "hex").unwrap_or_default();
        hex_string_request(hex.as_str())
    }
}

#[golden_core::item("module_command", node = "streaming_send_hex_string_command", via = base, from_struct)]
impl Node for StreamingSendHexStringCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE).then(Self::create)
    }

    command_node_impl!("streaming hex string command");
}

#[node("streaming_send_values_command", label = "Send Values")]
#[children(
    prefix: String = String::new() (
        label = "Prefix",
        description = "Text prepended before the formatted values. Escape sequences are supported."
    );
    suffix: String = String::new() (
        label = "Suffix",
        description = "Text appended after the formatted values. Escape sequences are supported."
    );
    separator: String = ",".to_string() (
        label = "Separator",
        description = "Text inserted between formatted values. Escape sequences are supported."
    );
    node values: StreamingCommandValues = StreamingCommandValues::new() (
        label = "Values",
        description = "Values formatted into the outgoing string in child order."
    );
)]
pub struct StreamingSendValuesCommand {
    base: crate::app::ModuleCommandBase,
}

impl StreamingSendValuesCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<StreamingSendRequest, String> {
        let prefix = command_string_param(snapshot, self.id(), "prefix").unwrap_or_default();
        let suffix = command_string_param(snapshot, self.id(), "suffix").unwrap_or_default();
        let separator = command_string_param(snapshot, self.id(), "separator").unwrap_or_else(|| ",".to_string());
        let values_id = crate::app::module_command::resolve_module_command_child(snapshot, self.id(), "values")
            .ok_or_else(|| "missing streaming command values folder 'values'".to_string())?;

        let mut values = Vec::new();
        collect_values_in_child_order(snapshot, values_id, &mut values);

        Ok(values_request(
            values.as_slice(),
            prefix.as_str(),
            suffix.as_str(),
            separator.as_str(),
        ))
    }
}

#[golden_core::item("module_command", node = "streaming_send_values_command", via = base, from_struct)]
impl Node for StreamingSendValuesCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == STREAMING_SEND_VALUES_COMMAND_NODE_TYPE).then(Self::create)
    }

    command_node_impl!("streaming values command");
}

#[node("streaming_send_values_as_json_command", label = "Send Values As JSON")]
#[children(
    node values: StreamingCommandValues = StreamingCommandValues::new() (
        label = "Values",
        description = "Values encoded as a JSON object keyed by child label. Nested folders become nested objects."
    );
)]
pub struct StreamingSendValuesAsJsonCommand {
    base: crate::app::ModuleCommandBase,
}

impl StreamingSendValuesAsJsonCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<StreamingSendRequest, String> {
        let values_id = crate::app::module_command::resolve_module_command_child(snapshot, self.id(), "values")
            .ok_or_else(|| "missing streaming command values folder 'values'".to_string())?;

        values_json_request(&encode_values_tree_json(snapshot, values_id))
    }
}

#[golden_core::item(
    "module_command",
    node = "streaming_send_values_as_json_command",
    via = base,
    from_struct
)]
impl Node for StreamingSendValuesAsJsonCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == STREAMING_SEND_VALUES_AS_JSON_COMMAND_NODE_TYPE).then(Self::create)
    }

    command_node_impl!("streaming values as json command");
}

fn command_string_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
}

fn command_enum_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_enum)
    })
}

fn create_value_parameter(label: &str, value: ParamValue) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter
}

fn create_values_folder(label: &str) -> StreamingCommandValues {
    let mut folder = StreamingCommandValues::new();
    folder.node_data_mut().meta.label = label.to_string();
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    folder
}

fn collect_values_in_child_order(snapshot: &ProcessTreeSnapshot, parent_id: NodeId, output: &mut Vec<ParamValue>) {
    for child_id in snapshot.child_ids(parent_id) {
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };

        if let Some(value) = child.param_value.clone() {
            output.push(value);
            continue;
        }

        if child.node_type == StreamingCommandValues::NODE_TYPE || child.node_type == FOLDER_NODE_TYPE {
            collect_values_in_child_order(snapshot, child_id, output);
        }
    }
}

fn encode_values_tree_json(snapshot: &ProcessTreeSnapshot, parent_id: NodeId) -> serde_json::Value {
    let mut object = serde_json::Map::new();

    for child_id in snapshot.child_ids(parent_id) {
        let Some(child) = snapshot.node(child_id) else {
            continue;
        };

        let key = child.label.trim();
        if key.is_empty() {
            continue;
        }

        if let Some(value) = child.param_value.as_ref() {
            object.insert(key.to_string(), value.to_script_json());
            continue;
        }

        if child.node_type == StreamingCommandValues::NODE_TYPE || child.node_type == FOLDER_NODE_TYPE {
            object.insert(key.to_string(), encode_values_tree_json(snapshot, child_id));
        }
    }

    serde_json::Value::Object(object)
}

fn value_label_for_node_type(node_type: &str) -> &str {
    match node_type {
        "trigger" => "Trigger",
        "int" => "Int",
        "float" => "Float",
        "str" => "String",
        "file" => "File",
        "enum" => "Enum",
        "bool" => "Bool",
        "css_value" => "CSS Value",
        "vec2" => "Vec2",
        "vec3" => "Vec3",
        "color" => "Color",
        "reference" => "Reference",
        _ => "Value",
    }
}

fn default_value_for_node_type(node_type: &str) -> Option<ParamValue> {
    match node_type {
        "trigger" => Some(ParamValue::Trigger()),
        "int" => Some(ParamValue::Int(0)),
        "float" => Some(ParamValue::Float(0.0)),
        "str" => Some(ParamValue::Str(String::new())),
        "file" => Some(ParamValue::File(String::new())),
        "enum" => Some(ParamValue::Enum(String::new())),
        "bool" => Some(ParamValue::Bool(false)),
        "css_value" => Some(ParamValue::CssValue(golden_core::parameter::CssValue::default())),
        "vec2" => Some(ParamValue::Vec2(0.0, 0.0)),
        "vec3" => Some(ParamValue::Vec3(0.0, 0.0, 0.0)),
        "color" => Some(ParamValue::Color(0.0, 0.0, 0.0, 1.0)),
        "reference" => Some(ParamValue::Reference(golden_core::node::NodeReference::default())),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
