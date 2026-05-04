use golden_core::{
    node,
    node::{Node, UserContainerRules, UserCreatableItem},
    parameter::{ParamValue, Parameter, ParameterChangeCheck},
    process_ctx::ProcessCtx,
};

const OSC_COMMAND_ARGUMENT_TYPES: &[&str] = &[
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

#[node("osc_command_arguments", label = "Arguments")]
pub struct OscCommandArguments {}

#[node("osc_command_arguments", from_struct)]
impl Node for OscCommandArguments {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(OSC_COMMAND_ARGUMENT_TYPES))
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
        let default_value = default_argument_value_for_node_type(normalized)?;
        Some(Box::new(create_argument_parameter(
            argument_label_for_node_type(normalized),
            default_value,
        )))
    }
}

fn create_argument_parameter(label: &str, value: ParamValue) -> Parameter {
    let mut parameter = Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter
}

fn argument_label_for_node_type(node_type: &str) -> &str {
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
        _ => "Argument",
    }
}

fn default_argument_value_for_node_type(node_type: &str) -> Option<ParamValue> {
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
mod osc_command_arguments_tests;
