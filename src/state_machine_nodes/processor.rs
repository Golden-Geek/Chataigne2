use golden_core::{
    node,
    node::{
        Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeUuid, NodeUserPermissions,
        UserContainerRules, UserCreatableItem,
    },
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

// ─── Condition evaluation helpers ──────────────────────────────────────────

#[derive(Clone, Copy)]
enum CondEval {
    Known(bool),
    Disabled,
    Error,
}

fn read_enum_child(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<String> {
    let param_id = snapshot.find_child_by_decl_id(parent, decl_id)?;
    match snapshot.node(param_id)?.param_value.as_ref()? {
        ParamValue::Enum(s) | ParamValue::Str(s) => Some(s.clone()),
        _ => None,
    }
}

fn read_float_child(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<f64> {
    let param_id = snapshot.find_child_by_decl_id(parent, decl_id)?;
    match snapshot.node(param_id)?.param_value.as_ref()? {
        ParamValue::Float(f) => Some(*f),
        ParamValue::Int(i) => Some(*i as f64),
        _ => None,
    }
}

fn read_bool_child(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<bool> {
    let param_id = snapshot.find_child_by_decl_id(parent, decl_id)?;
    match snapshot.node(param_id)?.param_value.as_ref()? {
        ParamValue::Bool(b) => Some(*b),
        _ => None,
    }
}

fn read_str_child(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<String> {
    let param_id = snapshot.find_child_by_decl_id(parent, decl_id)?;
    match snapshot.node(param_id)?.param_value.as_ref()? {
        ParamValue::Str(s) | ParamValue::Enum(s) => Some(s.clone()),
        _ => None,
    }
}

fn resolve_reference_child(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<NodeId> {
    let param_id = snapshot.find_child_by_decl_id(parent, decl_id)?;
    let ParamValue::Reference(reference) = snapshot.node(param_id)?.param_value.as_ref()? else {
        return None;
    };
    reference
        .cached_id()
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))
}

/// Non-parameter direct children of `container` — the user-added condition items.
fn condition_item_children(snapshot: &ProcessTreeSnapshot, container: NodeId) -> Vec<NodeId> {
    snapshot
        .child_ids(container)
        .into_iter()
        .filter(|&id| snapshot.node(id).map_or(false, |n| !n.is_parameter()))
        .collect()
}

fn project_to_scalar(value: &ParamValue, projection: &str) -> Option<f64> {
    match projection {
        "auto" | "number" => match value {
            ParamValue::Float(f) => Some(*f),
            ParamValue::Int(i) => Some(*i as f64),
            ParamValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            ParamValue::Vec2(x, _) => Some(*x),
            ParamValue::Vec3(x, _, _) => Some(*x),
            _ => None,
        },
        "bool" => match value {
            ParamValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            ParamValue::Float(f) => Some(if *f != 0.0 { 1.0 } else { 0.0 }),
            ParamValue::Int(i) => Some(if *i != 0 { 1.0 } else { 0.0 }),
            _ => None,
        },
        "vec2_x" => {
            if let ParamValue::Vec2(x, _) = value { Some(*x) } else { None }
        }
        "vec2_y" => {
            if let ParamValue::Vec2(_, y) = value { Some(*y) } else { None }
        }
        "vec2_magnitude" => {
            if let ParamValue::Vec2(x, y) = value { Some((x * x + y * y).sqrt()) } else { None }
        }
        "vec3_x" => {
            if let ParamValue::Vec3(x, _, _) = value { Some(*x) } else { None }
        }
        "vec3_y" => {
            if let ParamValue::Vec3(_, y, _) = value { Some(*y) } else { None }
        }
        "vec3_z" => {
            if let ParamValue::Vec3(_, _, z) = value { Some(*z) } else { None }
        }
        "vec3_magnitude" => {
            if let ParamValue::Vec3(x, y, z) = value {
                Some((x * x + y * y + z * z).sqrt())
            } else {
                None
            }
        }
        "color_red" => {
            if let ParamValue::Color(r, _, _, _) = value { Some(*r) } else { None }
        }
        "color_green" => {
            if let ParamValue::Color(_, g, _, _) = value { Some(*g) } else { None }
        }
        "color_blue" => {
            if let ParamValue::Color(_, _, b, _) = value { Some(*b) } else { None }
        }
        "color_alpha" => {
            if let ParamValue::Color(_, _, _, a) = value { Some(*a) } else { None }
        }
        "color_luminance" => {
            if let ParamValue::Color(r, g, b, _) = value {
                Some(0.2126 * r + 0.7152 * g + 0.0722 * b)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn project_to_string(value: &ParamValue) -> String {
    match value {
        ParamValue::Str(s) | ParamValue::Enum(s) => s.clone(),
        ParamValue::Float(f) => f.to_string(),
        ParamValue::Int(i) => i.to_string(),
        ParamValue::Bool(b) => b.to_string(),
        _ => String::new(),
    }
}

fn apply_comparator(
    source_value: &ParamValue,
    projection: &str,
    comparator: &str,
    reference: f64,
    reference_max: f64,
    reference_string: &str,
) -> bool {
    // String-mode: explicit string comparators or explicit string projection
    if matches!(comparator, "contains" | "starts_with" | "ends_with")
        || matches!(projection, "string_value" | "enum_value")
    {
        let s = project_to_string(source_value);
        return match comparator {
            "equal" => s == reference_string,
            "not_equal" => s != reference_string,
            "contains" => s.contains(reference_string),
            "starts_with" => s.starts_with(reference_string),
            "ends_with" => s.ends_with(reference_string),
            _ => false,
        };
    }
    // Numeric path: project_to_scalar handles bool→float via "auto"
    let Some(scalar) = project_to_scalar(source_value, projection) else { return false; };
    match comparator {
        "equal" => (scalar - reference).abs() < 1e-9,
        "not_equal" => (scalar - reference).abs() >= 1e-9,
        "greater_than" => scalar > reference,
        "greater_than_or_equal" => scalar >= reference,
        "less_than" => scalar < reference,
        "less_than_or_equal" => scalar <= reference,
        "between" => scalar >= reference && scalar <= reference_max,
        "outside" => scalar < reference || scalar > reference_max,
        "is_true" => scalar != 0.0,
        "is_false" => scalar == 0.0,
        _ => false,
    }
}

fn evaluate_input_value_condition(snapshot: &ProcessTreeSnapshot, cond_id: NodeId) -> Option<bool> {
    let source_id = resolve_reference_child(snapshot, cond_id, "source")?;
    let source_value = snapshot.node(source_id)?.param_value.as_ref()?.clone();

    let projection = read_enum_child(snapshot, cond_id, "projection")
        .unwrap_or_else(|| "auto".into());
    let comparator = read_enum_child(snapshot, cond_id, "comparator")
        .unwrap_or_else(|| "equal".into());
    let reference = read_float_child(snapshot, cond_id, "reference").unwrap_or(0.0);
    let reference_max = read_float_child(snapshot, cond_id, "reference_max").unwrap_or(1.0);
    let reference_string = read_str_child(snapshot, cond_id, "reference_string")
        .unwrap_or_default();
    let invert = read_bool_child(snapshot, cond_id, "invert").unwrap_or(false);

    let result = apply_comparator(
        &source_value,
        &projection,
        &comparator,
        reference,
        reference_max,
        &reference_string,
    );
    Some(if invert { !result } else { result })
}

fn reduce_results(
    results: &[CondEval],
    operator: &str,
    count: f64,
    empty_policy: &str,
    disabled_policy: &str,
    error_policy: &str,
) -> bool {
    let known: Vec<bool> = results
        .iter()
        .filter_map(|r| match r {
            CondEval::Known(b) => Some(*b),
            CondEval::Disabled => match disabled_policy {
                "treat_as_invalid" => Some(false),
                "treat_as_valid" => Some(true),
                _ => None,
            },
            CondEval::Error => match error_policy {
                "treat_as_invalid" => Some(false),
                _ => None,
            },
        })
        .collect();

    if known.is_empty() {
        return empty_policy == "valid";
    }

    let true_count = known.iter().filter(|&&b| b).count();
    match operator {
        "all" => true_count == known.len(),
        "any" => true_count > 0,
        "none" => true_count == 0,
        "at_least" => true_count >= count as usize,
        "exactly" => true_count == count as usize,
        _ => false,
    }
}

fn evaluate_container(
    snapshot: &ProcessTreeSnapshot,
    container_id: NodeId,
    operator: &str,
    count: f64,
    empty_policy: &str,
    disabled_policy: &str,
    error_policy: &str,
) -> bool {
    let children = condition_item_children(snapshot, container_id);
    if children.is_empty() {
        return empty_policy == "valid";
    }

    let results: Vec<CondEval> = children
        .iter()
        .map(|&child_id| {
            let enabled = snapshot.node(child_id).map_or(true, |n| n.enabled);
            if !enabled {
                return CondEval::Disabled;
            }
            match evaluate_single_condition(snapshot, child_id) {
                Some(b) => CondEval::Known(b),
                None => CondEval::Error,
            }
        })
        .collect();

    reduce_results(&results, operator, count, empty_policy, disabled_policy, error_policy)
}

fn evaluate_condition_group(snapshot: &ProcessTreeSnapshot, group_id: NodeId) -> Option<bool> {
    let operator = read_enum_child(snapshot, group_id, "operator")
        .unwrap_or_else(|| "all".into());
    let count = read_float_child(snapshot, group_id, "operator_count").unwrap_or(1.0);
    let empty_policy = read_enum_child(snapshot, group_id, "empty_policy")
        .unwrap_or_else(|| "invalid".into());
    let disabled_policy = read_enum_child(snapshot, group_id, "disabled_policy")
        .unwrap_or_else(|| "ignore".into());
    let error_policy = read_enum_child(snapshot, group_id, "error_policy")
        .unwrap_or_else(|| "treat_as_invalid".into());
    let invert = read_bool_child(snapshot, group_id, "invert").unwrap_or(false);

    let result = evaluate_container(
        snapshot,
        group_id,
        &operator,
        count,
        &empty_policy,
        &disabled_policy,
        &error_policy,
    );
    Some(if invert { !result } else { result })
}

fn evaluate_single_condition(snapshot: &ProcessTreeSnapshot, cond_id: NodeId) -> Option<bool> {
    let node_type = snapshot.node(cond_id)?.node_type.clone();
    match node_type.as_str() {
        "sm_input_value_condition" => evaluate_input_value_condition(snapshot, cond_id),
        "sm_condition_group" => evaluate_condition_group(snapshot, cond_id),
        _ => None,
    }
}

fn evaluate_condition_manager(snapshot: &ProcessTreeSnapshot, manager_id: NodeId) -> bool {
    let operator = read_enum_child(snapshot, manager_id, "operator")
        .unwrap_or_else(|| "all".into());
    let count = read_float_child(snapshot, manager_id, "operator_count").unwrap_or(1.0);
    let empty_policy = read_enum_child(snapshot, manager_id, "empty_policy")
        .unwrap_or_else(|| "invalid".into());
    let disabled_policy = read_enum_child(snapshot, manager_id, "disabled_policy")
        .unwrap_or_else(|| "ignore".into());
    let error_policy = read_enum_child(snapshot, manager_id, "error_policy")
        .unwrap_or_else(|| "treat_as_invalid".into());

    evaluate_container(
        snapshot,
        manager_id,
        &operator,
        count,
        &empty_policy,
        &disabled_policy,
        &error_policy,
    )
}

// ─── End of condition evaluation helpers ───────────────────────────────────

pub(crate) const PROCESSOR_ITEM_KIND: &str = "state_processor";
pub(crate) const PROCESSOR_FOLDER_ITEM_KIND: &str = "state_processor_folder";
pub(crate) const PROCESSOR_FOLDER_NODE_TYPE: &str = "state_processor_folder";

const FORMULA_LIBRARY_NODE_TYPE: &str = "alchemist_formula_library";

fn processor_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[PROCESSOR_ITEM_KIND, PROCESSOR_FOLDER_ITEM_KIND])
}

fn processor_container_accepts(item_type: &str, item_kind: &str) -> bool {
    if item_kind == PROCESSOR_ITEM_KIND {
        // Accept both the raw node type and the "state_processor:{UUID}" encoding
        // used by UserCreatableItem to carry the formula reference.
        item_type == StateProcessor::NODE_TYPE || item_type.starts_with("state_processor:")
    } else {
        item_type == PROCESSOR_FOLDER_NODE_TYPE && item_kind == PROCESSOR_FOLDER_ITEM_KIND
    }
}

fn initialize_processor_item(node: &mut dyn Node) {
    node.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
}

/// Builds the formula item list from the snapshot rooted at `lib_id`.
/// Each formula node becomes a `UserCreatableItem` whose `node_type` encodes the
/// formula UUID as `"state_processor:{uuid}"`, carrying the UUID through to
/// `create_user_item` without requiring extra state.
fn build_formula_items(snapshot: &ProcessTreeSnapshot, lib_id: NodeId) -> Vec<UserCreatableItem> {
    snapshot
        .child_ids(lib_id)
        .into_iter()
        .filter_map(|formula_id| {
            let n = snapshot.node(formula_id)?;
            let node_type = format!("state_processor:{}", n.uuid.0);
            Some(UserCreatableItem::new(node_type, PROCESSOR_ITEM_KIND, &n.label))
        })
        .collect()
}

/// Finds the FormulaLibrary node id in the snapshot, if present.
fn find_formula_library(snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
    snapshot
        .child_ids(snapshot.root())
        .into_iter()
        .find(|&id| {
            snapshot
                .node(id)
                .map_or(false, |n| n.node_type == FORMULA_LIBRARY_NODE_TYPE)
        })
}

/// Processor manager. Maintains a reactive cache of formula items sourced from the
/// FormulaLibrary node. Rebuilds the cache whenever FormulaLibrary or its children
/// change, using event subscriptions to FormulaLibrary's subtree.
#[node(
    "state_processor_manager",
    label = "Processors",
    presentation = golden_core::node::PresentationHint {
        show_in_nested_inspector: false,
        ..Default::default()
    }
)]
pub struct StateProcessorManager {
    #[state(default = Vec::new())]
    formula_items: Vec<UserCreatableItem>,
}

#[node("state_processor_manager", from_struct)]
impl Node for StateProcessorManager {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(processor_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        processor_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        let mut items = self.formula_items.clone();
        items.push(UserCreatableItem::new(
            PROCESSOR_FOLDER_NODE_TYPE,
            PROCESSOR_FOLDER_ITEM_KIND,
            "Folder",
        ));
        items
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if node_type == PROCESSOR_FOLDER_NODE_TYPE {
            return Some(Box::new(StateProcessorFolder::new()));
        }
        create_processor_for_formula_type(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        let mut permissions = NodeUserPermissions::all();
        permissions.can_remove_and_duplicate = false;
        self.node_data_mut().meta.user_permissions = permissions;
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let self_id = self.id();

        // Find FormulaLibrary and build initial cache; also grab root for subscription.
        let (root_id, formula_lib_id) = {
            let Some(snapshot) = ctx.tree_snapshot() else {
                return;
            };
            let root_id = snapshot.root();
            let lib_id = find_formula_library(snapshot);
            (root_id, lib_id)
        };

        // Subscribe to root at depth=1 so on_node_created fires when FormulaLibrary
        // is added later (e.g. when StateMachineManager initialises before it).
        ctx.add_event_listener_subtree(self_id, root_id, 1);

        if let Some(lib_id) = formula_lib_id {
            ctx.add_event_listener_subtree(self_id, lib_id, 2);
            if let Some(snapshot) = ctx.tree_snapshot() {
                self.formula_items = build_formula_items(snapshot, lib_id);
            }
        }
    }

    fn on_node_created(&mut self, ctx: &mut ProcessCtx, node: NodeId) {
        let is_formula_library = {
            let Some(snapshot) = ctx.tree_snapshot() else {
                return;
            };
            snapshot
                .node(node)
                .map_or(false, |n| n.node_type == FORMULA_LIBRARY_NODE_TYPE)
        };

        if is_formula_library {
            let self_id = self.id();
            ctx.add_event_listener_subtree(self_id, node, 2);
        }

        self.refresh_formula_items(ctx);
    }

    fn on_node_deleted(&mut self, ctx: &mut ProcessCtx, _node: NodeId) {
        self.refresh_formula_items(ctx);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, _node: NodeId, _patch: NodeMetaPatch) {
        self.refresh_formula_items(ctx);
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.refresh_formula_items(ctx);
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.refresh_formula_items(ctx);
    }
}

impl StateProcessorManager {
    fn refresh_formula_items(&mut self, ctx: &mut ProcessCtx) {
        let items = {
            let Some(snapshot) = ctx.tree_snapshot() else {
                return;
            };
            find_formula_library(snapshot)
                .map(|lib_id| build_formula_items(snapshot, lib_id))
                .unwrap_or_default()
        };
        self.formula_items = items;
    }
}

/// Parses `"state_processor:{UUID}"` and returns a pre-configured `StateProcessor`.
fn create_processor_for_formula_type(node_type: &str) -> Option<Box<dyn Node>> {
    let uuid_str = node_type.strip_prefix("state_processor:")?;
    let uuid = uuid_str.parse::<uuid::Uuid>().ok().map(NodeUuid)?;
    let mut processor = StateProcessor::new();
    processor
        .formula_uuid
        .apply_runtime_value(&ParamValue::Str(uuid.0.to_string()));
    Some(Box::new(processor))
}

/// Folder node for grouping processors. Accepts any `state_processor` item.
#[node("state_processor_folder", label = "Folder")]
pub struct StateProcessorFolder {}

#[node("state_processor_folder", from_struct)]
impl Node for StateProcessorFolder {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(processor_container_rules())
    }

    fn user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        processor_container_accepts(item_type, item_kind)
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        // Folders can't provide the reactive formula list on their own, so they
        // defer to a Folder item only (processors are added via the parent manager).
        vec![UserCreatableItem::new(
            PROCESSOR_FOLDER_NODE_TYPE,
            PROCESSOR_FOLDER_ITEM_KIND,
            "Folder",
        )]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if node_type == PROCESSOR_FOLDER_NODE_TYPE {
            return Some(Box::new(StateProcessorFolder::new()));
        }
        create_processor_for_formula_type(node_type)
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

/// A single processor node. `formula_uuid` identifies which formula this
/// processor instantiates. On fresh creation, the formula's ANode children
/// are iterated and each one is responsible for creating its manager child.
#[node("state_processor", label = "Processor")]
#[children(
    formula_uuid: String = String::new() (
        label = "Formula UUID",
        show_in_inspector_content = false
    );
)]
pub struct StateProcessor {
    #[state(default = false)]
    condition_valid: bool,
}

#[node("state_processor", from_struct)]
impl Node for StateProcessor {
    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        let self_id = self.id();
        let new_valid = {
            let Some(snapshot) = ctx.tree_snapshot() else { return; };
            snapshot
                .find_child_by_decl_id(self_id, "conditions")
                .map_or(false, |mgr_id| evaluate_condition_manager(snapshot, mgr_id))
        };
        self.condition_valid = new_valid;
    }

    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        if context != NodeCreationContext::Fresh {
            return;
        }

        let uuid_str = self.formula_uuid.get_ref().clone();
        let Some(uuid) = uuid_str
            .parse::<uuid::Uuid>()
            .ok()
            .map(NodeUuid)
            .filter(|u| !u.is_nil())
        else {
            return;
        };

        // Collect ANode metadata before borrowing ctx mutably.
        let anode_data: Vec<(String, String, String)> = {
            let Some(snapshot) = ctx.tree_snapshot() else {
                return;
            };
            let Some(formula_id) = snapshot.node_id_by_uuid(uuid) else {
                return;
            };
            snapshot
                .child_ids(formula_id)
                .into_iter()
                .filter_map(|id| {
                    let n = snapshot.node(id)?;
                    Some((n.node_type.clone(), n.label.clone(), n.decl_id.clone()))
                })
                .collect()
        };

        let processor_id = self.id();
        for (anode_type, anode_label, anode_decl_id) in anode_data {
            crate::app::state_machine_nodes_formula::instantiate_anode_for_processor(
                &anode_type,
                &anode_label,
                &anode_decl_id,
                processor_id,
                ctx,
            );
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod processor_tests;
