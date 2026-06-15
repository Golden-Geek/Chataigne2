use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use golden_alchemist::{
    ANodeFieldPath, ANodeId, ANodeInstance, ANodeTypeId, AlchemistFormula,
    AlchemistGraph, ColorValue, CompileCtx, DiagnosticOrigin,
    DiagnosticSeverity,
    FormulaContextContract, FormulaId, FormulaSurface, InputSocketRef,
    OutputSocketRef, ParamUiHints, RuntimeValue, SignatureCtx, StableRef,
    SurfaceItem, SurfaceItemId, SurfaceItemKind, SurfaceSection,
    SurfaceSectionId, SurfaceSource, TriggerValue, TypeBindingSource,
    TypeBindings, TypeConstraint, TypeSolveCtx, ValueTypeId, ValueTypeSpec,
    compile_graph, solve_types,
};
use golden_core::{
    color::Color,
    edit::{Edit, NodeTree},
    events::Event,
    item, node,
    node::{
        DeclId, Node, NodeCreationContext, NodeId, NodeMetaPatch,
        NodeReference, NodeUserPermissions, NodeUuid, UserContainerRules,
        UserCreatableItem,
    },
    parameter::{
        CssValue, ParamValue, Parameter, ParameterChangeCheck,
        ParameterConstraintPolicy, ParameterEnumOption, ParameterEventBehaviour,
        ReferenceTargetKind,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

pub(crate) const FORMULA_ITEM_KIND: &str = "alchemist_formula";
pub(crate) const FORMULA_FOLDER_ITEM_KIND: &str =
    "alchemist_formula_folder";
pub(crate) const FORMULA_FOLDER_NODE_TYPE: &str =
    "alchemist_formula_folder";
pub(crate) const ANODE_ITEM_KIND: &str = "alchemist_anode";
pub(crate) const CONNECTION_ITEM_KIND: &str = "alchemist_connection";
pub(crate) const ANODE_CREATE_PREFIX: &str = "alchemist_anode:";
pub(crate) const PROPERTY_ITEM_KIND: &str = "alchemist_property";
pub(crate) const PROPERTY_MANAGER_ITEM_KIND: &str =
    "alchemist_property_manager";
pub(crate) const PROPERTY_FOLDER_ITEM_KIND: &str = "alchemist_property_folder";
pub(crate) const PROPERTY_CREATE_PREFIX: &str = "alchemist_property:";
pub(crate) const PROPERTY_MANAGER_CREATE_PREFIX: &str =
    "alchemist_property_manager:";

const ANODE_NODE_TYPE: &str = "alchemist_anode";
const CONNECTION_NODE_TYPE: &str = "alchemist_connection";
pub(crate) const PROPERTY_MANAGER_NODE_TYPE: &str =
    "alchemist_property_manager";
pub(crate) const PROPERTY_FOLDER_NODE_TYPE: &str =
    "alchemist_property_folder";
pub(crate) const PROPERTY_NODE_TYPE: &str = "alchemist_property";
const PROPERTY_ANODE_TYPE: &str = "property";
pub(crate) const PROPERTIES_DECL_ID: &str = "properties";
pub(crate) const TRIGGER_INPUT_SOCKET_ID: &str = "__trigger";
const ANODE_TYPE_TAG_PREFIX: &str = "alchemist.anode.type:";
const PROPERTY_TYPE_TAG_PREFIX: &str = "alchemist.property.type:";
const FORMULA_WARNING_ID: &str = "alchemist_formula";
const ANODE_FORMULA_DIAGNOSTIC_WARNING_ID: &str =
    "alchemist_formula_diagnostic";

fn tagged_value<'a>(tags: &'a [String], prefix: &str) -> Option<&'a str> {
    tags.iter().find_map(|tag| tag.strip_prefix(prefix))
}

fn set_tag(tags: &mut Vec<String>, prefix: &str, value: &str) {
    tags.retain(|tag| !tag.starts_with(prefix));
    tags.push(format!("{prefix}{value}"));
}

fn anode_type_from_tags(tags: &[String]) -> Option<String> {
    tagged_value(tags, ANODE_TYPE_TAG_PREFIX)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn stable_hash(value: &str) -> u32 {
    let mut hash = 2_166_136_261_u32;
    for byte in value.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

fn family_hue(family: &str) -> u32 {
    match family {
        "Number" => 211,
        "Geometry" => 188,
        "String" => 42,
        "Chataigne" => 326,
        "Values" => 42,
        "Logic" => 268,
        "Flow" => 158,
        "Debug" => 14,
        _ => stable_hash(family) % 360,
    }
}

fn hsl_color(hue: f64, saturation: f64, lightness: f64) -> Color {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let hue_prime = hue / 60.0;
    let x = chroma * (1.0 - (hue_prime.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match hue_prime as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    Color::new(r1 + m, g1 + m, b1 + m, 1.0)
}

fn anode_default_color(family: &str, type_id: &str) -> Color {
    if family == "Routing"
        || type_id == chataigne_state_machine::alchemist::ROUTING_TYPE
    {
        return Color::new(0.46, 0.48, 0.5, 1.0);
    }
    let variation = stable_hash(type_id);
    let hue = (family_hue(family) + (variation % 25) + 348) % 360;
    let saturation = f64::from(62 + ((variation >> 8) % 16)) / 100.0;
    let lightness = f64::from(48 + ((variation >> 16) % 12)) / 100.0;
    hsl_color(f64::from(hue), saturation, lightness)
}

fn value_type_color(type_id: &str) -> Color {
    match type_id {
        "trigger" => Color::new(0.98, 0.42, 0.22, 1.0),
        "int" => Color::new(0.35, 0.62, 0.95, 1.0),
        "float" => Color::new(0.28, 0.74, 0.52, 1.0),
        "bool" => Color::new(0.86, 0.44, 0.78, 1.0),
        "vec2" => Color::new(0.32, 0.72, 0.92, 1.0),
        "vec3" => Color::new(0.48, 0.58, 0.94, 1.0),
        "color" => Color::new(0.98, 0.36, 0.32, 1.0),
        "value_array" => Color::new(0.51, 0.57, 0.63, 1.0),
        "reference" | "chataigne.module_endpoint" => {
            Color::new(0.64, 0.52, 0.92, 1.0)
        }
        "css_value" => Color::new(0.62, 0.68, 0.74, 1.0),
        "str" | "string" | "file" | "enum" => {
            Color::new(0.92, 0.68, 0.26, 1.0)
        }
        _ => Color::new(0.55, 0.62, 0.7, 1.0),
    }
}

fn parameter_value_color(value: &ParamValue) -> Color {
    value_type_color(parameter_node_type(value))
}

fn registry() -> golden_alchemist::ANodeRegistry {
    chataigne_state_machine::alchemist::node_registry()
}

fn value_types() -> golden_alchemist::ValueTypeRegistry {
    chataigne_state_machine::alchemist::value_type_registry()
}

fn formula_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[FORMULA_ITEM_KIND, FORMULA_FOLDER_ITEM_KIND])
}

fn formula_container_accepts(item_type: &str, item_kind: &str) -> bool {
    match item_kind {
        FORMULA_ITEM_KIND => crate::app::declared_user_item_type_matches(
            item_type,
            FORMULA_ITEM_KIND,
        ),
        FORMULA_FOLDER_ITEM_KIND => item_type == FORMULA_FOLDER_NODE_TYPE,
        _ => false,
    }
}

fn formula_container_creatable_items() -> Vec<UserCreatableItem> {
    let mut items = crate::app::declared_user_creatable_items(FORMULA_ITEM_KIND);
    items.push(UserCreatableItem::new(
        FORMULA_FOLDER_NODE_TYPE,
        FORMULA_FOLDER_ITEM_KIND,
        "Folder",
    ));
    items
}

fn create_formula_container_item(node_type: &str) -> Option<Box<dyn Node>> {
    crate::app::create_declared_user_item(node_type, FORMULA_ITEM_KIND)
        .or_else(|| {
            (node_type == FORMULA_FOLDER_NODE_TYPE).then(|| {
                Box::new(AlchemistFormulaFolder::new()) as Box<dyn Node>
            })
        })
}

fn parameter(
    label: &str,
    decl_id: impl Into<String>,
    value: ParamValue,
    read_only: bool,
) -> Parameter {
    let mut parameter =
        Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    parameter.read_only = read_only;
    parameter.control_modes_enabled = !read_only;
    parameter.node_data_mut().meta.decl_id = DeclId(decl_id.into());
    parameter
}

fn runtime_value_type_id(value: &RuntimeValue) -> String {
    value.value_type().to_string()
}

fn parameter_node_type(value: &ParamValue) -> &'static str {
    match value {
        ParamValue::Trigger() => "trigger",
        ParamValue::Int(_) => "int",
        ParamValue::Float(_) => "float",
        ParamValue::Str(_) => "str",
        ParamValue::File(_) => "file",
        ParamValue::Enum(_) => "enum",
        ParamValue::Bool(_) => "bool",
        ParamValue::CssValue(_) => "css_value",
        ParamValue::Vec2(_, _) => "vec2",
        ParamValue::Vec3(_, _, _) => "vec3",
        ParamValue::Color(_, _, _, _) => "color",
        ParamValue::Reference(_) => "reference",
    }
}

fn runtime_value_to_param(value: &RuntimeValue) -> Result<ParamValue, String> {
    Ok(match value {
        RuntimeValue::Unit => ParamValue::Str(String::new()),
        RuntimeValue::Bool(value) => ParamValue::Bool(*value),
        RuntimeValue::Trigger(_) => ParamValue::Trigger(),
        RuntimeValue::Int(value) => ParamValue::Int(
            i32::try_from(*value)
                .map_err(|_| format!("integer value {value} exceeds Golden Core i32 range"))?,
        ),
        RuntimeValue::Float(value) => ParamValue::Float(*value),
        RuntimeValue::String(value) => ParamValue::Str(value.to_string()),
        RuntimeValue::Vec2(value) => ParamValue::Vec2(value[0], value[1]),
        RuntimeValue::Vec3(value) => {
            ParamValue::Vec3(value[0], value[1], value[2])
        }
        RuntimeValue::Color(value) => ParamValue::Color(
            f64::from(value.red),
            f64::from(value.green),
            f64::from(value.blue),
            f64::from(value.alpha),
        ),
        RuntimeValue::Duration(value) => ParamValue::Float(value.as_secs_f64()),
        RuntimeValue::Array(values) => ParamValue::Str(
            values
                .iter()
                .map(|value| match value {
                    RuntimeValue::String(value) => value.to_string(),
                    RuntimeValue::Int(value) => value.to_string(),
                    RuntimeValue::Float(value) => value.to_string(),
                    RuntimeValue::Bool(value) => value.to_string(),
                    _ => format!("{value:?}"),
                })
                .collect::<Vec<_>>()
                .join(","),
        ),
        RuntimeValue::Ref(value) => {
            let uuid = value
                .stable_id
                .parse::<uuid::Uuid>()
                .map(NodeUuid)
                .unwrap_or_else(|_| NodeUuid::nil());
            ParamValue::Reference(NodeReference::new(uuid))
        }
        RuntimeValue::Extension(value) => ParamValue::Str(
            value
                .payload
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        ),
    })
}

fn default_runtime_value(
    value_type: &ValueTypeId,
) -> Result<RuntimeValue, String> {
    value_types()
        .get(value_type)
        .map(|descriptor| (descriptor.default_value)())
        .ok_or_else(|| format!("unknown Alchemist value type `{value_type}`"))
}

fn param_to_runtime_value(
    value: &ParamValue,
    value_type: &ValueTypeId,
) -> Result<RuntimeValue, String> {
    match value_type.as_str() {
        "unit" => Ok(RuntimeValue::Unit),
        "trigger" => Ok(RuntimeValue::Trigger(TriggerValue {
            fired: value.as_bool().unwrap_or(matches!(value, ParamValue::Trigger())),
            ..TriggerValue::default()
        })),
        "bool" | "int" | "float" | "string" | "vec2" | "vec3" | "color" | "duration" => {
            param_to_untyped_runtime_value(value)?.convert_to(value_type)
        }
        "value_array" => match value {
            ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => {
                Ok(RuntimeValue::Array(
                    value
                        .split(',')
                        .map(|part| RuntimeValue::String(Arc::from(part.trim())))
                        .collect(),
                ))
            }
            _ => Ok(RuntimeValue::Array(vec![param_to_untyped_runtime_value(value)?])),
        },
        _ => match value {
            ParamValue::Reference(reference) => {
                let stable_id = if reference.is_empty() {
                    String::new()
                } else {
                    reference.uuid().0.to_string()
                };
                Ok(RuntimeValue::Ref(StableRef::new(
                    value_type.clone(),
                    stable_id,
                )))
            }
            _ => Err(format!(
                "Alchemist value type `{value_type}` requires a node reference parameter"
            )),
        },
    }
}

fn param_to_untyped_runtime_value(value: &ParamValue) -> Result<RuntimeValue, String> {
    Ok(match value {
        ParamValue::Trigger() => RuntimeValue::Trigger(TriggerValue::default()),
        ParamValue::Bool(value) => RuntimeValue::Bool(*value),
        ParamValue::Int(value) => RuntimeValue::Int(i64::from(*value)),
        ParamValue::Float(value) => RuntimeValue::Float(*value),
        ParamValue::Str(value) | ParamValue::File(value) | ParamValue::Enum(value) => {
            RuntimeValue::String(Arc::from(value.as_str()))
        }
        ParamValue::CssValue(value) => RuntimeValue::Float(value.value),
        ParamValue::Vec2(x, y) => RuntimeValue::Vec2([*x, *y]),
        ParamValue::Vec3(x, y, z) => RuntimeValue::Vec3([*x, *y, *z]),
        ParamValue::Color(r, g, b, a) => RuntimeValue::Color(ColorValue {
            red: *r as f32,
            green: *g as f32,
            blue: *b as f32,
            alpha: *a as f32,
        }),
        ParamValue::Reference(reference) => {
            let stable_id = if reference.is_empty() {
                String::new()
            } else {
                reference.uuid().0.to_string()
            };
            RuntimeValue::String(Arc::from(stable_id))
        }
    })
}

fn config_decl_id(field: &str) -> String {
    format!("config/{field}")
}

fn config_type_decl_id(field: &str) -> String {
    format!("config/{field}__type")
}

fn socket_decl_id(direction: &str, socket: &str) -> String {
    format!("{direction}/{socket}")
}

fn socket_value_decl_id(direction: &str, socket: &str) -> String {
    format!("{direction}/{socket}/value")
}

fn numbered_socket_index(socket: &str, prefix: &str) -> Option<i32> {
    let index = socket.strip_prefix(prefix)?;
    index.parse::<i32>().ok().filter(|index| *index > 0)
}

fn child_param<'a>(
    snapshot: &'a ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<&'a ParamValue> {
    let child = snapshot.find_child_by_decl_id(parent, decl_id)?;
    snapshot.node(child)?.param_value.as_ref()
}

fn child_string(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<String> {
    child_param(snapshot, parent, decl_id)
        .and_then(ParamValue::as_str)
        .map(|value| value.to_string())
}

fn child_vec2(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<[f64; 2]> {
    child_param(snapshot, parent, decl_id)
        .and_then(ParamValue::as_vec2)
        .map(|value| [value.0, value.1])
}

fn enabled_child_vec2(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<[f64; 2]> {
    let child = snapshot.find_child_by_decl_id(parent, decl_id)?;
    let node = snapshot.node(child)?;
    node.enabled.then_some(())?;
    node.param_value
        .as_ref()
        .and_then(ParamValue::as_vec2)
        .map(|value| [value.0, value.1])
}

fn child_bool(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<bool> {
    child_param(snapshot, parent, decl_id).and_then(ParamValue::as_bool)
}

fn direct_child_under(
    snapshot: &ProcessTreeSnapshot,
    ancestor: NodeId,
    descendant: NodeId,
) -> Option<NodeId> {
    let mut child = descendant;
    let mut parent = snapshot.node(child)?.parent?;
    while parent != ancestor {
        child = parent;
        parent = snapshot.node(child)?.parent?;
    }
    Some(child)
}

fn node_removal_pending(ctx: &ProcessCtx, node: NodeId) -> bool {
    ctx.edits.pending.iter().any(|request| {
        matches!(&request.edit, Edit::RemoveNode { node: pending } if *pending == node)
    })
}

fn replace_child_tree_once(
    ctx: &mut ProcessCtx,
    parent: NodeId,
    existing: Option<NodeId>,
    tree: NodeTree,
) {
    if let Some(existing) = existing {
        if node_removal_pending(ctx, existing) {
            return;
        }
        ctx.edits.push(Edit::RemoveNode { node: existing });
    }
    ctx.add_child_tree(parent, tree, None);
}

fn is_anode_type_variable_config_param(
    snapshot: &ProcessTreeSnapshot,
    anode: NodeId,
    param: NodeId,
) -> bool {
    let Some(anode_snapshot) = snapshot.node(anode) else {
        return false;
    };
    let Some(type_id) = anode_type_from_tags(&anode_snapshot.tags)
        .or_else(|| child_string(snapshot, anode, "anode_type"))
    else {
        return false;
    };
    let Some(config_folder) = snapshot.find_child_by_decl_id(anode, "config") else {
        return false;
    };
    let registry = registry();
    let Some(declaration) = registry.get(&ANodeTypeId::new(type_id)) else {
        return false;
    };
    declaration
        .config_fields()
        .into_iter()
        .filter(|field| field.type_variable.is_some())
        .any(|field| {
            snapshot.find_child_by_decl_id(config_folder, &config_decl_id(field.id.as_str()))
                == Some(param)
        })
}

fn child_reference_uuid(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeUuid> {
    child_param(snapshot, parent, decl_id).and_then(|value| match value {
        ParamValue::Reference(reference) => Some(reference.uuid()),
        _ => None,
    })
}

fn constraint_value_type(
    constraint: &TypeConstraint,
    bindings: &TypeBindings,
) -> ValueTypeId {
    match constraint {
        TypeConstraint::Exact(value_type) => value_type.clone(),
        TypeConstraint::Generic(variable) => bindings
            .get(variable)
            .map(|binding| binding.value_type.clone())
            .unwrap_or_else(|| ValueTypeId::new("float")),
        TypeConstraint::Facet(_)
        | TypeConstraint::Any
        | TypeConstraint::Primitive
        | TypeConstraint::NumericLike
        | TypeConstraint::OneOf(_) => ValueTypeId::new("float"),
    }
}

fn log_config_field() -> golden_alchemist::ANodeConfigFieldDecl {
    golden_alchemist::ANodeConfigFieldDecl::new(
        "log",
        "Log",
        RuntimeValue::Bool(false),
    )
    .with_description("Emit a debug log entry whenever this node processes successfully.")
}

fn config_fields_for_instance(
    declaration: &dyn golden_alchemist::ANodeDeclaration,
    instance: &ANodeInstance,
) -> Vec<golden_alchemist::ANodeConfigFieldDecl> {
    let mut fields = declaration.config_fields_for(instance);
    if !fields.iter().any(|field| field.id.as_str() == "log") {
        fields.push(log_config_field());
    }
    fields
}

fn existing_or_default_config(
    snapshot: &ProcessTreeSnapshot,
    anode: NodeId,
    declaration: &dyn golden_alchemist::ANodeDeclaration,
) -> Result<golden_alchemist::ANodeConfig, String> {
    let config_folder = snapshot
        .find_child_by_decl_id(anode, "config")
        .ok_or_else(|| "ANode Config folder is missing".to_string())?;
    let mut config = golden_alchemist::ANodeConfig::default();
    for field in declaration.config_fields() {
        let value = config_field_value(snapshot, config_folder, &field);
        config.set(field.id, value);
    }
    let mut instance =
        ANodeInstance::new(declaration.type_id(), declaration.label());
    instance.config = config.clone();
    for field in config_fields_for_instance(declaration, &instance) {
        let value = config_field_value(snapshot, config_folder, &field);
        config.set(field.id, value);
    }
    Ok(config)
}

fn config_field_value(
    snapshot: &ProcessTreeSnapshot,
    config_folder: NodeId,
    field: &golden_alchemist::ANodeConfigFieldDecl,
) -> RuntimeValue {
    let value_type = if field.editor.as_deref() == Some("runtime_value") {
        child_string(
            snapshot,
            config_folder,
            &config_type_decl_id(field.id.as_str()),
        )
        .map(ValueTypeId::new)
        .unwrap_or_else(|| field.default_value.value_type())
    } else {
        field.default_value.value_type()
    };
    child_param(
        snapshot,
        config_folder,
        &config_decl_id(field.id.as_str()),
    )
    .and_then(|value| param_to_runtime_value(value, &value_type).ok())
    .unwrap_or_else(|| {
        if value_type == field.default_value.value_type() {
            field.default_value.clone()
        } else {
            default_runtime_value(&value_type)
                .unwrap_or_else(|_| field.default_value.clone())
        }
    })
}

fn apply_forced_type_bindings_from_config(
    snapshot: &ProcessTreeSnapshot,
    config_folder: NodeId,
    declaration: &dyn golden_alchemist::ANodeDeclaration,
    signature: &golden_alchemist::ANodeSignature,
    value_types: &golden_alchemist::ValueTypeRegistry,
    instance: &mut ANodeInstance,
) {
    for field in config_fields_for_instance(declaration, instance) {
        let Some(variable) = field.type_variable.clone() else {
            continue;
        };
        let decl_id = config_decl_id(field.id.as_str());
        let Some(parameter_node_id) = snapshot.find_child_by_decl_id(config_folder, &decl_id) else {
            continue;
        };
        let Some(parameter_node) = snapshot.node(parameter_node_id) else {
            continue;
        };
        if !parameter_node.enabled {
            continue;
        }
        let Some(selected_type) = parameter_node
            .param_value
            .as_ref()
            .and_then(ParamValue::as_str)
        else {
            continue;
        };
        let selected_type = ValueTypeId::new(selected_type);
        let type_options = field.resolved_type_options(signature, value_types);
        if !type_options.contains(&selected_type) {
            continue;
        }
        instance.forced_type_bindings.insert(
            variable,
            selected_type,
            TypeBindingSource::ForcedByUser,
        );
    }
}

fn local_signature_bindings(
    signature: &golden_alchemist::ANodeSignature,
    instance: &ANodeInstance,
) -> TypeBindings {
    let mut bindings = signature.default_bindings.clone();
    let _ = bindings.merge_from(&instance.type_bindings);
    let _ = bindings.merge_from(&instance.forced_type_bindings);
    bindings
}

fn sync_auto_type_variable_config_params(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    config_folder: NodeId,
    declaration: &dyn golden_alchemist::ANodeDeclaration,
    signature: &golden_alchemist::ANodeSignature,
    value_types: &golden_alchemist::ValueTypeRegistry,
    bindings: &TypeBindings,
) {
    for field in declaration.config_fields() {
        let Some(variable) = field.type_variable.clone() else {
            continue;
        };
        let Some(binding) = bindings.get(&variable) else {
            continue;
        };
        let type_options = field.resolved_type_options(signature, value_types);
        if !type_options.is_empty() && !type_options.contains(&binding.value_type) {
            continue;
        }
        let decl_id = config_decl_id(field.id.as_str());
        let Some(parameter_node_id) =
            snapshot.find_child_by_decl_id(config_folder, &decl_id)
        else {
            continue;
        };
        let Some(parameter_node) = snapshot.node(parameter_node_id) else {
            continue;
        };
        if parameter_node.enabled {
            continue;
        }
        let next_value = ParamValue::Enum(binding.value_type.to_string());
        if parameter_node.param_value.as_ref() == Some(&next_value) {
            continue;
        }
        ctx.edits.push(Edit::SetParam {
            node: parameter_node_id,
            value: next_value,
            behaviour: ParameterEventBehaviour::Coalesce,
        });
    }
}

fn sync_auto_input_count(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    anode: NodeId,
    config_folder: NodeId,
    type_id: &str,
) {
    let socket_prefix = match type_id {
        "math" => "value",
        "concatenate" => "part",
        _ => return,
    };
    let Some(count_param) =
        snapshot.find_child_by_decl_id(config_folder, "config/num_inputs")
    else {
        return;
    };
    let Some(count_node) = snapshot.node(count_param) else {
        return;
    };
    if count_node.enabled {
        return;
    }
    let Some(anode_snapshot) = snapshot.node(anode) else {
        return;
    };
    let Some(formula) = anode_snapshot.parent else {
        return;
    };
    let target_uuid = anode_snapshot.uuid;
    let mut connected_indices = HashSet::<i32>::new();
    for child in snapshot.child_ids(formula) {
        let Some(connection) = snapshot.node(child) else {
            continue;
        };
        if connection.node_type != CONNECTION_NODE_TYPE {
            continue;
        }
        if child_reference_uuid(snapshot, child, "target_node")
            != Some(target_uuid)
        {
            continue;
        }
        let Some(socket) = child_string(snapshot, child, "target_socket")
        else {
            continue;
        };
        if let Some(index) = numbered_socket_index(&socket, socket_prefix) {
            connected_indices.insert(index);
        }
    }
    let current = match count_node.param_value.as_ref() {
        Some(ParamValue::Int(value)) => *value,
        _ => 2,
    }
    .clamp(2, 64);
    let all_current_inputs_connected =
        (1..=current).all(|index| connected_indices.contains(&index));
    let highest_connected = connected_indices.iter().copied().max().unwrap_or(0);
    let desired = if all_current_inputs_connected {
        (current + 1).clamp(2, 64)
    } else if highest_connected < current {
        (highest_connected + 1).clamp(2, current)
    } else {
        current
    };
    if count_node.param_value.as_ref() == Some(&ParamValue::Int(desired)) {
        return;
    }
    if ctx.edits.pending.iter().any(|request| {
        matches!(
            &request.edit,
            Edit::SetParam { node, value, .. }
                if *node == count_param && *value == ParamValue::Int(desired)
        )
    }) {
        return;
    }
    ctx.edits.push(Edit::SetParam {
        node: count_param,
        value: ParamValue::Int(desired),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
}

fn anode_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    anode: NodeId,
) -> Result<ANodeInstance, String> {
    let node = snapshot
        .node(anode)
        .ok_or_else(|| format!("ANode {anode:?} is missing"))?;
    let type_id = anode_type_from_tags(&node.tags)
        .or_else(|| child_string(snapshot, anode, "anode_type"))
        .ok_or_else(|| format!("ANode `{}` has no internal type", node.label))?;
    let registry = registry();
    let declaration = registry
        .get(&ANodeTypeId::new(type_id.clone()))
        .ok_or_else(|| format!("ANode type `{type_id}` is not registered"))?;
    let mut instance =
        ANodeInstance::new(ANodeTypeId::new(type_id), node.label.clone());
    instance.id = ANodeId::from_uuid(node.uuid.0);
    instance.enabled = node.enabled;
    let config_folder = snapshot
        .find_child_by_decl_id(anode, "config")
        .ok_or_else(|| "ANode Config folder is missing".to_string())?;
    instance.config =
        existing_or_default_config(snapshot, anode, declaration.as_ref())?;
    let value_types = value_types();
    let signature_ctx = SignatureCtx {
        value_types: &value_types,
    };
    let signature = declaration.signature(
        &signature_ctx,
        &instance,
        &instance.type_bindings,
    );
    apply_forced_type_bindings_from_config(
        snapshot,
        config_folder,
        declaration.as_ref(),
        &signature,
        &value_types,
        &mut instance,
    );
    instance.ui.position =
        child_vec2(snapshot, anode, "position").unwrap_or([0.0, 0.0]);
    instance.ui.size = enabled_child_vec2(snapshot, anode, "size")
        .filter(|size| size[0] > 0.0 && size[1] > 0.0);
    instance.ui.collapsed = node.presentation.collapsed;

    let signature = declaration.signature(
        &signature_ctx,
        &instance,
        &instance.type_bindings,
    );
    let signature_bindings = local_signature_bindings(&signature, &instance);
    if let Some(inputs_folder) = snapshot.find_child_by_decl_id(anode, "inputs") {
        for input in signature.inputs {
            let Some(socket_node) = snapshot.find_child_by_decl_id(
                inputs_folder,
                &socket_decl_id("inputs", input.id.as_str()),
            ) else {
                continue;
            };
            let value_type =
                constraint_value_type(&input.constraint, &signature_bindings);
            let value = child_param(
                snapshot,
                socket_node,
                &socket_value_decl_id("inputs", input.id.as_str()),
            )
            .map(|value| param_to_runtime_value(value, &value_type))
            .transpose()?
            .or(input.default_value);
            if let Some(value) = value {
                instance.input_defaults.insert(input.id, value);
            }
        }
    }
    Ok(instance)
}

pub(crate) fn formula_from_snapshot(
	snapshot: &ProcessTreeSnapshot,
	formula_node: NodeId,
) -> Result<AlchemistFormula, String> {
    let formula_snapshot = snapshot
        .node(formula_node)
        .ok_or_else(|| format!("Formula {formula_node:?} is missing"))?;
    if formula_snapshot.node_type != AlchemistFormulaDefinition::NODE_TYPE {
        return Err(format!(
            "node `{}` is not an Alchemist Formula",
            formula_snapshot.label
        ));
    }

    let mut graph = AlchemistGraph::new();
    graph.id = golden_alchemist::AlchemistGraphId::from_uuid(
        formula_snapshot.uuid.0,
    );
    graph.metadata.label = formula_snapshot.label.clone();
    let mut anodes_by_uuid = HashMap::<NodeUuid, ANodeId>::new();

    for child in snapshot.child_ids(formula_node) {
        let Some(child_snapshot) = snapshot.node(child) else {
            continue;
        };
        if child_snapshot.node_type != ANODE_NODE_TYPE {
            continue;
        }
        let instance = anode_from_snapshot(snapshot, child)?;
        anodes_by_uuid.insert(child_snapshot.uuid, instance.id);
        graph
            .add_node(instance)
            .map_err(|error| error.to_string())?;
    }

    for child in snapshot.child_ids(formula_node) {
        let Some(child_snapshot) = snapshot.node(child) else {
            continue;
        };
        if child_snapshot.node_type != CONNECTION_NODE_TYPE {
            continue;
        }
        let source = child_reference_uuid(snapshot, child, "source_node")
            .and_then(|uuid| anodes_by_uuid.get(&uuid).copied())
            .ok_or_else(|| {
                format!("connection `{}` has no valid source ANode", child_snapshot.label)
            })?;
        let target = child_reference_uuid(snapshot, child, "target_node")
            .and_then(|uuid| anodes_by_uuid.get(&uuid).copied())
            .ok_or_else(|| {
                format!("connection `{}` has no valid target ANode", child_snapshot.label)
            })?;
        let source_socket = child_string(snapshot, child, "source_socket")
            .ok_or_else(|| "connection source socket is missing".to_string())?;
        let target_socket = child_string(snapshot, child, "target_socket")
            .ok_or_else(|| "connection target socket is missing".to_string())?;
        graph
            .connect(
                OutputSocketRef::new(source, source_socket),
                InputSocketRef::new(target, target_socket),
            )
            .map_err(|error| error.to_string())?;
    }

    let surface = formula_surface_from_snapshot(
        snapshot,
        formula_node,
        &mut graph,
    )?;

    Ok(AlchemistFormula {
        id: FormulaId::new(formula_snapshot.uuid.0.to_string()),
        version: 1,
        label: formula_snapshot.label.clone(),
        description: None,
        tags: formula_snapshot.tags.clone(),
        graph,
        surface,
        context_contract: FormulaContextContract {
            accepts_additional_dimensions: true,
            ..FormulaContextContract::default()
        },
        migrations: Vec::new(),
	})
}

fn formula_anode_node_ids(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
) -> HashMap<ANodeId, NodeId> {
    snapshot
        .child_ids(formula_node)
        .into_iter()
        .filter_map(|child| {
            let node = snapshot.node(child)?;
            (node.node_type == ANODE_NODE_TYPE)
                .then(|| (ANodeId::from_uuid(node.uuid.0), child))
        })
        .collect()
}

fn diagnostic_origin_node_id(
    origin: &DiagnosticOrigin,
    anode_node_ids: &HashMap<ANodeId, NodeId>,
) -> Option<NodeId> {
    match origin {
        DiagnosticOrigin::Node(node)
        | DiagnosticOrigin::Socket { node, .. } => {
            anode_node_ids.get(node).copied()
        }
        DiagnosticOrigin::Graph
        | DiagnosticOrigin::Registry
        | DiagnosticOrigin::Runtime => None,
    }
}

fn node_has_warning(
    snapshot: &ProcessTreeSnapshot,
    node: NodeId,
    warning_id: &str,
) -> bool {
    snapshot.node(node).is_some_and(|node| {
        node.presentation
            .warnings
            .iter()
            .any(|warning| warning.id == warning_id)
    })
}

fn node_warning_matches(
    snapshot: &ProcessTreeSnapshot,
    node: NodeId,
    warning_id: &str,
    message: &str,
    detail: Option<&str>,
) -> bool {
    snapshot.node(node).is_some_and(|node| {
        node.presentation
            .warnings
            .iter()
            .find(|warning| warning.id == warning_id)
            .is_some_and(|warning| {
                warning.message == message
                    && warning.detail.as_deref() == detail
            })
    })
}

#[node("alchemist_input_socket", label = "Input")]
pub struct AlchemistInputSocket {}

#[node("alchemist_input_socket", from_struct)]
impl Node for AlchemistInputSocket {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("alchemist_output_socket", label = "Output")]
pub struct AlchemistOutputSocket {}

#[node("alchemist_output_socket", from_struct)]
impl Node for AlchemistOutputSocket {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("alchemist_anode", label = "ANode")]
#[children(
    position: golden_core::parameter::Vec2 = (0.0, 0.0) (
        label = "Position",
        show_in_inspector_content = false
    );
    size: golden_core::parameter::Vec2 = (13.0, 8.0) (
        label = "Size",
        enabled = false,
        can_be_disabled = true,
        show_in_inspector_content = false
    );
    trigger_input_enabled: bool = false (label = "Trigger Input");
    folder(config, label = "Config") {}
    folder(inputs, label = "Inputs") {}
    folder(outputs, label = "Outputs") {}
)]
pub struct AlchemistANode {}

#[node("alchemist_anode", from_struct)]
impl Node for AlchemistANode {
    fn user_item_kind(&self) -> &str {
        ANODE_ITEM_KIND
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        self.node_data_mut().meta.can_be_disabled = true;
        self.reconcile_structure(ctx);
    }

    fn on_node_ready(
        &mut self,
        ctx: &mut ProcessCtx,
        context: NodeCreationContext,
    ) {
        self.reconcile_structure(ctx);
        if context == NodeCreationContext::Duplicate {
            let Some(snapshot) = ctx.tree_snapshot_arc() else {
                return;
            };
            if let Some([x, y]) = child_vec2(&snapshot, self.id(), "position") {
                ctx.set_param(
                    self.position.id(),
                    ParamValue::Vec2(x + 1.5, y + 1.5),
                );
            }
        }
    }

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        if param == self.trigger_input_enabled.id() {
            self.reconcile_structure(ctx);
            return;
        }

        let should_reconcile = ctx.tree_snapshot().is_some_and(|snapshot| {
            let Some(config_folder) =
                snapshot.find_child_by_decl_id(self.id(), "config")
            else {
                return false;
            };
            snapshot
                .node(param)
                .is_some_and(|node| node.parent == Some(config_folder))
                && !is_anode_type_variable_config_param(
                    snapshot,
                    self.id(),
                    param,
                )
        });
        if should_reconcile {
            self.reconcile_structure(ctx);
        }
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        4
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

impl AlchemistANode {
    fn for_type(type_id: &str, label: &str, category: &str) -> Self {
        let mut node = Self::new();
        let meta = &mut node.node_data_mut().meta;
        set_tag(&mut meta.tags, ANODE_TYPE_TAG_PREFIX, type_id);
        meta.label = label.to_owned();
        meta.presentation.color = Some(anode_default_color(category, type_id));
        if type_id == chataigne_state_machine::alchemist::ROUTING_TYPE {
            meta.presentation.collapsed = true;
        }
        node
    }

    fn reconcile_structure(&mut self, ctx: &mut ProcessCtx) {
        let tagged_type = anode_type_from_tags(&self.node_data().meta.tags);
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let legacy_type = child_string(&snapshot, self.id(), "anode_type");
        let Some(type_id) = tagged_type.or(legacy_type) else {
            return;
        };
        if anode_type_from_tags(&self.node_data().meta.tags).is_none() {
            set_tag(
                &mut self.node_data_mut().meta.tags,
                ANODE_TYPE_TAG_PREFIX,
                &type_id,
            );
        }
        if let Some(legacy_type_child) =
            snapshot.find_child_by_decl_id(self.id(), "anode_type")
        {
            self.remove_child(ctx, legacy_type_child);
        }
        let property_getter = type_id == PROPERTY_ANODE_TYPE;
        let manager_ref = matches!(
            type_id.as_str(),
            chataigne_state_machine::alchemist::CONDITIONS_MANAGER_TYPE
                | chataigne_state_machine::alchemist::INPUTS_MANAGER_TYPE
                | chataigne_state_machine::alchemist::OUTPUTS_MANAGER_TYPE
        );
        let mut permissions = NodeUserPermissions::all();
        permissions.can_edit_name = !(property_getter || manager_ref);
        permissions.can_edit_color = !property_getter;
        self.node_data_mut().meta.user_permissions = permissions;
        self.node_data_mut().meta.can_be_disabled = true;
        let registry = registry();
        let Some(declaration) = registry.get(&ANodeTypeId::new(&type_id)) else {
            ctx.set_node_warning_with(
                self.id(),
                Some("alchemist_anode"),
                "Unknown ANode type",
                Some(&type_id),
            );
            return;
        };
        ctx.clear_node_warning(self.id(), Some("alchemist_anode"));
        if self.node_data().meta.presentation.color.is_none() {
            self.node_data_mut().meta.presentation.color = Some(
                anode_default_color(declaration.category(), &type_id),
            );
        }
        let Some(config_folder) =
            snapshot.find_child_by_decl_id(self.id(), "config")
        else {
            return;
        };
        let Some(inputs_folder) =
            snapshot.find_child_by_decl_id(self.id(), "inputs")
        else {
            return;
        };
        let Some(outputs_folder) =
            snapshot.find_child_by_decl_id(self.id(), "outputs")
        else {
            return;
        };
        sync_auto_input_count(ctx, &snapshot, self.id(), config_folder, &type_id);

        let value_types = value_types();
        let signature_ctx = SignatureCtx {
            value_types: &value_types,
        };
        let mut instance =
            ANodeInstance::new(declaration.type_id(), declaration.label());
        instance.config = existing_or_default_config(
            &snapshot,
            self.id(),
            declaration.as_ref(),
        )
        .unwrap_or_default();
        let config_signature = declaration.signature(
            &signature_ctx,
            &instance,
            &instance.type_bindings,
        );

        let mut desired_config = HashSet::new();
        for field in config_fields_for_instance(declaration.as_ref(), &instance) {
            let value_decl = config_decl_id(field.id.as_str());
            desired_config.insert(value_decl.clone());
            if field.type_variable.is_some() {
                let type_options =
                    field.resolved_type_options(&config_signature, &value_types);
                let selected_type = child_string(&snapshot, config_folder, value_decl.as_str())
                    .unwrap_or_else(|| match &field.default_value {
                        RuntimeValue::String(value) => value.to_string(),
                        value => runtime_value_type_id(value),
                    });
                self.ensure_parameter_node(
                    ctx,
                    &snapshot,
                    config_folder,
                    value_type_parameter(
                        &field.label,
                        &value_decl,
                        &selected_type,
                        property_getter,
                        &type_options,
                    ),
                );
                continue;
            }
            let value_type = if field.editor.as_deref() == Some("runtime_value") {
                let type_decl = config_type_decl_id(field.id.as_str());
                desired_config.insert(type_decl.clone());
                let type_options =
                    field.resolved_type_options(&config_signature, &value_types);
                let selected_type = child_string(
                    &snapshot,
                    config_folder,
                    type_decl.as_str(),
                )
                .unwrap_or_else(|| runtime_value_type_id(&field.default_value));
                let mut type_parameter = value_type_parameter(
                    &format!("{} Type", field.label),
                    &type_decl,
                    &selected_type,
                    property_getter,
                    &type_options,
                );
                if !property_getter {
                    // A runtime value's type is explicit (there are no inputs to
                    // infer it from), so it is shown in the header and stays
                    // always-on rather than being a disable-to-infer selector.
                    let meta = &mut type_parameter.node_data_mut().meta;
                    meta.can_be_disabled = false;
                    meta.enabled = true;
                }
                self.ensure_parameter_node(
                    ctx,
                    &snapshot,
                    config_folder,
                    type_parameter,
                );
                ValueTypeId::new(selected_type)
            } else {
                field.default_value.value_type()
            };
            let default = if value_type == field.default_value.value_type() {
                field.default_value.clone()
            } else {
                default_runtime_value(&value_type)
                    .unwrap_or_else(|_| field.default_value.clone())
            };
            if let Ok(value) = runtime_value_to_param(&default) {
                let mut config_parameter = parameter(
                    &field.label,
                    &value_decl,
                    value,
                    property_getter,
                );
                if !field.enum_options.is_empty() {
                    let selected = child_string(
                        &snapshot,
                        config_folder,
                        value_decl.as_str(),
                    )
                    .or_else(|| match &field.default_value {
                        RuntimeValue::String(value) => Some(value.to_string()),
                        _ => None,
                    })
                    .unwrap_or_default();
                    config_parameter.value = ParamValue::Enum(selected);
                    config_parameter.default_value =
                        ParamValue::Enum(match &field.default_value {
                            RuntimeValue::String(value) => value.to_string(),
                            _ => String::new(),
                        });
                    config_parameter.constraints.enum_options = field
                        .enum_options
                        .iter()
                        .map(|(variant_id, label)| ParameterEnumOption {
                            variant_id: variant_id.to_string(),
                            value: ParamValue::Enum(variant_id.to_string()),
                            label: label.clone(),
                            tags: Vec::new(),
                            ordering: None,
                        })
                        .collect();
                    config_parameter.constraints.policy =
                        ParameterConstraintPolicy::Reject;
                }
                if field.editor.as_deref() == Some("optional_count") {
                    config_parameter.node_data_mut().meta.can_be_disabled =
                        true;
                    config_parameter.node_data_mut().meta.enabled = false;
                }
                config_parameter
                    .node_data_mut()
                    .meta
                    .presentation
                    .color = Some(value_type_color(value_type.as_str()));
                if property_getter {
                    config_parameter
                        .node_data_mut()
                        .meta
                        .presentation
                        .show_in_inspector_content = false;
                }
                self.ensure_parameter_node(
                    ctx,
                    &snapshot,
                    config_folder,
                    config_parameter,
                );
            }
        }
        self.remove_obsolete_children(
            ctx,
            &snapshot,
            config_folder,
            &desired_config,
        );

        apply_forced_type_bindings_from_config(
            &snapshot,
            config_folder,
            declaration.as_ref(),
            &config_signature,
            &value_types,
            &mut instance,
        );
        let signature = declaration.signature(
            &signature_ctx,
            &instance,
            &instance.type_bindings,
        );
        let signature_bindings = local_signature_bindings(&signature, &instance);

        let mut desired_inputs = HashSet::new();
        for input in signature.inputs {
            let decl_id = socket_decl_id("inputs", input.id.as_str());
            desired_inputs.insert(decl_id.clone());
            let value_type =
                constraint_value_type(&input.constraint, &signature_bindings);
            let default = input
                .default_value
                .or_else(|| default_runtime_value(&value_type).ok())
                .unwrap_or(RuntimeValue::Float(0.0));
            let existing =
                snapshot.find_child_by_decl_id(inputs_folder, &decl_id);
            let needs_rebuild = existing.is_none_or(|socket| {
                !input_socket_matches(
                    &snapshot,
                    socket,
                    input.id.as_str(),
                    &value_type,
                    &default,
                )
            });
            if needs_rebuild {
                if let Ok(tree) = input_socket_tree(
                    input.id.as_str(),
                    &input.label,
                    &value_type,
                    &default,
                ) {
                    replace_child_tree_once(ctx, inputs_folder, existing, tree);
                }
            }
        }
        let trigger_input = child_bool(&snapshot, self.id(), "trigger_input_enabled").unwrap_or(false);
        let trigger_decl = socket_decl_id("inputs", TRIGGER_INPUT_SOCKET_ID);
        if trigger_input {
            desired_inputs.insert(trigger_decl.clone());
            if snapshot.find_child_by_decl_id(inputs_folder, &trigger_decl).is_none() {
                if let Ok(tree) = input_socket_tree(
                    TRIGGER_INPUT_SOCKET_ID,
                    "Trigger",
                    &ValueTypeId::new("trigger"),
                    &golden_alchemist::RuntimeValue::Trigger(golden_alchemist::TriggerValue::default()),
                ) {
                    ctx.add_child_tree(inputs_folder, tree, None);
                }
            }
        }

        self.remove_obsolete_children(
            ctx,
            &snapshot,
            inputs_folder,
            &desired_inputs,
        );

        let mut desired_outputs = HashSet::new();
        for output in signature.outputs {
            let decl_id = socket_decl_id("outputs", output.id.as_str());
            desired_outputs.insert(decl_id.clone());
            let value_type =
                constraint_value_type(&output.constraint, &signature_bindings);
            let existing =
                snapshot.find_child_by_decl_id(outputs_folder, &decl_id);
            if existing.is_none_or(|socket| {
                !output_socket_matches(
                    &snapshot,
                    socket,
                    output.id.as_str(),
                    &value_type,
                )
            }) {
                replace_child_tree_once(
                    ctx,
                    outputs_folder,
                    existing,
                    output_socket_tree(
                        output.id.as_str(),
                        &output.label,
                        &value_type,
                    ),
                );
            }
        }
        self.remove_obsolete_children(
            ctx,
            &snapshot,
            outputs_folder,
            &desired_outputs,
        );
    }

    fn ensure_parameter_node(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent: NodeId,
        parameter: Parameter,
    ) {
        let decl_id = parameter.node_data().meta.decl_id.clone();
        if let Some(existing) =
            snapshot.find_child_by_decl_id(parent, decl_id.0.as_str())
        {
            let Some(existing_snapshot) = snapshot.node(existing) else {
                return;
            };
            let expected_type = parameter.get_type();
            if existing_snapshot.node_type != expected_type {
                ctx.replace_node(existing, parameter);
            } else if parameter.read_only {
                let presentation =
                    parameter.node_data().meta.presentation.clone();
                ctx.call_node_mutation(existing, move |node, _ctx| {
                    let Some(existing) =
                        node.as_any_mut().downcast_mut::<Parameter>()
                    else {
                        return Err(
                            "expected an Alchemist config parameter".into(),
                        );
                    };
                    existing.read_only = true;
                    existing.control_modes_enabled = false;
                    existing.node_data_mut().meta.presentation = presentation;
                    Ok(())
                });
            } else {
                let can_be_disabled =
                    parameter.node_data().meta.can_be_disabled;
                let presentation =
                    parameter.node_data().meta.presentation.clone();
                let constraints = parameter.constraints.clone();
                let default_value = parameter.default_value.clone();
                let fallback_value = parameter.value.clone();
                let read_only = parameter.read_only;
                let control_modes_enabled = parameter.control_modes_enabled;
                ctx.call_node_mutation(existing, move |node, _ctx| {
                    let Some(existing) =
                        node.as_any_mut().downcast_mut::<Parameter>()
                    else {
                        return Err(
                            "expected an Alchemist config parameter".into(),
                        );
                    };
                    existing.constraints = constraints;
                    existing.default_value = default_value;
                    existing.read_only = read_only;
                    existing.control_modes_enabled = control_modes_enabled;
                    existing.node_data_mut().meta.can_be_disabled =
                        can_be_disabled;
                    existing.node_data_mut().meta.presentation = presentation;
                    existing.value = existing
                        .constraints
                        .normalize(existing.value.clone())
                        .unwrap_or_else(|_| fallback_value.clone());
                    Ok(())
                });
            }
            return;
        }
        ctx.add_child(parent, parameter, None);
    }

    fn remove_obsolete_children(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent: NodeId,
        desired_decl_ids: &HashSet<String>,
    ) {
        for child in snapshot.child_ids(parent) {
            let Some(child_snapshot) = snapshot.node(child) else {
                continue;
            };
            if !desired_decl_ids.contains(child_snapshot.decl_id.as_str()) {
                if !node_removal_pending(ctx, child) {
                    ctx.edits.push(Edit::RemoveNode { node: child });
                }
            }
        }
    }
}

fn value_type_parameter(
    label: &str,
    decl_id: &str,
    selected_type: &str,
    read_only: bool,
    type_options: &[ValueTypeId],
) -> Parameter {
    let registry = value_types();
    let options = registry
        .iter()
        .filter(|descriptor| {
            !matches!(
                descriptor.storage,
                golden_alchemist::ValueStorageKind::Extension
            ) && (type_options.is_empty()
                || type_options.iter().any(|id| id == &descriptor.id))
        })
        .map(|descriptor| ParameterEnumOption {
            variant_id: descriptor.id.to_string(),
            value: ParamValue::Enum(descriptor.id.to_string()),
            label: descriptor.label.clone(),
            tags: Vec::new(),
            ordering: None,
        })
        .collect::<Vec<_>>();
    let selected_type = options
        .iter()
        .any(|option| option.variant_id == selected_type)
        .then_some(selected_type)
        .or_else(|| options.first().map(|option| option.variant_id.as_str()))
        .unwrap_or(selected_type);
    let mut parameter = parameter(
        label,
        decl_id,
        ParamValue::Enum(selected_type.to_owned()),
        read_only,
    );
    if read_only {
        parameter
            .node_data_mut()
            .meta
            .presentation
            .show_in_inspector_content = false;
    } else {
        // ANode-level type selector: disabled by default (auto-inferred from inputs)
        parameter.node_data_mut().meta.can_be_disabled = true;
        parameter.node_data_mut().meta.enabled = false;
    }
    parameter.constraints.enum_options = options;
    parameter.constraints.policy = ParameterConstraintPolicy::Reject;
    parameter
}

fn socket_value_type(
    snapshot: &ProcessTreeSnapshot,
    socket: NodeId,
    direction: &str,
    socket_id: &str,
) -> Option<String> {
    child_string(
        snapshot,
        socket,
        &format!("{direction}/{socket_id}/value_type"),
    )
}

fn input_socket_matches(
    snapshot: &ProcessTreeSnapshot,
    socket: NodeId,
    socket_id: &str,
    value_type: &ValueTypeId,
    default: &RuntimeValue,
) -> bool {
    if socket_value_type(snapshot, socket, "inputs", socket_id).as_deref()
        != Some(value_type.as_str())
    {
        return false;
    }
    let Some(value) = snapshot.find_child_by_decl_id(
        socket,
        &socket_value_decl_id("inputs", socket_id),
    ) else {
        return false;
    };
    snapshot.node(value).is_some_and(|node| {
        runtime_value_to_param(default)
            .is_ok_and(|default| node.node_type == parameter_node_type(&default))
    })
}

fn output_socket_matches(
    snapshot: &ProcessTreeSnapshot,
    socket: NodeId,
    socket_id: &str,
    value_type: &ValueTypeId,
) -> bool {
    socket_value_type(snapshot, socket, "outputs", socket_id).as_deref()
        == Some(value_type.as_str())
}

fn input_socket_tree(
    socket_id: &str,
    label: &str,
    value_type: &ValueTypeId,
    default: &RuntimeValue,
) -> Result<NodeTree, String> {
    let decl_id = socket_decl_id("inputs", socket_id);
    let mut socket = AlchemistInputSocket::new();
    socket.node_data_mut().meta.label = label.to_owned();
    socket.node_data_mut().meta.decl_id = DeclId(decl_id.clone());
    socket.node_data_mut().meta.presentation.color =
        Some(value_type_color(value_type.as_str()));
    let mut value = parameter(
        "Value",
        socket_value_decl_id("inputs", socket_id),
        runtime_value_to_param(default)?,
        false,
    );
    value.node_data_mut().meta.presentation.color =
        Some(value_type_color(value_type.as_str()));
    let mut socket_id_param = parameter(
        "Socket ID",
        format!("{decl_id}/socket_id"),
        ParamValue::Str(socket_id.to_owned()),
        true,
    );
    socket_id_param
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    let mut value_type_param = parameter(
        "Value Type",
        format!("{decl_id}/value_type"),
        ParamValue::Str(value_type.to_string()),
        true,
    );
    value_type_param
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    Ok(NodeTree::new(socket)
        .with_child(NodeTree::new(socket_id_param))
        .with_child(NodeTree::new(value_type_param))
        .with_child(NodeTree::new(value)))
}

fn output_socket_tree(
    socket_id: &str,
    label: &str,
    value_type: &ValueTypeId,
) -> NodeTree {
    let decl_id = socket_decl_id("outputs", socket_id);
    let mut socket = AlchemistOutputSocket::new();
    socket.node_data_mut().meta.label = label.to_owned();
    socket.node_data_mut().meta.decl_id = DeclId(decl_id.clone());
    socket.node_data_mut().meta.presentation.color =
        Some(value_type_color(value_type.as_str()));
    let mut socket_id_param = parameter(
        "Socket ID",
        format!("{decl_id}/socket_id"),
        ParamValue::Str(socket_id.to_owned()),
        true,
    );
    socket_id_param
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    let mut value_type_param = parameter(
        "Value Type",
        format!("{decl_id}/value_type"),
        ParamValue::Str(value_type.to_string()),
        true,
    );
    value_type_param
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    NodeTree::new(socket)
        .with_child(NodeTree::new(socket_id_param))
        .with_child(NodeTree::new(value_type_param))
}

pub(crate) const PROPERTY_MANAGER_ROLES: [(&str, &str); 3] = [
    ("condition", "Conditions"),
    ("input", "Inputs"),
    ("output", "Output Commands"),
];

const PROPERTY_TYPES: [(&str, &str); 12] = [
    ("trigger", "Trigger"),
    ("int", "Integer"),
    ("float", "Float"),
    ("str", "String"),
    ("file", "File"),
    ("enum", "Enum"),
    ("bool", "Boolean"),
    ("css_value", "CSS Value"),
    ("vec2", "Vector 2"),
    ("vec3", "Vector 3"),
    ("color", "Color"),
    ("reference", "Reference"),
];

fn locked_properties_permissions() -> NodeUserPermissions {
    let mut permissions = NodeUserPermissions::all();
    permissions.can_remove_and_duplicate = false;
    permissions.can_edit_name = false;
    permissions
}

fn property_default(property_type: &str) -> Option<ParamValue> {
    Some(match property_type {
        "trigger" => ParamValue::Trigger(),
        "int" => ParamValue::Int(0),
        "float" => ParamValue::Float(0.0),
        "str" => ParamValue::Str(String::new()),
        "file" => ParamValue::File(String::new()),
        "enum" => ParamValue::Enum("option".to_owned()),
        "bool" => ParamValue::Bool(false),
        "css_value" => ParamValue::CssValue(CssValue::default()),
        "vec2" => ParamValue::Vec2(0.0, 0.0),
        "vec3" => ParamValue::Vec3(0.0, 0.0, 0.0),
        "color" => ParamValue::Color(0.0, 0.0, 0.0, 1.0),
        "reference" => {
            ParamValue::Reference(NodeReference::new(NodeUuid::nil()))
        }
        _ => return None,
    })
}

fn property_value_type(property_type: &str) -> &'static str {
    match property_type {
        "trigger" => "trigger",
        "int" => "int",
        "float" => "float",
        "bool" => "bool",
        "vec2" => "vec2",
        "vec3" => "vec3",
        "color" => "color",
        "reference" => "chataigne.module_endpoint",
        _ => "string",
    }
}

fn property_parameter(property_type: &str) -> Option<Parameter> {
    let default = property_default(property_type)?;
    let color = parameter_value_color(&default);
    let mut value = parameter("Default value", "value", default, false);
    value.node_data_mut().meta.presentation.color = Some(color);
    // Allow users to edit range / step / enum options on the default value, and
    // ensure those constraints are persisted across save/load.
    let mut permissions = NodeUserPermissions::none();
    permissions.can_edit_constraints = true;
    value.node_data_mut().meta.user_permissions = permissions;
    if property_type == "enum" {
        value.constraints.enum_options = vec![ParameterEnumOption {
            variant_id: "option".to_owned(),
            value: ParamValue::Enum("option".to_owned()),
            label: "Option".to_owned(),
            tags: Vec::new(),
            ordering: None,
        }];
    }
    Some(value)
}

fn formula_property_creatable_items() -> Vec<UserCreatableItem> {
    let mut items = PROPERTY_TYPES
        .into_iter()
        .map(|(property_type, label)| {
            UserCreatableItem::new(
                format!("{PROPERTY_CREATE_PREFIX}{property_type}"),
                PROPERTY_ITEM_KIND,
                label,
            )
            .with_menu_path(["Parameter"])
        })
        .collect::<Vec<_>>();
    items.extend(PROPERTY_MANAGER_ROLES.into_iter().map(|(role, label)| {
        UserCreatableItem::new(
            format!("{PROPERTY_MANAGER_CREATE_PREFIX}{role}"),
            PROPERTY_MANAGER_ITEM_KIND,
            label,
        )
        .with_menu_path(["Manager"])
    }));
    items.push(UserCreatableItem::new(
        PROPERTY_FOLDER_NODE_TYPE,
        PROPERTY_FOLDER_ITEM_KIND,
        "Folder",
    ));
    items
}

fn formula_properties_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[
        PROPERTY_ITEM_KIND,
        PROPERTY_MANAGER_ITEM_KIND,
        PROPERTY_FOLDER_ITEM_KIND,
    ])
}

fn formula_properties_accepts(
    item_type: &str,
    item_kind: &str,
) -> bool {
    match item_kind {
        PROPERTY_ITEM_KIND => {
            item_type == PROPERTY_NODE_TYPE
                || item_type.starts_with(PROPERTY_CREATE_PREFIX)
        }
        PROPERTY_MANAGER_ITEM_KIND => {
            item_type == PROPERTY_MANAGER_NODE_TYPE
                || item_type.starts_with(PROPERTY_MANAGER_CREATE_PREFIX)
        }
        PROPERTY_FOLDER_ITEM_KIND => item_type == PROPERTY_FOLDER_NODE_TYPE,
        _ => false,
    }
}

fn create_formula_property_item(node_type: &str) -> Option<Box<dyn Node>> {
    if node_type == PROPERTY_NODE_TYPE {
        return Some(Box::new(AlchemistProperty::for_type("float")));
    }
    if node_type == PROPERTY_FOLDER_NODE_TYPE {
        return Some(Box::new(AlchemistPropertyFolder::new()));
    }
    if let Some(property_type) = node_type.strip_prefix(PROPERTY_CREATE_PREFIX) {
        return property_default(property_type)
            .map(|_| Box::new(AlchemistProperty::for_type(property_type)) as _);
    }
    let role = node_type.strip_prefix(PROPERTY_MANAGER_CREATE_PREFIX)?;
    PROPERTY_MANAGER_ROLES
        .iter()
        .find(|(candidate, _)| *candidate == role)
        .map(|(_, label)| {
            let mut manager = AlchemistPropertyManager::for_role(role);
            manager.node_data_mut().meta.label = (*label).to_owned();
            Box::new(manager) as Box<dyn Node>
        })
}

fn properties_tree() -> NodeTree {
    let mut properties = AlchemistPropertiesManager::new();
    properties.node_data_mut().meta.decl_id =
        DeclId(PROPERTIES_DECL_ID.to_owned());
    NodeTree::new(properties)
}

#[node("alchemist_properties", label = "Properties")]
pub struct AlchemistPropertiesManager {}

#[node("alchemist_properties", from_struct)]
impl Node for AlchemistPropertiesManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(formula_properties_container_rules())
    }

    fn user_container_accepts_item(
        &self,
        item_type: &str,
        item_kind: &str,
    ) -> bool {
        formula_properties_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        formula_property_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_formula_property_item(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions =
            locked_properties_permissions();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("alchemist_property_folder", label = "Folder")]
pub struct AlchemistPropertyFolder {}

#[node("alchemist_property_folder", from_struct)]
impl Node for AlchemistPropertyFolder {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(formula_properties_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        formula_properties_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        formula_property_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_formula_property_item(node_type)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("alchemist_property_manager", label = "Manager")]
#[children(
    role: golden_core::parameter::Enum = "condition" (
        label = "Role",
        enum_options = [
            "condition",
            "input",
            "output"
        ],
        read_only = true,
        show_in_inspector_content = false
    );
)]
pub struct AlchemistPropertyManager {}

#[node("alchemist_property_manager", from_struct)]
impl Node for AlchemistPropertyManager {
    fn user_item_kind(&self) -> &str {
        PROPERTY_MANAGER_ITEM_KIND
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_edit_name = false;
        self.node_data_mut().meta.user_permissions = permissions;
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

impl AlchemistPropertyManager {
    fn for_role(role: &str) -> Self {
        let mut manager = Self::new();
        manager
            .role
            .apply_runtime_value(&ParamValue::Enum(role.to_owned()));
        manager
    }
}

#[node("alchemist_property", label = "Property")]
#[children(
    property_type: String = String::from("float") (
        label = "Parameter Type",
        read_only = true,
        show_in_inspector_content = false
    );
    exposed: bool = true (
        label = "Exposed"
    );
)]
pub struct AlchemistProperty {}

#[node("alchemist_property", from_struct)]
impl Node for AlchemistProperty {
    fn user_item_kind(&self) -> &str {
        PROPERTY_ITEM_KIND
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        self.node_data_mut().meta.can_be_disabled = false;
        self.reconcile_value(ctx);
    }

    fn on_node_ready(
        &mut self,
        ctx: &mut ProcessCtx,
        _context: NodeCreationContext,
    ) {
        self.reconcile_value(ctx);
    }

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        if param == self.property_type.id() {
            self.reconcile_value(ctx);
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

impl AlchemistProperty {
    fn for_type(property_type: &str) -> Self {
        let mut property = Self::new();
        property.property_type.apply_runtime_value(&ParamValue::Str(
            property_type.to_owned(),
        ));
        set_tag(
            &mut property.node_data_mut().meta.tags,
            PROPERTY_TYPE_TAG_PREFIX,
            property_type,
        );
        property.node_data_mut().meta.presentation.color =
            Some(value_type_color(property_type));
        property
    }

    fn reconcile_value(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };

        // Newly created typed properties carry their type in node metadata so
        // persistence cannot collapse them back to the default hidden field.
        // Legacy files still resolve through the saved child.
        let Some(property_type) =
            tagged_value(&self.node_data().meta.tags, PROPERTY_TYPE_TAG_PREFIX)
                .map(str::trim)
                .filter(|value| property_default(value).is_some())
                .map(ToOwned::to_owned)
                .or_else(|| {
                    child_string(&snapshot, self.id(), "property_type")
                        .filter(|value| property_default(value).is_some())
                })
                .or_else(|| {
                    let fallback = self.property_type.get_ref();
                    (fallback != "float").then(|| fallback.to_owned())
                })
        else {
            return;
        };

        if self.node_data().meta.presentation.color.is_none() {
            self.node_data_mut().meta.presentation.color =
                Some(value_type_color(&property_type));
        }

        if let Some(property_type_param) =
            snapshot.find_child_by_decl_id(self.id(), "property_type")
        {
            let desired = ParamValue::Str(property_type.clone());
            if snapshot
                .node(property_type_param)
                .and_then(|node| node.param_value.as_ref())
                != Some(&desired)
            {
                ctx.edits.push(Edit::SetParam {
                    node: property_type_param,
                    value: desired,
                    behaviour: ParameterEventBehaviour::Coalesce,
                });
            }
        }

        // Remove legacy / obsolete children
        for decl_id in ["value_type", "range_min", "range_max", "options"] {
            if let Some(child) =
                snapshot.find_child_by_decl_id(self.id(), decl_id)
            {
                self.remove_child(ctx, child);
            }
        }

        let Some(value) = property_parameter(&property_type) else {
            return;
        };
        let desired_type = value.get_type();

        // Collect every `value` child. Load/reconcile races can leave more than
        // one (e.g. a transient default created before the saved child loads),
        // so we keep a single one of the right type and drop the rest.
        let value_children: Vec<NodeId> = snapshot
            .child_ids(self.id())
            .into_iter()
            .filter(|child| {
                snapshot
                    .node(*child)
                    .is_some_and(|node| node.decl_id == "value")
            })
            .collect();

        let mut kept: Option<NodeId> = None;
        for child in value_children {
            let matches_type = snapshot
                .node(child)
                .is_some_and(|node| node.node_type == desired_type);
            if kept.is_none() && matches_type {
                kept = Some(child);
            } else {
                self.remove_child(ctx, child);
            }
        }

        if let Some(existing) = kept {
            // Type matches — only refresh the display color, preserve
            // user-edited constraints (range, enum options, etc.)
            let color = value.node_data().meta.presentation.color;
            ctx.call_node_mutation(existing, move |node, _ctx| {
                node.node_data_mut().meta.presentation.color = color;
                Ok(())
            });
        } else {
            ctx.add_child(self.id(), value, None);
        }
    }
}

fn formula_surface_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
    graph: &mut AlchemistGraph,
) -> Result<FormulaSurface, String> {
    let Some(properties) =
        snapshot.find_child_by_decl_id(formula_node, PROPERTIES_DECL_ID)
    else {
        return Ok(FormulaSurface::default());
    };
    let mut parameter_items = Vec::new();
    let mut manager_sections = Vec::new();
    for child in snapshot.child_ids(properties) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if node.node_type == PROPERTY_NODE_TYPE {
            parameter_items.push(surface_item_from_property(
                snapshot, child, graph,
            )?);
        } else if node.node_type == PROPERTY_MANAGER_NODE_TYPE {
            let role = child_string(snapshot, child, "role")
                .unwrap_or_else(|| "condition".to_owned());
            manager_sections.push(SurfaceSection {
                id: SurfaceSectionId::new(role.clone()),
                label: node.label.clone(),
                items: Vec::new(),
                source: SurfaceSource::Formula,
            });
        }
    }
    let mut sections = vec![SurfaceSection {
        id: SurfaceSectionId::new("parameter"),
        label: "Parameters".to_owned(),
        items: parameter_items,
        source: SurfaceSource::Formula,
    }];
    sections.extend(manager_sections);
    Ok(FormulaSurface { sections })
}

fn surface_item_from_property(
    snapshot: &ProcessTreeSnapshot,
    property: NodeId,
    graph: &mut AlchemistGraph,
) -> Result<SurfaceItem, String> {
    let node = snapshot
        .node(property)
        .ok_or_else(|| format!("Property {property:?} is missing"))?;
    let value = child_param(snapshot, property, "value")
        .ok_or_else(|| format!("Property `{}` has no value", node.label))?;
    let value_type = ValueTypeId::new(property_value_type(
        child_string(snapshot, property, "property_type")
            .as_deref()
            .unwrap_or("float"),
    ));
    let runtime_value = param_to_runtime_value(value, &value_type)?;
    let id = node.uuid.0.to_string();
    Ok(SurfaceItem {
        id: SurfaceItemId::new(id.clone()),
        label: node.label.clone(),
        description: None,
        path: Vec::new(),
        kind: SurfaceItemKind::Parameter,
        value_type: Some(ValueTypeSpec::Exact(value_type)),
        ui: ParamUiHints::default(),
        bindings: property_bindings(graph, &id, &runtime_value),
    })
}

fn property_bindings(
    graph: &mut AlchemistGraph,
    property_id: &str,
    default_value: &RuntimeValue,
) -> Vec<ANodeFieldPath> {
    graph
        .nodes
        .values_mut()
        .filter_map(|node| {
            let matches_property = node.type_id.as_str() == PROPERTY_ANODE_TYPE
                && node.config.get("property_id").is_some_and(|value| {
                    matches!(
                        value,
                        RuntimeValue::String(value) if value.as_ref() == property_id
                    )
                });
            if !matches_property {
                return None;
            }
            node.config.set("value", default_value.clone());
            Some(ANodeFieldPath::new(node.id, "value"))
        })
        .collect()
}

#[node("alchemist_connection", label = "Connection")]
#[children(
    source_node: NodeReference (
        label = "Source",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![ANODE_NODE_TYPE.to_owned()],
        reference_allow_projections = false
    );
    source_socket: String = String::new() (label = "Source Socket");
    target_node: NodeReference (
        label = "Target",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![ANODE_NODE_TYPE.to_owned()],
        reference_allow_projections = false
    );
    target_socket: String = String::new() (label = "Target Socket");
)]
pub struct AlchemistConnection {}

#[node("alchemist_connection", from_struct)]
impl Node for AlchemistConnection {
    fn user_item_kind(&self) -> &str {
        CONNECTION_ITEM_KIND
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("alchemist_formula", label = "Formula")]
#[children(
    is_valid: bool = false (label = "Valid", read_only = true, show_in_inspector_content = false);
    diagnostics_json: String = String::from("[]") (
        label = "Diagnostics",
        read_only = true,
        show_in_inspector_content = false
    );
)]
pub struct AlchemistFormulaDefinition {}

#[item("alchemist_formula", node = "alchemist_formula", from_struct)]
impl Node for AlchemistFormulaDefinition {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        self.node_data_mut().meta.can_be_disabled = false;
        self.reconcile_properties(ctx);
    }

    fn on_node_ready(
        &mut self,
        ctx: &mut ProcessCtx,
        _context: NodeCreationContext,
    ) {
        self.reconcile_properties(ctx);
        self.sync_property_getters(ctx);
        self.sync_anode_sockets(ctx, None);
        self.validate(ctx);
    }

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        if param != self.is_valid.id() && param != self.diagnostics_json.id() {
            let skip_anode = ctx.tree_snapshot().and_then(|snapshot| {
                let child = direct_child_under(snapshot, self.id(), param)?;
                snapshot
                    .node(child)
                    .is_some_and(|node| node.node_type == ANODE_NODE_TYPE)
                    .then_some(child)
                    .filter(|anode| {
                        !is_anode_type_variable_config_param(
                            snapshot,
                            *anode,
                            param,
                        )
                    })
            });
            self.sync_property_getters(ctx);
            self.sync_anode_sockets(ctx, skip_anode);
            self.validate(ctx);
        }
    }

    fn on_child_added(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: NodeId,
        _child: NodeId,
    ) {
        self.sync_property_getters(ctx);
        self.sync_anode_sockets(ctx, None);
        self.validate(ctx);
    }

    fn on_child_removed(
        &mut self,
        ctx: &mut ProcessCtx,
        _parent: NodeId,
        _child: NodeId,
    ) {
        self.remove_dangling_connections(ctx);
        self.sync_property_getters(ctx);
        self.sync_anode_sockets(ctx, None);
        self.validate(ctx);
    }

    fn on_meta_changed(
        &mut self,
        ctx: &mut ProcessCtx,
        _node: NodeId,
        _patch: NodeMetaPatch,
    ) {
        self.sync_property_getters(ctx);
        self.sync_anode_sockets(ctx, None);
        self.validate(ctx);
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        8
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            ANODE_ITEM_KIND,
            CONNECTION_ITEM_KIND,
        ]))
    }

    fn user_container_accepts_item(
        &self,
        item_type: &str,
        item_kind: &str,
    ) -> bool {
        (item_kind == ANODE_ITEM_KIND
            && (item_type == ANODE_NODE_TYPE
                || item_type.starts_with(ANODE_CREATE_PREFIX)))
            || (item_type == CONNECTION_NODE_TYPE
                && item_kind == CONNECTION_ITEM_KIND)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = registry()
            .iter()
            .map(|declaration| {
                UserCreatableItem::new(
                    format!("{ANODE_CREATE_PREFIX}{}", declaration.type_id()),
                    ANODE_ITEM_KIND,
                    declaration.label(),
                )
                .with_menu_path([declaration.category()])
            })
            .collect::<Vec<_>>();
        items.push(
            UserCreatableItem::new(
                CONNECTION_NODE_TYPE,
                CONNECTION_ITEM_KIND,
                "Connection",
            )
            .with_menu_path(["Graph"]),
        );
        items
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if node_type == CONNECTION_NODE_TYPE {
            return Some(Box::new(AlchemistConnection::new()));
        }
        if node_type == ANODE_NODE_TYPE {
            return Some(Box::new(AlchemistANode::new()));
        }
        let type_id = node_type.strip_prefix(ANODE_CREATE_PREFIX)?;
        let registry = registry();
        let declaration = registry.get(&ANodeTypeId::new(type_id))?;
        Some(Box::new(AlchemistANode::for_type(
            type_id,
            declaration.label(),
            declaration.category(),
        )))
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

impl AlchemistFormulaDefinition {
    fn reconcile_properties(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        if snapshot
            .find_child_by_decl_id(self.id(), PROPERTIES_DECL_ID)
            .is_none()
        {
            ctx.add_child_tree(self.id(), properties_tree(), None);
        }
    }

    fn remove_dangling_connections(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let anode_uuids = snapshot
            .child_ids(self.id())
            .into_iter()
            .filter_map(|child| {
                let node = snapshot.node(child)?;
                (node.node_type == ANODE_NODE_TYPE).then_some(node.uuid)
            })
            .collect::<HashSet<_>>();

        for child in snapshot.child_ids(self.id()) {
            let Some(node) = snapshot.node(child) else {
                continue;
            };
            if node.node_type != CONNECTION_NODE_TYPE {
                continue;
            }
            let source =
                child_reference_uuid(&snapshot, child, "source_node");
            let target =
                child_reference_uuid(&snapshot, child, "target_node");
            if source.is_none_or(|uuid| !anode_uuids.contains(&uuid))
                || target.is_none_or(|uuid| !anode_uuids.contains(&uuid))
            {
                if !node_removal_pending(ctx, child) {
                    ctx.edits.push(Edit::RemoveNode { node: child });
                }
            }
        }
    }

    fn sync_anode_sockets(&self, ctx: &mut ProcessCtx, skip_anode: Option<NodeId>) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let nodes = registry();
        for child in snapshot.child_ids(self.id()) {
            if Some(child) == skip_anode {
                continue;
            }
            let Some(anode) = snapshot.node(child) else {
                continue;
            };
            if anode.node_type != ANODE_NODE_TYPE {
                continue;
            }
            let Some(type_id) = anode_type_from_tags(&anode.tags) else {
                continue;
            };
            if nodes.get(&ANodeTypeId::new(&type_id)).is_none() {
                continue;
            }
            let Some(config_folder) =
                snapshot.find_child_by_decl_id(child, "config")
            else {
                continue;
            };
            sync_auto_input_count(ctx, &snapshot, child, config_folder, &type_id);
        }
        let Ok(formula) = formula_from_snapshot(&snapshot, self.id()) else {
            return;
        };
        let value_types = value_types();
        let solved = solve_types(
            &formula.graph,
            &TypeSolveCtx {
                value_types: &value_types,
                nodes: &nodes,
            },
        );
        let signature_ctx = SignatureCtx {
            value_types: &value_types,
        };

        for child in snapshot.child_ids(self.id()) {
            if Some(child) == skip_anode {
                continue;
            }
            let Some(anode) = snapshot.node(child) else {
                continue;
            };
            if anode.node_type != ANODE_NODE_TYPE {
                continue;
            }
            let anode_id = ANodeId::from_uuid(anode.uuid.0);
            let Some(instance) = formula.graph.nodes.get(&anode_id) else {
                continue;
            };
            let Some(declaration) = nodes.get(&instance.type_id) else {
                continue;
            };
            let config_folder = snapshot.find_child_by_decl_id(child, "config");
            let Some(inputs_folder) =
                snapshot.find_child_by_decl_id(child, "inputs")
            else {
                continue;
            };
            let Some(outputs_folder) =
                snapshot.find_child_by_decl_id(child, "outputs")
            else {
                continue;
            };
            let signature =
                declaration.signature(&signature_ctx, instance, &instance.type_bindings);
            let fallback_bindings = local_signature_bindings(&signature, instance);
            let resolved_node = solved.graph.nodes.get(&anode_id);
            let resolved_bindings = resolved_node
                .map(|node| &node.bindings)
                .unwrap_or(&fallback_bindings);

            if let Some(config_folder) = config_folder {
                sync_auto_type_variable_config_params(
                    ctx,
                    &snapshot,
                    config_folder,
                    declaration.as_ref(),
                    &signature,
                    &value_types,
                    resolved_bindings,
                );
            }

            for input in signature.inputs {
                let value_type = resolved_node
                    .and_then(|node| node.signature.inputs.get(&input.id))
                    .and_then(|socket| socket.value_type.clone())
                    .unwrap_or_else(|| {
                        constraint_value_type(&input.constraint, &fallback_bindings)
                    });
                let default = instance
                    .input_defaults
                    .get(&input.id)
                    .cloned()
                    .or(input.default_value)
                    .and_then(|value| value.convert_to(&value_type).ok())
                    .or_else(|| default_runtime_value(&value_type).ok())
                    .unwrap_or(RuntimeValue::Float(0.0));
                let decl_id = socket_decl_id("inputs", input.id.as_str());
                let existing =
                    snapshot.find_child_by_decl_id(inputs_folder, &decl_id);
                let needs_rebuild = existing.is_none_or(|socket| {
                    !input_socket_matches(
                        &snapshot,
                        socket,
                        input.id.as_str(),
                        &value_type,
                        &default,
                    )
                });
                if !needs_rebuild {
                    continue;
                }
                if let Ok(tree) = input_socket_tree(
                    input.id.as_str(),
                    &input.label,
                    &value_type,
                    &default,
                ) {
                    replace_child_tree_once(ctx, inputs_folder, existing, tree);
                }
            }

            for output in signature.outputs {
                let value_type = resolved_node
                    .and_then(|node| node.signature.outputs.get(&output.id))
                    .and_then(|socket| socket.value_type.clone())
                    .unwrap_or_else(|| {
                        constraint_value_type(&output.constraint, &fallback_bindings)
                    });
                let decl_id = socket_decl_id("outputs", output.id.as_str());
                let existing =
                    snapshot.find_child_by_decl_id(outputs_folder, &decl_id);
                let needs_rebuild = existing.is_none_or(|socket| {
                    !output_socket_matches(
                        &snapshot,
                        socket,
                        output.id.as_str(),
                        &value_type,
                    )
                });
                if !needs_rebuild {
                    continue;
                }
                replace_child_tree_once(
                    ctx,
                    outputs_folder,
                    existing,
                    output_socket_tree(
                        output.id.as_str(),
                        &output.label,
                        &value_type,
                    ),
                );
            }
        }
    }

    fn sync_property_getters(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(properties) =
            snapshot.find_child_by_decl_id(self.id(), PROPERTIES_DECL_ID)
        else {
            return;
        };
        let properties_by_id = snapshot
            .child_ids(properties)
            .into_iter()
            .filter_map(|property| {
                let node = snapshot.node(property)?;
                (node.node_type == PROPERTY_NODE_TYPE).then(|| {
                    (
                        node.uuid.0.to_string(),
                        (node.label.clone(), node.presentation.color),
                    )
                })
            })
            .collect::<HashMap<_, _>>();

        for child in snapshot.child_ids(self.id()) {
            let Some(anode) = snapshot.node(child) else {
                continue;
            };
            if anode.node_type != ANODE_NODE_TYPE
                || anode_type_from_tags(&anode.tags).as_deref()
                    != Some(PROPERTY_ANODE_TYPE)
            {
                continue;
            }
            let Some(config) =
                snapshot.find_child_by_decl_id(child, "config")
            else {
                continue;
            };
            let Some(property_id) =
                child_string(&snapshot, config, "config/property_id")
            else {
                continue;
            };
            let Some((label, color)) = properties_by_id.get(&property_id)
            else {
                continue;
            };
            if &anode.label != label || anode.presentation.color != *color {
                let mut presentation = anode.presentation.clone();
                presentation.color = *color;
                ctx.patch_node_meta(
                    child,
                    NodeMetaPatch {
                        label: (&anode.label != label).then(|| label.clone()),
                        presentation: (anode.presentation.color != *color)
                            .then_some(presentation),
                        ..NodeMetaPatch::default()
                    },
                );
            }
        }
    }

    fn validate(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        let anode_node_ids = formula_anode_node_ids(snapshot, self.id());
        let mut node_diagnostics = HashMap::<NodeId, Vec<String>>::new();
        let result = formula_from_snapshot(snapshot, self.id()).map(|formula| {
            let value_types = value_types();
            let nodes = registry();
            compile_graph(
                &formula.graph,
                &CompileCtx {
                    value_types: &value_types,
                    nodes: &nodes,
                },
            )
        });
        let (valid, diagnostics) = match result {
            Ok(compilation) => {
                for diagnostic in &compilation.diagnostics {
                    if !matches!(
                        diagnostic.severity,
                        DiagnosticSeverity::Warning | DiagnosticSeverity::Error
                    ) {
                        continue;
                    }
                    if let Some(node_id) = diagnostic_origin_node_id(
                        &diagnostic.origin,
                        &anode_node_ids,
                    ) {
                        node_diagnostics
                            .entry(node_id)
                            .or_default()
                            .push(diagnostic.message.clone());
                    }
                }
                let diagnostics = compilation
                    .diagnostics
                    .iter()
                    .map(|diagnostic| {
                        serde_json::json!({
                            "code": diagnostic.code,
                            "message": diagnostic.message,
                            "severity": match diagnostic.severity {
                                DiagnosticSeverity::Info => "info",
                                DiagnosticSeverity::Warning => "warning",
                                DiagnosticSeverity::Error => "error",
                            },
                            "origin": format!("{:?}", diagnostic.origin),
                        })
                    })
                    .collect::<Vec<_>>();
                (!compilation.has_errors(), diagnostics)
            }
            Err(message) => (
                false,
                vec![serde_json::json!({
                    "code": "invalid_formula_tree",
                    "message": message,
                    "severity": "error",
                    "origin": "Formula",
                })],
            ),
        };
        let diagnostics_json = serde_json::to_string(&diagnostics)
            .expect("Formula diagnostics must serialize");
        let diagnostic_node_ids =
            node_diagnostics.keys().copied().collect::<HashSet<_>>();
        let anode_warnings_to_clear = anode_node_ids
            .values()
            .copied()
            .filter(|node_id| {
                !diagnostic_node_ids.contains(node_id)
                    && node_has_warning(
                        snapshot,
                        *node_id,
                        ANODE_FORMULA_DIAGNOSTIC_WARNING_ID,
                    )
            })
            .collect::<Vec<_>>();
        let anode_warnings_to_set = node_diagnostics
            .into_iter()
            .filter_map(|(node_id, messages)| {
                let detail = messages.join("\n");
                let title = if messages.len() > 1 {
                    "Formula issues"
                } else {
                    "Formula issue"
                };
                (!node_warning_matches(
                    snapshot,
                    node_id,
                    ANODE_FORMULA_DIAGNOSTIC_WARNING_ID,
                    title,
                    Some(&detail),
                ))
                .then_some((node_id, title, detail))
            })
            .collect::<Vec<_>>();
        let formula_warning_to_clear =
            valid && node_has_warning(snapshot, self.id(), FORMULA_WARNING_ID);
        let formula_warning_to_set = if valid {
            None
        } else {
            let summary = diagnostics
                .first()
                .and_then(|diagnostic| diagnostic["message"].as_str())
                .unwrap_or("Formula compilation failed");
            (!node_warning_matches(
                snapshot,
                self.id(),
                FORMULA_WARNING_ID,
                "Formula is invalid",
                Some(summary),
            ))
            .then_some(summary.to_owned())
        };
        self.is_valid.set(ctx, valid);
        self.diagnostics_json.set(ctx, diagnostics_json);
        for node_id in anode_warnings_to_clear {
            ctx.clear_node_warning(
                node_id,
                Some(ANODE_FORMULA_DIAGNOSTIC_WARNING_ID),
            );
        }
        for (node_id, title, detail) in anode_warnings_to_set {
            ctx.set_node_warning_with(
                node_id,
                Some(ANODE_FORMULA_DIAGNOSTIC_WARNING_ID),
                title,
                Some(&detail),
            );
        }
        if valid {
            if formula_warning_to_clear {
                ctx.clear_node_warning(self.id(), Some(FORMULA_WARNING_ID));
            }
        } else if let Some(summary) = formula_warning_to_set {
            ctx.set_node_warning_with(
                self.id(),
                Some(FORMULA_WARNING_ID),
                "Formula is invalid",
                Some(&summary),
            );
        }
    }
}

#[node("alchemist_formula_library", label = "Formulas")]
pub struct FormulaLibrary {}

#[node("alchemist_formula_library", from_struct)]
impl Node for FormulaLibrary {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(formula_container_rules())
    }

    fn user_container_accepts_item(
        &self,
        item_type: &str,
        item_kind: &str,
    ) -> bool {
        formula_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        formula_container_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_formula_container_item(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }
}

#[node("alchemist_formula_folder", label = "Folder")]
pub struct AlchemistFormulaFolder {}

#[node("alchemist_formula_folder", from_struct)]
impl Node for AlchemistFormulaFolder {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(formula_container_rules())
    }

    fn user_container_accepts_item(
        &self,
        item_type: &str,
        item_kind: &str,
    ) -> bool {
        formula_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        formula_container_creatable_items()
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        create_formula_container_item(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod formula_tests;
