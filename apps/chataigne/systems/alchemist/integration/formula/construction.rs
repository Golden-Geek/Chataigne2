use super::*;
use chataigne_alchemist::configured_managed_variant;

pub(super) fn tagged_value<'a>(tags: &'a [String], prefix: &str) -> Option<&'a str> {
    tags.iter().find_map(|tag| tag.strip_prefix(prefix))
}

pub(super) fn set_tag(tags: &mut Vec<String>, prefix: &str, value: &str) {
    tags.retain(|tag| !tag.starts_with(prefix));
    tags.push(format!("{prefix}{value}"));
}

pub(super) fn anode_type_from_tags(tags: &[String]) -> Option<String> {
    tagged_value(tags, ANODE_TYPE_TAG_PREFIX)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn stable_hash(value: &str) -> u32 {
    let mut hash = 2_166_136_261_u32;
    for byte in value.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

pub(super) fn family_hue(family: &str) -> u32 {
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

pub(super) fn hsl_color(hue: f64, saturation: f64, lightness: f64) -> Color {
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

pub(super) fn anode_default_color(family: &str, type_id: &str) -> Color {
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

pub(super) fn manager_role_anode_type(role: &str) -> Option<&'static str> {
    match role {
        "condition" => Some(chataigne_state_machine::alchemist::CONDITIONS_MANAGER_TYPE),
        "filter" => Some(chataigne_state_machine::alchemist::FILTERS_MANAGER_TYPE),
        "input" => Some(chataigne_state_machine::alchemist::INPUTS_MANAGER_TYPE),
        "output" => Some(chataigne_state_machine::alchemist::OUTPUTS_MANAGER_TYPE),
        _ => None,
    }
}

pub(super) fn property_manager_role_from_tags(tags: &[String]) -> Option<String> {
    tagged_value(tags, PROPERTY_MANAGER_ROLE_TAG_PREFIX)
        .map(str::trim)
        .filter(|role| manager_role_anode_type(role).is_some())
        .map(ToOwned::to_owned)
}

pub(super) fn property_manager_role_from_label(label: &str) -> Option<&'static str> {
    PROPERTY_MANAGER_ROLES
        .iter()
        .find(|(_, candidate)| *candidate == label)
        .map(|(role, _)| *role)
}

pub(super) fn manager_role_color(role: &str) -> Option<Color> {
    manager_role_anode_type(role).map(|type_id| anode_default_color("Managers", type_id))
}

pub(super) fn value_type_color(type_id: &str) -> Color {
    match type_id {
        "trigger" => Color::new(0.98, 0.42, 0.22, 1.0),
        "int" => Color::new(0.35, 0.62, 0.95, 1.0),
        "float" => Color::new(0.28, 0.74, 0.52, 1.0),
        "bool" => Color::new(0.86, 0.44, 0.78, 1.0),
        "vec2" => Color::new(0.32, 0.72, 0.92, 1.0),
        "vec3" => Color::new(0.48, 0.58, 0.94, 1.0),
        "color" => Color::new(0.98, 0.36, 0.32, 1.0),
        chataigne_state_machine::alchemist::VALUE_SET_TYPE => {
            Color::new(0.28, 0.68, 0.72, 1.0)
        }
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

pub(super) fn parameter_value_color(value: &ParamValue) -> Color {
    value_type_color(parameter_node_type(value))
}

pub(super) fn registry() -> &'static chataigne_alchemist::ANodeRegistry {
    chataigne_state_machine::alchemist::shared_node_registry()
}

pub(super) fn value_types() -> &'static chataigne_alchemist::ValueTypeRegistry {
    chataigne_state_machine::alchemist::shared_value_type_registry()
}

pub(super) fn formula_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[FORMULA_ITEM_KIND, FORMULA_FOLDER_ITEM_KIND])
}

pub(super) fn formula_container_accepts(item_type: &str, item_kind: &str) -> bool {
    match item_kind {
        FORMULA_ITEM_KIND => {
            item_type == FORMULA_EXTERNAL_FILE_CREATE_TYPE
                || crate::app::declared_user_item_type_matches(
                    item_type,
                    FORMULA_ITEM_KIND,
                )
        }
        FORMULA_FOLDER_ITEM_KIND => item_type == FORMULA_FOLDER_NODE_TYPE,
        _ => false,
    }
}

pub(super) fn formula_container_creatable_items() -> Vec<UserCreatableItem> {
    let mut items = crate::app::declared_user_creatable_items(FORMULA_ITEM_KIND);
    items.push(UserCreatableItem::new(
        FORMULA_EXTERNAL_FILE_CREATE_TYPE,
        FORMULA_ITEM_KIND,
        "External Formula",
    ));
    items.push(UserCreatableItem::new(
        FORMULA_FOLDER_NODE_TYPE,
        FORMULA_FOLDER_ITEM_KIND,
        "Folder",
    ));
    items
}

pub(crate) fn anode_creatable_items_for_roles(
    roles: &[SurfaceItemKind],
) -> Vec<UserCreatableItem> {
    registry()
        .iter()
        .filter(|declaration| {
            roles.is_empty()
                || roles
                    .iter()
                    .any(|role| declaration.supports_role(*role))
        })
        .map(|declaration| {
            UserCreatableItem::new(
                format!("{ANODE_CREATE_PREFIX}{}", declaration.type_id()),
                ANODE_ITEM_KIND,
                declaration.label(),
            )
            .with_menu_path([declaration.category()])
        })
        .collect()
}

pub(crate) fn anode_container_accepts_for_roles(
    item_type: &str,
    item_kind: &str,
    roles: &[SurfaceItemKind],
) -> bool {
    if item_kind != ANODE_ITEM_KIND {
        return false;
    }
    if item_type == ANODE_NODE_TYPE {
        return true;
    }
    let Some((type_id, variant)) = anode_create_spec(item_type) else {
        return false;
    };
    registry()
        .get(&ANodeTypeId::new(type_id))
        .is_some_and(|declaration| {
            variant.is_none_or(|variant| {
                configured_managed_variant(declaration.as_ref(), variant.index, variant.input_count).is_some()
            }) && (roles.is_empty()
                || roles
                    .iter()
                    .any(|role| declaration.supports_role(*role)))
        })
}

#[derive(Clone, Copy)]
struct ManagedVariantSpec {
    index: usize,
    input_count: Option<usize>,
}

fn anode_create_spec(node_type: &str) -> Option<(&str, Option<ManagedVariantSpec>)> {
    let spec = node_type.strip_prefix(ANODE_CREATE_PREFIX)?;
    if let Some((type_id, variant)) = spec.split_once(ANODE_MANAGED_VARIANT_SEPARATOR) {
        let (index, input_count) = match variant.split_once("/inputs/") {
            Some((index, input_count)) => (index, Some(input_count.parse().ok()?)),
            None => (variant, None),
        };
        return Some((type_id, Some(ManagedVariantSpec {
            index: index.parse().ok()?,
            input_count,
        })));
    }
    Some((spec, None))
}

pub(crate) fn create_anode_user_item(node_type: &str) -> Option<Box<dyn Node>> {
    if node_type == ANODE_NODE_TYPE {
        return Some(Box::new(AlchemistANode::new()));
    }
    let (type_id, variant) = anode_create_spec(node_type)?;
    let registry = registry();
    let declaration = registry.get(&ANodeTypeId::new(type_id))?;
    if variant.is_some_and(|variant| {
        configured_managed_variant(declaration.as_ref(), variant.index, variant.input_count).is_none()
    }) {
        return None;
    }
    Some(Box::new(AlchemistANode::for_type(
        type_id,
        declaration.label(),
        declaration.category(),
    )))
}

pub(crate) fn create_anode_user_item_tree(node_type: &str) -> Option<NodeTree> {
    if node_type == ANODE_NODE_TYPE {
        return Some(NodeTree::new(AlchemistANode::new()));
    }
    let (type_id, variant) = anode_create_spec(node_type)?;
    let registry = registry();
    let declaration = registry.get(&ANodeTypeId::new(type_id))?;
    match variant {
        Some(variant) => {
            let variant = configured_managed_variant(declaration.as_ref(), variant.index, variant.input_count)?;
            anode_tree_for_configured_instance(type_id, declaration.category(), declaration.as_ref(), variant)
        }
        None => Some(anode_tree_for_declaration(
            type_id,
            declaration.label(),
            declaration.category(),
            declaration.as_ref(),
        )),
    }
}

pub(super) fn create_formula_container_item(node_type: &str) -> Option<Box<dyn Node>> {
    if node_type == FORMULA_EXTERNAL_FILE_CREATE_TYPE {
        return Some(Box::new(external_formula_node()));
    }
    crate::app::create_declared_user_item(node_type, FORMULA_ITEM_KIND)
        .or_else(|| {
            (node_type == FORMULA_FOLDER_NODE_TYPE).then(|| {
                Box::new(AlchemistFormulaFolder::new()) as Box<dyn Node>
            })
        })
}

pub(super) fn create_formula_container_item_tree(node_type: &str) -> Option<NodeTree> {
    if node_type == FORMULA_EXTERNAL_FILE_CREATE_TYPE {
        return Some(external_formula_tree());
    }
    if node_type == AlchemistFormulaDefinition::NODE_TYPE {
        return Some(
            NodeTree::new(AlchemistFormulaDefinition::new())
                .with_child(NodeTree::new(formula_copy_source_parameter())),
        );
    }
    create_formula_container_item(node_type).map(NodeTree::boxed)
}

pub(super) fn parameter(
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

pub(super) fn hidden_parameter(
    label: &str,
    decl_id: impl Into<String>,
    value: ParamValue,
) -> Parameter {
    let mut parameter = parameter(label, decl_id, value, false);
    parameter
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    parameter
}

pub(super) fn external_formula_file_parameter() -> Parameter {
    parameter(
        "Formula File",
        FORMULA_EXTERNAL_FILE_DECL_ID,
        ParamValue::File(String::new()),
        false,
    )
}

pub(super) fn external_formula_source_parameter() -> Parameter {
    hidden_parameter(
        "External Formula Source",
        FORMULA_EXTERNAL_SOURCE_DECL_ID,
        ParamValue::Reference(NodeReference::empty()),
    )
}

pub(super) fn external_formula_delete_file_parameter() -> Parameter {
    hidden_parameter(
        "Delete External Formula File",
        FORMULA_EXTERNAL_DELETE_FILE_DECL_ID,
        ParamValue::Bool(false),
    )
}

pub(super) fn formula_copy_source_parameter() -> Parameter {
    hidden_parameter(
        "Formula Copy Source",
        FORMULA_COPY_SOURCE_DECL_ID,
        ParamValue::Reference(NodeReference::empty()),
    )
}

pub(super) fn external_formula_node() -> AlchemistFormulaDefinition {
    let mut formula = AlchemistFormulaDefinition::new();
    formula.node_data_mut().meta.label = "External Formula".to_owned();
    formula
        .node_data_mut()
        .meta
        .tags
        .push(FORMULA_EXTERNAL_FILE_TAG.to_owned());
    formula
}

pub(super) fn external_formula_tree() -> NodeTree {
    let formula = external_formula_node();
    NodeTree::new(formula)
        .with_child(NodeTree::new(external_formula_file_parameter()))
        .with_child(NodeTree::new(external_formula_source_parameter()))
        .with_child(NodeTree::new(external_formula_delete_file_parameter()))
}

/// Builds an external-file-linked formula tree pre-pointed at `path`, in the
/// same shape as manually adding one via the "External Formula" Add-menu
/// item (`external_formula_tree`) — used to auto-populate the library with
/// formulas found in the user's shared formulas folder. Its real label and
/// content are filled in by the existing external-file sync as soon as it
/// attaches (see `AlchemistFormulaDefinition::sync_external_formula_file`);
/// `label` is just the placeholder shown until then.
pub(crate) fn external_formula_tree_for_path(path: &Path, label: impl Into<String>) -> NodeTree {
    let mut formula = external_formula_node();
    formula.node_data_mut().meta.label = label.into();
    let file_param = parameter(
        "Formula File",
        FORMULA_EXTERNAL_FILE_DECL_ID,
        ParamValue::File(path.to_string_lossy().into_owned()),
        false,
    );
    NodeTree::new(formula)
        .with_child(NodeTree::new(file_param))
        .with_child(NodeTree::new(external_formula_source_parameter()))
        .with_child(NodeTree::new(external_formula_delete_file_parameter()))
}

pub(super) fn declared_folder(label: &str, decl_id: &str) -> Folder {
    let mut folder = Folder::new(label);
    folder.node_data_mut().meta.decl_id = DeclId(decl_id.to_owned());
    folder
}

pub(super) fn anode_position_parameter() -> Parameter {
    let mut position = parameter(
        "Position",
        "position",
        ParamValue::Vec2(0.0, 0.0),
        false,
    );
    position
        .node_data_mut()
        .meta
        .presentation
        .show_in_inspector_content = false;
    position
}

pub(super) fn anode_size_parameter() -> Parameter {
    let mut size = parameter("Size", "size", ParamValue::Vec2(13.0, 8.0), false);
    let meta = &mut size.node_data_mut().meta;
    meta.enabled = false;
    meta.can_be_disabled = true;
    meta.presentation.show_in_inspector_content = false;
    size
}

pub(super) fn default_config_for_declaration(
    declaration: &dyn chataigne_alchemist::ANodeDeclaration,
) -> chataigne_alchemist::ANodeConfig {
    let mut config = chataigne_alchemist::ANodeConfig::default();
    for field in declaration.config_fields() {
        config.set(field.id, field.default_value);
    }

    let mut instance = ANodeInstance::new(declaration.type_id(), declaration.label());
    instance.config = config.clone();
    for field in config_fields_for_instance(declaration, &instance) {
        config.set(field.id, field.default_value);
    }
    config
}

pub(super) fn config_value_parameter_for_field(
    field: &chataigne_alchemist::ANodeConfigFieldDecl,
    value_type: &ValueTypeId,
    default: RuntimeValue,
) -> Option<Parameter> {
    let value = runtime_value_to_param(&default).ok()?;
    let value_decl = config_decl_id(field.id.as_str());
    let mut config_parameter = parameter(&field.label, &value_decl, value, false);
    if !field.enum_options.is_empty() {
        let selected = match &field.default_value {
            RuntimeValue::String(value) => value.to_string(),
            _ => String::new(),
        };
        config_parameter.value = ParamValue::Enum(selected);
        config_parameter.default_value = ParamValue::Enum(match &field.default_value {
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
        config_parameter.constraints.policy = ParameterConstraintPolicy::Reject;
    }
    if field.editor.as_deref() == Some("optional_count") {
        config_parameter.node_data_mut().meta.can_be_disabled = true;
        config_parameter.node_data_mut().meta.enabled = false;
    }
    config_parameter.node_data_mut().meta.presentation.default_color =
        Some(value_type_color(value_type.as_str()));
    Some(config_parameter)
}

pub(super) fn config_field_trees_for_instance(
    declaration: &dyn chataigne_alchemist::ANodeDeclaration,
    instance: &ANodeInstance,
) -> Vec<NodeTree> {
    let value_types = value_types();
    let signature_ctx = SignatureCtx {
        value_types,
        properties: None,
    };
    let config_signature = declaration.signature(
        &signature_ctx,
        instance,
        &instance.type_bindings,
    );
    let mut trees = Vec::new();

    for mut field in config_fields_for_instance(declaration, instance) {
        if let Some(value) = instance.config.get(field.id.as_str()) {
            field.default_value = value.clone();
        }
        let value_decl = config_decl_id(field.id.as_str());
        if field.editor.as_deref() == Some("gradient") {
            let mut gradient = GradientNode::new_with_label(&field.label);
            gradient.node_data_mut().meta.decl_id = DeclId(value_decl);
            trees.push(NodeTree::new(gradient));
            continue;
        }
        if field.editor.as_deref() == Some("curve") {
            let mut curve = CurveNode::new_with_label(&field.label);
            curve.node_data_mut().meta.decl_id = DeclId(value_decl);
            trees.push(NodeTree::new(curve));
            continue;
        }

        if field.type_variable.is_some() {
            let type_options = field.resolved_type_options(&config_signature, value_types);
            let selected_type = match &field.default_value {
                RuntimeValue::String(value) => value.to_string(),
                value => runtime_value_type_id(value),
            };
            trees.push(NodeTree::new(value_type_parameter(
                &field.label,
                &value_decl,
                &selected_type,
                false,
                &type_options,
            )));
            continue;
        }

        let value_type = if field.editor.as_deref() == Some("runtime_value") {
            let type_decl = config_type_decl_id(field.id.as_str());
            let type_options = field.resolved_type_options(&config_signature, value_types);
            let selected_type = runtime_value_type_id(&field.default_value);
            let mut type_parameter = value_type_parameter(
                &format!("{} Type", field.label),
                &type_decl,
                &selected_type,
                false,
                &type_options,
            );
            let meta = &mut type_parameter.node_data_mut().meta;
            meta.can_be_disabled = false;
            meta.enabled = true;
            trees.push(NodeTree::new(type_parameter));
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
        if let Some(parameter) =
            config_value_parameter_for_field(&field, &value_type, default)
        {
            trees.push(NodeTree::new(parameter));
        }
    }

    trees
}

pub(super) fn anode_tree_for_declaration(
    type_id: &str,
    label: &str,
    category: &str,
    declaration: &dyn chataigne_alchemist::ANodeDeclaration,
) -> NodeTree {
    anode_tree_for_configured_instance(
        type_id,
        category,
        declaration,
        ANodeInstance::new(declaration.type_id(), label),
    )
    .expect("a default ANode instance has a materializable config")
}

fn anode_tree_for_configured_instance(
    type_id: &str,
    category: &str,
    declaration: &dyn chataigne_alchemist::ANodeDeclaration,
    mut instance: ANodeInstance,
) -> Option<NodeTree> {
    let mut config = default_config_for_declaration(declaration);
    for (field, value) in &instance.config.fields {
        if !config.fields.contains_key(field) {
            return None;
        }
        config.set(field.clone(), value.clone());
    }
    instance.config = config;
    let label = instance.label.as_str();

    let value_types = value_types();
    let signature_ctx = SignatureCtx {
        value_types,
        properties: None,
    };
    let signature =
        declaration.signature(&signature_ctx, &instance, &instance.type_bindings);
    let signature_bindings = local_signature_bindings(&signature, &instance);

    let mut config_tree = NodeTree::new(declared_folder("Config", "config"));
    for child in config_field_trees_for_instance(declaration, &instance) {
        config_tree.push_child(child);
    }

    let mut inputs_tree = NodeTree::new(declared_folder("Inputs", "inputs"));
    for input in signature.inputs {
        let value_type = constraint_value_type(&input.constraint, &signature_bindings);
        let default = instance
            .input_defaults
            .get(&input.id)
            .cloned()
            .or(input.default_value)
            .or_else(|| default_runtime_value(&value_type).ok())
            .unwrap_or(RuntimeValue::Float(0.0));
        inputs_tree.push_child(input_socket_tree(
            input.id.as_str(),
            &input.label,
            &value_type,
            &default,
        ));
    }

    let mut outputs_tree = NodeTree::new(declared_folder("Outputs", "outputs"));
    for output in signature.outputs {
        let value_type =
            constraint_value_type(&output.constraint, &signature_bindings);
        outputs_tree.push_child(output_socket_tree(
            output.id.as_str(),
            &output.label,
            &value_type,
        ));
    }

    let anode = AlchemistANode::for_type(type_id, label, category);
    Some(NodeTree::new(anode)
        .with_child(NodeTree::new(anode_position_parameter()))
        .with_child(NodeTree::new(anode_size_parameter()))
        .with_child(config_tree)
        .with_child(inputs_tree)
        .with_child(outputs_tree))
}
