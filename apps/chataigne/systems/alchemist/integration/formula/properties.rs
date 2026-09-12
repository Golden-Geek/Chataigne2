use super::*;

pub(crate) const PROPERTY_MANAGER_ROLES: [(&str, &str); 4] = [
    ("condition", "Conditions"),
    ("filter", "Filters"),
    ("input", "Inputs"),
    ("output", "Outputs"),
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

pub(super) fn locked_properties_permissions() -> NodeUserPermissions {
    let mut permissions = NodeUserPermissions::all();
    permissions.can_remove_and_duplicate = false;
    permissions.can_edit_name = false;
    permissions
}

pub(super) fn property_default(property_type: &str) -> Option<ParamValue> {
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

pub(super) fn property_value_type(property_type: &str) -> &'static str {
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

pub(super) fn property_parameter(property_type: &str) -> Option<Parameter> {
    let default = property_default(property_type)?;
    let color = parameter_value_color(&default);
    let mut value = parameter("Default value", "value", default, false);
    value.node_data_mut().meta.presentation.default_color = Some(color);
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

pub(super) fn formula_property_creatable_items() -> Vec<UserCreatableItem> {
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

pub(super) fn formula_properties_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[
        PROPERTY_ITEM_KIND,
        PROPERTY_MANAGER_ITEM_KIND,
        PROPERTY_FOLDER_ITEM_KIND,
    ])
}

pub(super) fn formula_properties_accepts(
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

pub(super) fn create_formula_property_item(node_type: &str) -> Option<Box<dyn Node>> {
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

pub(super) fn properties_tree() -> NodeTree {
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
            "filter",
            "input",
            "output"
        ],
        read_only = true,
        show_in_inspector_content = false
    );
    exposed: bool = true (
        label = "Exposed"
    );
)]
pub struct AlchemistPropertyManager {}

#[node("alchemist_property_manager", from_struct)]
impl Node for AlchemistPropertyManager {
    fn user_item_kind(&self) -> &str {
        PROPERTY_MANAGER_ITEM_KIND
    }

    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
        self.node_data_mut().meta.can_be_disabled = false;
        self.reconcile_role(ctx);
    }

    fn on_node_ready(
        &mut self,
        ctx: &mut ProcessCtx,
        _context: NodeCreationContext,
    ) {
        self.reconcile_role(ctx);
    }

    fn on_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        param: NodeId,
        _old_value: ParamValue,
    ) {
        if param == self.role.id() {
            self.reconcile_role(ctx);
        }
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
        set_tag(
            &mut manager.node_data_mut().meta.tags,
            PROPERTY_MANAGER_ROLE_TAG_PREFIX,
            role,
        );
        manager.node_data_mut().meta.presentation.default_color = manager_role_color(role);
        manager
    }

    fn reconcile_role(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };

        let tagged_role = property_manager_role_from_tags(&self.node_data().meta.tags);
        let child_role = child_string(&snapshot, self.id(), "role")
            .filter(|role| manager_role_anode_type(role).is_some());
        let label_role =
            property_manager_role_from_label(&self.node_data().meta.label).map(ToOwned::to_owned);
        let Some(role) = tagged_role
            .clone()
            .or_else(|| {
                let child_role = child_role.clone()?;
                if child_role == "condition" {
                    label_role
                        .clone()
                        .filter(|role| role != "condition")
                        .or(Some(child_role))
                } else {
                    Some(child_role)
                }
            })
            .or_else(|| label_role.clone())
            .or_else(|| {
                let fallback = self.role.get_ref();
                (fallback.as_str() != "condition").then(|| fallback.as_str().to_owned())
            })
        else {
            return;
        };

        if tagged_role.as_deref() != Some(role.as_str()) {
            set_tag(
                &mut self.node_data_mut().meta.tags,
                PROPERTY_MANAGER_ROLE_TAG_PREFIX,
                &role,
            );
        }
        if self.node_data().meta.presentation.default_color.is_none() {
            self.node_data_mut().meta.presentation.default_color = manager_role_color(&role);
        }

        if let Some(role_param) = snapshot.find_child_by_decl_id(self.id(), "role") {
            let desired = ParamValue::Enum(role);
            ctx.call_node_mutation(role_param, |node, _ctx| {
                let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>()
                else {
                    return Err("expected an Alchemist manager role parameter".into());
                };
                parameter.persist_read_only_value = true;
                Ok(())
            });
            if snapshot
                .node(role_param)
                .and_then(|node| node.param_value.as_ref())
                != Some(&desired)
            {
                ctx.edits.push(Edit::SetParam {
                    node: role_param,
                    value: desired,
                    behaviour: ParameterEventBehaviour::Coalesce,
                });
            }
        }
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
        property.node_data_mut().meta.presentation.default_color =
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

        if self.node_data().meta.presentation.default_color.is_none() {
            self.node_data_mut().meta.presentation.default_color =
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
            let color = value.node_data().meta.presentation.default_color;
            ctx.call_node_mutation(existing, move |node, _ctx| {
                node.node_data_mut().meta.presentation.default_color = color;
                Ok(())
            });
        } else {
            ctx.add_child(self.id(), value, None);
        }
    }
}

pub(super) fn formula_surface_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
    anodes: &mut [ANodeInstance],
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
                snapshot, child, anodes,
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
    Ok(FormulaSurface {
        sections,
        managed_regions: formula_managed_regions_from_snapshot(snapshot, formula_node)?,
    })
}

pub(super) fn formula_managed_regions_from_snapshot(
    snapshot: &ProcessTreeSnapshot,
    formula_node: NodeId,
) -> Result<Vec<ManagedRegionDefinition>, String> {
    if snapshot.child_ids(formula_node).into_iter().any(|child| {
        snapshot
            .node(child)
            .is_some_and(|node| node.node_type == ANODE_NODE_TYPE)
    }) {
        return Ok(Vec::new());
    }
    let Some(ParamValue::Str(raw)) =
        child_param(snapshot, formula_node, FORMULA_MANAGED_REGIONS_JSON_DECL_ID)
    else {
        return Ok(Vec::new());
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(raw).map_err(|error| {
        format!("Formula managed region metadata is invalid: {error}")
    })
}

pub(super) fn surface_item_from_property(
    snapshot: &ProcessTreeSnapshot,
    property: NodeId,
    anodes: &mut [ANodeInstance],
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
        bindings: property_bindings(anodes, &id, &runtime_value),
    })
}

pub(super) fn property_bindings(
    anodes: &mut [ANodeInstance],
    property_id: &str,
    default_value: &RuntimeValue,
) -> Vec<ANodeFieldPath> {
    anodes
        .iter_mut()
        .filter_map(|node| {
            let matches_property = node.type_id.as_str() == PROPERTY_ANODE_TYPE
                && node.config.get("property_id").is_some_and(|value| {
                    matches!(
                        value,
                        RuntimeValue::Ref(value) if value.stable_id.as_ref() == property_id
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
