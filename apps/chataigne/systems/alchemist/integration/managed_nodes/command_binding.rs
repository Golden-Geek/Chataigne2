use chataigne_alchemist::{StableRef, ValueComponent, ValueLaneKey, ValueTypeId};
use chataigne_state_machine::{
    OutputArgumentBinding, OutputBindingConfig, OutputSendPolicy, OutputValueSource,
};
use golden_core::{
    edit::NodeTree,
    node::{
        DeclId, Node, NodeId, NodeMetaPatch, NodeReference, NodeUserPermissions,
        UserContainerRules, UserCreatableItem,
    },
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{module_command, systems_alchemist_formula::param_to_untyped_runtime_value};

pub(crate) const MAPPING_COMMAND_BINDINGS_DECL_ID: &str = "mapping_bindings";
pub(crate) const MAPPING_COMMAND_BINDING_ITEM_KIND: &str = "mapping_command_argument_binding";
pub(crate) const MAPPING_COMMAND_BINDING_NODE_TYPE: &str = "mapping_command_argument_binding";

const INITIALIZED_TAG: &str = "chataigne.mapping.command_bindings.initialized";
const VALUE_SOURCE_DECL: &str = "value_source";
const VALUE_ELEMENT_DECL: &str = "value_element";
const VALUE_COMPONENT_DECL: &str = "value_component";
const VALUE_CONSTANT_TYPE_DECL: &str = "value_constant_type";
const VALUE_CONSTANT_BOOL_DECL: &str = "value_constant_bool";
const VALUE_CONSTANT_INT_DECL: &str = "value_constant_int";
const VALUE_CONSTANT_FLOAT_DECL: &str = "value_constant_float";
const VALUE_CONSTANT_STRING_DECL: &str = "value_constant_string";
const VALUE_CONSTANT_VEC2_DECL: &str = "value_constant_vec2";
const VALUE_CONSTANT_VEC3_DECL: &str = "value_constant_vec3";
const VALUE_CONSTANT_COLOR_DECL: &str = "value_constant_color";
const SEND_POLICY_DECL: &str = "send_policy";
const PARAMETER_DECL: &str = "parameter";
pub(crate) const UNRESOLVED_LEGACY_BINDINGS_DECL: &str = "unresolved_legacy_bindings";

const SOURCE_WHOLE: &str = "whole";
const SOURCE_ELEMENT: &str = "element";
const SOURCE_COMPONENT: &str = "component";
const SOURCE_CONSTANT: &str = "constant";
const SEND_EVERY_DELIVERY: &str = "every_delivery";
const SEND_ON_CHANGE: &str = "on_change";

#[golden_core::node("mapping_command_bindings", label = "Mapping Binding")]
#[golden_core::children(
    value_source: Enum = SOURCE_WHOLE (
        label = "Delivery Value",
        description = "Value used for delivery and change detection.",
        enum_options = ["whole", "element", "component", "constant"]
    );
    value_element: String = String::new() (
        label = "Tuple Element",
        description = "Stable tuple-element identity used by Element or Component. Leave empty for a component of the whole value."
    );
    value_component: Enum = "x" (
        label = "Component",
        enum_options = ["x", "y", "z", "r", "g", "b", "a"]
    );
    value_constant_type: Enum = "float" (
        label = "Constant Type",
        enum_options = ["bool", "int", "float", "string", "vec2", "vec3", "color"]
    );
    value_constant_bool: bool = false (label = "Boolean Constant");
    value_constant_int: i32 = 0 (label = "Integer Constant");
    value_constant_float: f64 = 0.0 (label = "Number Constant");
    value_constant_string: String = String::new() (label = "Text Constant");
    value_constant_vec2: (f64, f64) = (0.0, 0.0) (label = "2D Constant");
    value_constant_vec3: (f64, f64, f64) = (0.0, 0.0, 0.0) (label = "3D Constant");
    value_constant_color: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 1.0) (
        label = "Color Constant"
    );
    send_policy: Enum = SEND_EVERY_DELIVERY (
        label = "Send",
        enum_options = ["every_delivery", "on_change"]
    );
    unresolved_legacy_bindings: String = String::new() (
        label = "Unresolved Legacy Bindings",
        description = "Original binding document retained when migration cannot represent a legacy constant with ordinary typed controls.",
        read_only = true
    );
)]
pub struct MappingCommandBindings {
    #[state(default = None)]
    pending_config: Option<OutputBindingConfig>,
}

impl MappingCommandBindings {
    pub(crate) fn set_pending_config(&mut self, config: OutputBindingConfig) {
        self.pending_config = Some(config);
        if !self
            .node_data()
            .meta
            .tags
            .iter()
            .any(|tag| tag == INITIALIZED_TAG)
        {
            self.node_data_mut()
                .meta
                .tags
                .push(INITIALIZED_TAG.to_owned());
        }
    }

    pub(crate) fn apply_config(&mut self, ctx: &mut ProcessCtx, config: OutputBindingConfig) {
        self.set_pending_config(config);
        self.apply_pending_config(ctx);
    }

    pub(crate) fn from_config(config: OutputBindingConfig) -> Self {
        let mut bindings = Self::new();
        bindings.set_pending_config(config);
        bindings
    }

    fn apply_pending_config(&mut self, ctx: &mut ProcessCtx) {
        let Some(config) = self.pending_config.clone() else {
            return;
        };
        let Some(children) = ctx.tree_snapshot().and_then(|snapshot| {
            declared_children(
                snapshot,
                self.id(),
                &[
                    VALUE_SOURCE_DECL,
                    VALUE_ELEMENT_DECL,
                    VALUE_COMPONENT_DECL,
                    VALUE_CONSTANT_TYPE_DECL,
                    VALUE_CONSTANT_BOOL_DECL,
                    VALUE_CONSTANT_INT_DECL,
                    VALUE_CONSTANT_FLOAT_DECL,
                    VALUE_CONSTANT_STRING_DECL,
                    VALUE_CONSTANT_VEC2_DECL,
                    VALUE_CONSTANT_VEC3_DECL,
                    VALUE_CONSTANT_COLOR_DECL,
                    SEND_POLICY_DECL,
                ],
            )
        })
        else {
            return;
        };
        let authored = AuthoredValueSource::from_domain(&config.value);
        ctx.set_param(children[0], ParamValue::Enum(authored.kind.to_owned()));
        ctx.set_param(children[1], ParamValue::Str(authored.element.clone()));
        ctx.set_param(
            children[2],
            ParamValue::Enum(authored.component.to_owned()),
        );
        apply_authored_constant(ctx, &children[3..11], &authored);
        ctx.set_param(
            children[11],
            ParamValue::Enum(
                match config.send_policy {
                    OutputSendPolicy::EveryDelivery => SEND_EVERY_DELIVERY,
                    OutputSendPolicy::OnChange => SEND_ON_CHANGE,
                }
                .to_owned(),
            ),
        );
        for argument in config.arguments {
            ctx.add_child_tree(
                self.id(),
                NodeTree::new(MappingCommandArgumentBinding::from_binding(argument))
                    .as_user_item(),
                None,
            );
        }
        self.pending_config = None;
    }
}

#[golden_core::node("mapping_command_bindings", from_struct)]
impl Node for MappingCommandBindings {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            MAPPING_COMMAND_BINDING_ITEM_KIND,
        ]))
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        item_type == MAPPING_COMMAND_BINDING_NODE_TYPE
            && item_kind == MAPPING_COMMAND_BINDING_ITEM_KIND
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![UserCreatableItem::new(
            MAPPING_COMMAND_BINDING_NODE_TYPE,
            MAPPING_COMMAND_BINDING_ITEM_KIND,
            "Argument Binding",
        )]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == MAPPING_COMMAND_BINDING_NODE_TYPE)
            .then(|| Box::new(MappingCommandArgumentBinding::new()) as Box<dyn Node>)
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.apply_pending_config(ctx);
    }

    fn needs_update(&self) -> bool {
        self.pending_config.is_some()
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, _param: NodeId, _old_value: ParamValue) {
        notify_command_changed(ctx, self.id(), 1);
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_edit_name = false;
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
        self.node_data_mut().meta.can_be_disabled = false;
        self.node_data_mut().meta.decl_id =
            DeclId(MAPPING_COMMAND_BINDINGS_DECL_ID.to_owned());
    }
}

#[golden_core::node("mapping_command_argument_binding", label = "Argument Binding")]
#[golden_core::children(
    parameter: NodeReference = NodeReference::default() (
        label = "Command Parameter",
        description = "Ordinary parameter overridden for this Mapping invocation.",
        reference_target_kind = golden_core::parameter::ReferenceTargetKind::ParameterOnly
    );
    value_source: Enum = SOURCE_WHOLE (
        label = "Value",
        enum_options = ["whole", "element", "component", "constant"]
    );
    value_element: String = String::new() (
        label = "Tuple Element",
        description = "Stable tuple-element identity used by Element or Component. Leave empty for a component of the whole value."
    );
    value_component: Enum = "x" (
        label = "Component",
        enum_options = ["x", "y", "z", "r", "g", "b", "a"]
    );
    value_constant_type: Enum = "float" (
        label = "Constant Type",
        enum_options = ["bool", "int", "float", "string", "vec2", "vec3", "color"]
    );
    value_constant_bool: bool = false (label = "Boolean Constant");
    value_constant_int: i32 = 0 (label = "Integer Constant");
    value_constant_float: f64 = 0.0 (label = "Number Constant");
    value_constant_string: String = String::new() (label = "Text Constant");
    value_constant_vec2: (f64, f64) = (0.0, 0.0) (label = "2D Constant");
    value_constant_vec3: (f64, f64, f64) = (0.0, 0.0, 0.0) (label = "3D Constant");
    value_constant_color: (f64, f64, f64, f64) = (0.0, 0.0, 0.0, 1.0) (
        label = "Color Constant"
    );
)]
pub struct MappingCommandArgumentBinding {
    #[state(default = None)]
    pending_binding: Option<OutputArgumentBinding>,
}

impl MappingCommandArgumentBinding {
    pub(crate) fn apply_pending(&mut self, ctx: &mut ProcessCtx) {
        self.apply_pending_binding(ctx);
    }

    pub(crate) fn from_binding(binding: OutputArgumentBinding) -> Self {
        let mut node = Self::new();
        node.pending_binding = Some(binding);
        node
    }

    fn apply_pending_binding(&mut self, ctx: &mut ProcessCtx) {
        let Some(binding) = self.pending_binding.clone() else {
            return;
        };
        let Some(children) = ctx.tree_snapshot().and_then(|snapshot| {
            declared_children(
                snapshot,
                self.id(),
                &[
                    PARAMETER_DECL,
                    VALUE_SOURCE_DECL,
                    VALUE_ELEMENT_DECL,
                    VALUE_COMPONENT_DECL,
                    VALUE_CONSTANT_TYPE_DECL,
                    VALUE_CONSTANT_BOOL_DECL,
                    VALUE_CONSTANT_INT_DECL,
                    VALUE_CONSTANT_FLOAT_DECL,
                    VALUE_CONSTANT_STRING_DECL,
                    VALUE_CONSTANT_VEC2_DECL,
                    VALUE_CONSTANT_VEC3_DECL,
                    VALUE_CONSTANT_COLOR_DECL,
                ],
            )
        })
        else {
            return;
        };
        let Ok(uuid) = binding.parameter.stable_id.parse::<uuid::Uuid>() else {
            return;
        };
        let authored = AuthoredValueSource::from_domain(&binding.source);
        ctx.set_param(
            children[0],
            ParamValue::Reference(NodeReference::new(golden_core::node::NodeUuid(uuid))),
        );
        ctx.set_param(children[1], ParamValue::Enum(authored.kind.to_owned()));
        ctx.set_param(children[2], ParamValue::Str(authored.element.clone()));
        ctx.set_param(
            children[3],
            ParamValue::Enum(authored.component.to_owned()),
        );
        apply_authored_constant(ctx, &children[4..12], &authored);
        self.pending_binding = None;
    }
}

#[golden_core::item(
    "mapping_command_argument_binding",
    node = "mapping_command_argument_binding",
    from_struct
)]
impl Node for MappingCommandArgumentBinding {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.apply_pending_binding(ctx);
    }

    fn needs_update(&self) -> bool {
        self.pending_binding.is_some()
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, _param: NodeId, _old_value: ParamValue) {
        notify_command_changed(ctx, self.id(), 2);
    }
}

struct AuthoredValueSource {
    kind: &'static str,
    element: String,
    component: &'static str,
    constant_type: &'static str,
    constant: ParamValue,
}

impl AuthoredValueSource {
    fn from_domain(source: &OutputValueSource) -> Self {
        let mut authored = Self {
            kind: SOURCE_WHOLE,
            element: String::new(),
            component: "x",
            constant_type: "float",
            constant: ParamValue::Float(0.0),
        };
        match source {
            OutputValueSource::Whole => {}
            OutputValueSource::Element(element) => {
                authored.kind = SOURCE_ELEMENT;
                authored.element = element.as_str().to_owned();
            }
            OutputValueSource::Component { element, component } => {
                authored.kind = SOURCE_COMPONENT;
                authored.element = element
                    .as_ref()
                    .map_or_else(String::new, |element| element.as_str().to_owned());
                authored.component = component_name(*component);
            }
            OutputValueSource::Constant(value) => {
                authored.kind = SOURCE_CONSTANT;
                authored.constant = crate::app::systems_alchemist_formula::runtime_value_to_param(value)
                    .unwrap_or_else(|_| ParamValue::Str(String::new()));
                authored.constant_type = param_constant_type(&authored.constant);
            }
        }
        authored
    }
}

pub(crate) fn mapping_command_bindings_tree(config: Option<OutputBindingConfig>) -> NodeTree {
    NodeTree::new(config.map_or_else(
        MappingCommandBindings::new,
        MappingCommandBindings::from_config,
    ))
}

pub(crate) fn ensure_mapping_command_bindings(
    ctx: &mut ProcessCtx,
    command: NodeId,
) {
    let action = ctx.tree_snapshot().and_then(|snapshot| {
        snapshot
            .find_child_by_decl_id(command, MAPPING_COMMAND_BINDINGS_DECL_ID)
            .map(|bindings| default_primary_binding(snapshot, command, bindings))
    });
    if let Some(Some((bindings, tree, tags))) = action {
        ctx.add_child_tree(bindings, tree, None);
        ctx.edits.push(golden_core::edit::Edit::PatchMeta {
            node: bindings,
            patch: NodeMetaPatch {
                tags: Some(tags),
                ..NodeMetaPatch::default()
            },
        });
    }
}

fn default_primary_binding(
    snapshot: &ProcessTreeSnapshot,
    command: NodeId,
    bindings: NodeId,
) -> Option<(NodeId, NodeTree, Vec<String>)> {
    if super::is_output_container(snapshot, command) {
        return None;
    }
    let bindings_node = snapshot.node(bindings)?;
    if bindings_node.tags.iter().any(|tag| tag == INITIALIZED_TAG)
        || snapshot.child_ids(bindings).into_iter().any(|child| {
            snapshot
                .node(child)
                .is_some_and(|node| node.node_type == MAPPING_COMMAND_BINDING_NODE_TYPE)
        })
    {
        return None;
    }
    let primary = module_command::command_primary_value_parameter(snapshot, command)?;
    let parameter = stable_parameter_ref(snapshot, primary)?;
    let tree = NodeTree::new(MappingCommandArgumentBinding::from_binding(
            OutputArgumentBinding {
                parameter,
                source: OutputValueSource::Whole,
            },
        ))
        .as_user_item();
    let mut tags = bindings_node.tags.clone();
    tags.push(INITIALIZED_TAG.to_owned());
    Some((bindings, tree, tags))
}

pub(crate) fn mapping_output_binding_config(
    snapshot: &ProcessTreeSnapshot,
    command: NodeId,
) -> Result<OutputBindingConfig, String> {
    let Some(bindings) = snapshot.find_child_by_decl_id(command, MAPPING_COMMAND_BINDINGS_DECL_ID)
    else {
        return Ok(OutputBindingConfig::default());
    };
    let value = value_source_from_snapshot(snapshot, bindings)?;
    let send_policy = match child_enum(snapshot, bindings, SEND_POLICY_DECL).as_deref() {
        Some(SEND_EVERY_DELIVERY) | None => OutputSendPolicy::EveryDelivery,
        Some(SEND_ON_CHANGE) => OutputSendPolicy::OnChange,
        Some(other) => return Err(format!("unknown Mapping send policy `{other}`")),
    };
    let arguments = snapshot
        .child_ids(bindings)
        .into_iter()
        .filter(|child| {
            snapshot
                .node(*child)
                .is_some_and(|node| node.node_type == MAPPING_COMMAND_BINDING_NODE_TYPE)
        })
        .map(|binding| argument_binding_from_snapshot(snapshot, binding))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(OutputBindingConfig {
        value,
        arguments,
        send_policy,
    })
}

fn argument_binding_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    binding: NodeId,
) -> Result<OutputArgumentBinding, String> {
    let parameter = snapshot
        .find_child_by_decl_id(binding, PARAMETER_DECL)
        .and_then(|parameter| snapshot.node(parameter))
        .and_then(|parameter| parameter.param_value.as_ref())
        .and_then(|value| match value {
            ParamValue::Reference(reference) if !reference.is_empty() => {
                stable_parameter_ref_by_reference(snapshot, reference)
            }
            _ => None,
        })
        .ok_or_else(|| "a Mapping command binding has no target parameter".to_owned())?;
    Ok(OutputArgumentBinding {
        parameter,
        source: value_source_from_snapshot(snapshot, binding)?,
    })
}

fn value_source_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    owner: NodeId,
) -> Result<OutputValueSource, String> {
    match child_enum(snapshot, owner, VALUE_SOURCE_DECL).as_deref() {
        Some(SOURCE_WHOLE) | None => Ok(OutputValueSource::Whole),
        Some(SOURCE_ELEMENT) => Ok(OutputValueSource::Element(
            ValueLaneKey::new(required_child_string(snapshot, owner, VALUE_ELEMENT_DECL)?)
                .map_err(|error| error.to_string())?,
        )),
        Some(SOURCE_COMPONENT) => {
            let element = child_string(snapshot, owner, VALUE_ELEMENT_DECL)
                .filter(|value| !value.trim().is_empty())
                .map(ValueLaneKey::new)
                .transpose()
                .map_err(|error| error.to_string())?;
            let component = child_enum(snapshot, owner, VALUE_COMPONENT_DECL)
                .and_then(|value| ValueComponent::parse(value.as_str()))
                .ok_or_else(|| "a Mapping command binding has an invalid component".to_owned())?;
            Ok(OutputValueSource::Component { element, component })
        }
        Some(SOURCE_CONSTANT) => {
            let constant_type = child_enum(snapshot, owner, VALUE_CONSTANT_TYPE_DECL)
                .unwrap_or_else(|| "float".to_owned());
            let decl_id = match constant_type.as_str() {
                "bool" => VALUE_CONSTANT_BOOL_DECL,
                "int" => VALUE_CONSTANT_INT_DECL,
                "float" => VALUE_CONSTANT_FLOAT_DECL,
                "string" => VALUE_CONSTANT_STRING_DECL,
                "vec2" => VALUE_CONSTANT_VEC2_DECL,
                "vec3" => VALUE_CONSTANT_VEC3_DECL,
                "color" => VALUE_CONSTANT_COLOR_DECL,
                other => return Err(format!("unknown Mapping constant type `{other}`")),
            };
            let constant = snapshot
                .find_child_by_decl_id(owner, decl_id)
                .and_then(|parameter| snapshot.node(parameter))
                .and_then(|parameter| parameter.param_value.as_ref())
                .ok_or_else(|| "a Mapping command binding has no constant value".to_owned())?;
            Ok(OutputValueSource::Constant(
                param_to_untyped_runtime_value(constant)?,
            ))
        }
        Some(other) => Err(format!("unknown Mapping value source `{other}`")),
    }
}

fn stable_parameter_ref(
    snapshot: &ProcessTreeSnapshot,
    parameter: NodeId,
) -> Option<StableRef> {
    let node = snapshot.node(parameter)?;
    let value_type = node
        .param_value
        .as_ref()
        .and_then(|value| param_to_untyped_runtime_value(value).ok())
        .map_or_else(|| ValueTypeId::new("parameter"), |value| value.value_type());
    Some(StableRef::new(value_type, node.uuid.0.to_string()))
}

fn stable_parameter_ref_by_reference(
    snapshot: &ProcessTreeSnapshot,
    reference: &NodeReference,
) -> Option<StableRef> {
    let parameter = reference
        .cached_id()
        .filter(|parameter| snapshot.node(*parameter).is_some())
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))?;
    stable_parameter_ref(snapshot, parameter)
}

fn child_enum(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<String> {
    snapshot
        .find_child_by_decl_id(parent, decl_id)
        .and_then(|child| snapshot.node(child))
        .and_then(|child| child.param_value.as_ref())
        .and_then(ParamValue::as_enum)
}

fn child_string(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<String> {
    snapshot
        .find_child_by_decl_id(parent, decl_id)
        .and_then(|child| snapshot.node(child))
        .and_then(|child| child.param_value.as_ref())
        .and_then(ParamValue::as_str)
}

fn required_child_string(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Result<String, String> {
    child_string(snapshot, parent, decl_id)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "a Mapping command binding has no tuple-element identity".to_owned())
}

fn component_name(component: ValueComponent) -> &'static str {
    match component {
        ValueComponent::X => "x",
        ValueComponent::Y => "y",
        ValueComponent::Z => "z",
        ValueComponent::R => "r",
        ValueComponent::G => "g",
        ValueComponent::B => "b",
        ValueComponent::A => "a",
    }
}

fn declared_children(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_ids: &[&str],
) -> Option<Vec<NodeId>> {
    decl_ids
        .iter()
        .map(|decl_id| snapshot.find_child_by_decl_id(parent, decl_id))
        .collect()
}

fn apply_authored_constant(
    ctx: &mut ProcessCtx,
    children: &[NodeId],
    authored: &AuthoredValueSource,
) {
    ctx.set_param(
        children[0],
        ParamValue::Enum(authored.constant_type.to_owned()),
    );
    let value_index = match authored.constant_type {
        "bool" => 1,
        "int" => 2,
        "float" => 3,
        "string" => 4,
        "vec2" => 5,
        "vec3" => 6,
        "color" => 7,
        _ => return,
    };
    ctx.set_param(children[value_index], authored.constant.clone());
}

fn param_constant_type(value: &ParamValue) -> &'static str {
    match value {
        ParamValue::Bool(_) => "bool",
        ParamValue::Int(_) => "int",
        ParamValue::Float(_) => "float",
        ParamValue::Str(_) => "string",
        ParamValue::Vec2(_, _) => "vec2",
        ParamValue::Vec3(_, _, _) => "vec3",
        ParamValue::Color(_, _, _, _) => "color",
        _ => "string",
    }
}

fn notify_command_changed(ctx: &mut ProcessCtx, node: NodeId, parent_steps: usize) {
    let command_and_tags = ctx.tree_snapshot().and_then(|snapshot| {
        let mut command = node;
        for _ in 0..parent_steps {
            command = snapshot.node(command)?.parent?;
        }
        Some((command, snapshot.node(command)?.tags.clone()))
    });
    let Some((command, tags)) = command_and_tags else {
        return;
    };
    ctx.edits.push(golden_core::edit::Edit::PatchMeta {
        node: command,
        patch: NodeMetaPatch {
            tags: Some(tags),
            ..NodeMetaPatch::default()
        },
    });
}
