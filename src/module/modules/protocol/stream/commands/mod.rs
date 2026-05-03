use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId, UserContainerRules, UserCreatableItem},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::common::streaming::commands::{
    bytes_request, hex_string_request, string_request, values_request, LINE_ENDING_NONE,
    STREAMING_SEND_BYTES_COMMAND_NODE_TYPE, STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE,
    STREAMING_SEND_STRING_COMMAND_NODE_TYPE, STREAMING_SEND_VALUES_COMMAND_NODE_TYPE, StreamingSendRequest,
};

const STREAMING_COMMAND_VALUE_TYPES: &[&str] = &[
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
        Some(UserContainerRules::new(STREAMING_COMMAND_VALUE_TYPES))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
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

#[node("streaming_command_tester", label = "Command Tester")]
pub struct StreamingCommandTester {
    manager: crate::app::ModuleCommandManagerBase,
}

impl StreamingCommandTester {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandManagerBase::new())
    }
}

#[node("streaming_command_tester", via = manager, from_struct)]
impl Node for StreamingCommandTester {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == "streaming_command_tester").then(Self::create)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
                crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
                "Send String",
            )
            .with_select_when_created(false),
            UserCreatableItem::new(
                STREAMING_SEND_BYTES_COMMAND_NODE_TYPE,
                crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
                "Send Bytes",
            )
            .with_select_when_created(false),
            UserCreatableItem::new(
                STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE,
                crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
                "Send Hex String",
            )
            .with_select_when_created(false),
            UserCreatableItem::new(
                STREAMING_SEND_VALUES_COMMAND_NODE_TYPE,
                crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
                "Send Values",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        match node_type {
            STREAMING_SEND_STRING_COMMAND_NODE_TYPE => Some(Box::new(StreamingSendStringCommand::create())),
            STREAMING_SEND_BYTES_COMMAND_NODE_TYPE => Some(Box::new(StreamingSendBytesCommand::create())),
            STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE => Some(Box::new(StreamingSendHexStringCommand::create())),
            STREAMING_SEND_VALUES_COMMAND_NODE_TYPE => Some(Box::new(StreamingSendValuesCommand::create())),
            _ => None,
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, parent: NodeId, child: NodeId) {
        if parent == self.id() {
            self.manager.ensure_command_tester_controls(ctx, child);
        }
    }
}

#[node("streaming_send_string_command", label = "Send String")]
#[children(
    text: String = String::new() (
        label = "Text",
        description = "UTF-8 text to send. Escape sequences such as \\n, \\r, \\t, and \\xNN are supported."
    );
    line_ending: golden_core::parameter::Enum = LINE_ENDING_NONE (
        label = "Line Ending",
        description = "Optional line ending appended to the sent text.",
        enum_options = ["none (default)", "nl", "cr", "crlf"]
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
        description = "Hexadecimal bytes to send. Whitespace, commas, and semicolons are ignored."
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

        let values = snapshot
            .child_ids(values_id)
            .into_iter()
            .filter_map(|child_id| snapshot.node(child_id).and_then(|child| child.param_value.clone()))
            .collect::<Vec<_>>();

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
mod tests {
    use golden_core::app::ProjectNode;
    use golden_core::node::Node;
    use golden_core::node::NodeMeta;

    use super::{
        StreamingCommandTester, StreamingSendBytesCommand, StreamingSendHexStringCommand,
        StreamingSendStringCommand, StreamingSendValuesCommand, STREAMING_SEND_BYTES_COMMAND_NODE_TYPE,
        STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE, STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
        STREAMING_SEND_VALUES_COMMAND_NODE_TYPE,
    };

    #[test]
    fn streaming_commands_are_module_command_items() {
        let commands: Vec<Box<dyn Node>> = vec![
            Box::new(StreamingSendStringCommand::create()),
            Box::new(StreamingSendBytesCommand::create()),
            Box::new(StreamingSendHexStringCommand::create()),
            Box::new(StreamingSendValuesCommand::create()),
        ];

        for command in commands {
            assert_eq!(
                command.user_item_kind(),
                crate::app::module_command::MODULE_COMMAND_ITEM_KIND,
                "streaming command '{}' should register as a module command item",
                command.get_type()
            );
        }
    }

    #[test]
    fn command_tester_accepts_streaming_command_items() {
        let tester = StreamingCommandTester::create();
        let commands: Vec<Box<dyn Node>> = vec![
            Box::new(StreamingSendStringCommand::create()),
            Box::new(StreamingSendBytesCommand::create()),
            Box::new(StreamingSendHexStringCommand::create()),
            Box::new(StreamingSendValuesCommand::create()),
        ];

        for command in commands {
            assert!(
                tester.user_container_accepts_item(command.get_type(), command.user_item_kind()),
                "streaming command tester should accept '{}' as '{}'",
                command.get_type(),
                command.user_item_kind()
            );
        }
    }

    #[test]
    fn streaming_command_tester_decodes_from_project_node_type() {
        let node_types = [
            "streaming_command_tester",
            STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
            STREAMING_SEND_BYTES_COMMAND_NODE_TYPE,
            STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE,
            STREAMING_SEND_VALUES_COMMAND_NODE_TYPE,
        ];

        for node_type in node_types {
            let node = <crate::app::AppNode as ProjectNode>::project_decode_node(
                node_type,
                &serde_json::Value::Null,
                &NodeMeta::new("Decoded Node".to_string()),
            )
            .unwrap_or_else(|error| panic!("{node_type} should decode from project files: {error}"));

            assert_eq!(node.get_type(), node_type);
        }
    }
}
