use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId, UserContainerRules, UserCreatableItem, FOLDER_NODE_TYPE},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::ProcessCtx,
};

use crate::app::module::common::streaming::commands::{
    preparation::CommandPreparation, CommandCapture, LINE_ENDING_NONE, STREAMING_SEND_BYTES_COMMAND_NODE_TYPE,
    STREAMING_SEND_HEX_STRING_COMMAND_NODE_TYPE, STREAMING_SEND_STRING_COMMAND_NODE_TYPE,
    STREAMING_SEND_VALUES_AS_JSON_COMMAND_NODE_TYPE, STREAMING_SEND_VALUES_COMMAND_NODE_TYPE,
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
            let Some(snapshot) = ctx.tree_snapshot_arc() else {
                return;
            };
            if !crate::app::module_command::module_command_triggered(&snapshot, self.id(), param) {
                return;
            }
            if !self.node_data().effective_enabled {
                golden_core::logerror!("Disabled stream command was not admitted");
                return;
            }
            if let Err(error) = self
                .capture()
                .and_then(|capture| self.preparation.enqueue(ctx, &snapshot, self.id(), capture, None))
            {
                golden_core::logerror!(format!("Failed to trigger {}: {}", $context, error));
            }
        }
        fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
            if event.topic != crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC
                || event
                    .payload
                    .get("command_id")
                    .and_then(serde_json::Value::as_u64)
                    != Some(self.id().0)
            {
                return;
            }
            if !self.node_data().effective_enabled {
                golden_core::logerror!("Disabled stream command was not admitted");
                return;
            }
            let Some(snapshot) = ctx.tree_snapshot_arc() else {
                return;
            };
            if let Err(error) = self.capture().and_then(|capture| {
                self.preparation
                    .enqueue(ctx, &snapshot, self.id(), capture, Some(event))
            }) {
                golden_core::logerror!(format!("Failed to execute {}: {}", $context, error));
            }
        }
        fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
            self.preparation.service(ctx);
        }
        fn worker_wake_requires_tree_snapshot(&self) -> bool {
            false
        }
        fn worker_wake_when_disabled(&self) -> bool {
            true
        }
        fn destroy(&mut self, ctx: &mut ProcessCtx) {
            self.preparation.clear();
            self.base.destroy(ctx);
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
            UserCreatableItem::new(Self::NODE_TYPE, FOLDER_NODE_TYPE, "Folder").with_select_when_created(false),
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
    preparation: CommandPreparation,
}

impl StreamingSendStringCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new(), CommandPreparation::default())
    }
    fn capture(&self) -> Result<CommandCapture, String> {
        Ok(CommandCapture::String {
            text: self.text.id(),
            ending: self.line_ending.id(),
        })
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
    preparation: CommandPreparation,
}

impl StreamingSendBytesCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new(), CommandPreparation::default())
    }
    fn capture(&self) -> Result<CommandCapture, String> {
        Ok(CommandCapture::Bytes(self.bytes.id()))
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
    preparation: CommandPreparation,
}

impl StreamingSendHexStringCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new(), CommandPreparation::default())
    }
    fn capture(&self) -> Result<CommandCapture, String> {
        Ok(CommandCapture::Hex(self.hex.id()))
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
    preparation: CommandPreparation,
}

impl StreamingSendValuesCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new(), CommandPreparation::default())
    }
    fn capture(&self) -> Result<CommandCapture, String> {
        Ok(CommandCapture::Values {
            root: self
                .values
                .current_id()
                .ok_or("Stream command values folder is unavailable")?,
            prefix: self.prefix.id(),
            suffix: self.suffix.id(),
            separator: self.separator.id(),
        })
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
    preparation: CommandPreparation,
}

impl StreamingSendValuesAsJsonCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new(), CommandPreparation::default())
    }
    fn capture(&self) -> Result<CommandCapture, String> {
        Ok(CommandCapture::Json(
            self.values
                .current_id()
                .ok_or("Stream command values folder is unavailable")?,
        ))
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
