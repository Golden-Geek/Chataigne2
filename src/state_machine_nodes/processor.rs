use std::collections::HashMap;

use golden_core::{
    node,
    node::{
        Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeUuid, NodeUserPermissions,
        UserContainerRules, UserCreatableItem,
    },
    parameter::{ParamValue, ParamValueProjection, project_param_value},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

// ─── Condition evaluation helpers ──────────────────────────────────────────

#[derive(Clone, Copy)]
enum CondEval {
    Known(bool),
    Disabled,
    Error,
}

/// Per-tick mutable state threaded through the recursive condition evaluation.
struct CondEvalCtx<'snap, 'state> {
    snapshot: &'snap ProcessTreeSnapshot,
    last_source_values: &'state mut HashMap<NodeId, ParamValue>,
    toggle_states: &'state mut HashMap<NodeId, bool>,
    prev_raw_results: &'state mut HashMap<NodeId, bool>,
    script_updates: Vec<(NodeId, &'static str, ParamValue)>,
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

/// Like `resolve_reference_child`, but also returns the projection stored on the reference.
fn resolve_reference_with_projection(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<(NodeId, Option<ParamValueProjection>)> {
    let param_id = snapshot.find_child_by_decl_id(parent, decl_id)?;
    let ParamValue::Reference(reference) = snapshot.node(param_id)?.param_value.as_ref()? else {
        return None;
    };
    let target_id = reference
        .cached_id()
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))?;
    Some((target_id, reference.projection()))
}

/// Non-parameter direct children of `container` — the user-added condition items.
fn condition_item_children(snapshot: &ProcessTreeSnapshot, container: NodeId) -> Vec<NodeId> {
    snapshot
        .child_ids(container)
        .into_iter()
        .filter(|&id| snapshot.node(id).map_or(false, |n| !n.is_parameter()))
        .collect()
}

fn apply_comparator_v2(
    value: &ParamValue,
    comparator: &str,
    reference: f64,
    reference_max: f64,
    reference_string: &str,
) -> bool {
    match value {
        ParamValue::Str(s) | ParamValue::Enum(s) => match comparator {
            "equal" => s == reference_string,
            "not_equal" => s != reference_string,
            "contains" => s.contains(reference_string),
            "starts_with" => s.starts_with(reference_string),
            "ends_with" => s.ends_with(reference_string),
            _ => false,
        },
        ParamValue::Bool(b) => match comparator {
            "is_true" => *b,
            "is_false" => !*b,
            "equal" => *b == (reference != 0.0),
            "not_equal" => *b != (reference != 0.0),
            _ => false,
        },
        _ => {
            let scalar = match value {
                ParamValue::Float(f) => *f,
                ParamValue::Int(i) => *i as f64,
                _ => return false,
            };
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
    }
}

fn valid_comparators_for_value(value: &ParamValue) -> &'static str {
    match value {
        ParamValue::Str(_) | ParamValue::Enum(_) => {
            "equal,not_equal,contains,starts_with,ends_with,value_changed"
        }
        ParamValue::Bool(_) => "is_true,is_false,equal,not_equal,value_changed",
        ParamValue::Float(_) | ParamValue::Int(_) => {
            "equal,not_equal,greater_than,greater_than_or_equal,less_than,less_than_or_equal,between,outside,is_true,is_false,value_changed"
        }
        _ => "value_changed",
    }
}

/// Applies toggle semantics to a raw boolean result.
/// In toggle mode, a false→true edge on `raw_result` flips the stored toggle state.
/// Without toggle mode, the raw result passes through unchanged.
fn apply_toggle(
    cond_id: NodeId,
    raw_result: bool,
    toggle_mode: bool,
    toggle_states: &mut HashMap<NodeId, bool>,
    prev_raw_results: &mut HashMap<NodeId, bool>,
) -> bool {
    let prev_raw = prev_raw_results.get(&cond_id).copied().unwrap_or(false);
    prev_raw_results.insert(cond_id, raw_result);

    if !toggle_mode {
        return raw_result;
    }

    if raw_result && !prev_raw {
        let new_state = !toggle_states.get(&cond_id).copied().unwrap_or(false);
        toggle_states.insert(cond_id, new_state);
    }

    toggle_states.get(&cond_id).copied().unwrap_or(false)
}

fn evaluate_input_value_condition(ctx: &mut CondEvalCtx<'_, '_>, cond_id: NodeId) -> Option<bool> {
    let (source_id, projection) = resolve_reference_with_projection(ctx.snapshot, cond_id, "source")?;
    let raw_source = ctx.snapshot.node(source_id)?.param_value.as_ref()?.clone();

    let effective_value = match projection {
        Some(proj) => project_param_value(&raw_source, proj).unwrap_or(raw_source),
        None => raw_source,
    };

    ctx.script_updates.push((
        cond_id,
        "valid_comparators",
        ParamValue::Str(valid_comparators_for_value(&effective_value).to_owned()),
    ));

    let comparator = read_enum_child(ctx.snapshot, cond_id, "comparator")
        .unwrap_or_else(|| "equal".into());

    let raw_result = if comparator == "value_changed" {
        // Clone previous to release the immutable borrow before any insert.
        let prev_val = ctx.last_source_values.get(&source_id).cloned();
        match prev_val {
            None => {
                ctx.last_source_values.insert(source_id, effective_value.clone());
                false
            }
            Some(ref prev) if prev == &effective_value => false,
            _ => {
                ctx.last_source_values.insert(source_id, effective_value.clone());
                true
            }
        }
    } else {
        let reference = read_float_child(ctx.snapshot, cond_id, "reference").unwrap_or(0.0);
        let reference_max = read_float_child(ctx.snapshot, cond_id, "reference_max").unwrap_or(1.0);
        let reference_string = read_str_child(ctx.snapshot, cond_id, "reference_string")
            .unwrap_or_default();
        apply_comparator_v2(&effective_value, &comparator, reference, reference_max, &reference_string)
    };

    let toggle_mode = read_bool_child(ctx.snapshot, cond_id, "toggle_mode").unwrap_or(false);
    let final_result = apply_toggle(
        cond_id,
        raw_result,
        toggle_mode,
        ctx.toggle_states,
        ctx.prev_raw_results,
    );

    ctx.script_updates.push((cond_id, "is_valid", ParamValue::Bool(final_result)));
    Some(final_result)
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
    ctx: &mut CondEvalCtx<'_, '_>,
    container_id: NodeId,
    operator: &str,
    count: f64,
    empty_policy: &str,
    disabled_policy: &str,
    error_policy: &str,
) -> bool {
    let children = condition_item_children(ctx.snapshot, container_id);
    if children.is_empty() {
        return empty_policy == "valid";
    }

    let mut results = Vec::with_capacity(children.len());
    for &child_id in &children {
        let enabled = ctx.snapshot.node(child_id).map_or(true, |n| n.enabled);
        if !enabled {
            results.push(CondEval::Disabled);
            continue;
        }
        match evaluate_single_condition(ctx, child_id) {
            Some(b) => results.push(CondEval::Known(b)),
            None => results.push(CondEval::Error),
        }
    }

    reduce_results(&results, operator, count, empty_policy, disabled_policy, error_policy)
}

fn evaluate_condition_group(ctx: &mut CondEvalCtx<'_, '_>, group_id: NodeId) -> Option<bool> {
    let operator = read_enum_child(ctx.snapshot, group_id, "operator")
        .unwrap_or_else(|| "all".into());
    let count = read_float_child(ctx.snapshot, group_id, "operator_count").unwrap_or(1.0);
    let empty_policy = read_enum_child(ctx.snapshot, group_id, "empty_policy")
        .unwrap_or_else(|| "invalid".into());
    let disabled_policy = read_enum_child(ctx.snapshot, group_id, "disabled_policy")
        .unwrap_or_else(|| "ignore".into());
    let error_policy = read_enum_child(ctx.snapshot, group_id, "error_policy")
        .unwrap_or_else(|| "treat_as_invalid".into());

    let raw_result =
        evaluate_container(ctx, group_id, &operator, count, &empty_policy, &disabled_policy, &error_policy);

    let toggle_mode = read_bool_child(ctx.snapshot, group_id, "toggle_mode").unwrap_or(false);
    let final_result = apply_toggle(
        group_id,
        raw_result,
        toggle_mode,
        ctx.toggle_states,
        ctx.prev_raw_results,
    );

    ctx.script_updates.push((group_id, "is_valid", ParamValue::Bool(final_result)));
    Some(final_result)
}

fn evaluate_single_condition(ctx: &mut CondEvalCtx<'_, '_>, cond_id: NodeId) -> Option<bool> {
    let node_type = ctx.snapshot.node(cond_id)?.node_type.clone();
    match node_type.as_str() {
        "sm_input_value_condition" => evaluate_input_value_condition(ctx, cond_id),
        "sm_condition_group" => evaluate_condition_group(ctx, cond_id),
        _ => None,
    }
}

fn evaluate_condition_manager(ctx: &mut CondEvalCtx<'_, '_>, manager_id: NodeId) -> bool {
    let operator = read_enum_child(ctx.snapshot, manager_id, "operator")
        .unwrap_or_else(|| "all".into());
    let count = read_float_child(ctx.snapshot, manager_id, "operator_count").unwrap_or(1.0);
    let empty_policy = read_enum_child(ctx.snapshot, manager_id, "empty_policy")
        .unwrap_or_else(|| "invalid".into());
    let disabled_policy = read_enum_child(ctx.snapshot, manager_id, "disabled_policy")
        .unwrap_or_else(|| "ignore".into());
    let error_policy = read_enum_child(ctx.snapshot, manager_id, "error_policy")
        .unwrap_or_else(|| "treat_as_invalid".into());

    let result =
        evaluate_container(ctx, manager_id, &operator, count, &empty_policy, &disabled_policy, &error_policy);

    ctx.script_updates.push((manager_id, "is_valid", ParamValue::Bool(result)));
    result
}

// ─── Consequence execution ─────────────────────────────────────────────────

/// Collects the `(target_id, value)` pairs for all enabled consequences in `manager_id`.
/// Ownership is fully extracted so the caller can drop the snapshot before calling ctx.
fn collect_consequence_actions(
    snapshot: &ProcessTreeSnapshot,
    manager_id: NodeId,
) -> Vec<(NodeId, ParamValue)> {
    snapshot
        .child_ids(manager_id)
        .into_iter()
        .filter(|&id| snapshot.node(id).map_or(false, |n| !n.is_parameter() && n.enabled))
        .filter_map(|child_id| {
            let node_type = snapshot.node(child_id)?.node_type.clone();
            match node_type.as_str() {
                "sm_set_value_consequence" => {
                    let target = resolve_reference_child(snapshot, child_id, "target")?;
                    let value = read_float_child(snapshot, child_id, "value").unwrap_or(0.0);
                    Some(vec![(target, ParamValue::Float(value))])
                }
                "sm_send_command_consequence" => {
                    let target = resolve_reference_child(snapshot, child_id, "target")?;
                    Some(vec![(target, ParamValue::Trigger())])
                }
                _ => None,
            }
        })
        .flatten()
        .collect()
}

// ─── End of consequence execution ──────────────────────────────────────────

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
    #[state(default = HashMap::new())]
    last_source_values: HashMap<NodeId, ParamValue>,
    #[state(default = HashMap::new())]
    toggle_states: HashMap<NodeId, bool>,
    #[state(default = HashMap::new())]
    prev_raw_results: HashMap<NodeId, bool>,
}

#[node("state_processor", from_struct)]
impl Node for StateProcessor {
    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        let self_id = self.id();
        let prev_valid = self.condition_valid;

        let (new_valid, actions, script_updates) = {
            let Some(snapshot) = ctx.tree_snapshot() else { return; };

            let mut eval = CondEvalCtx {
                snapshot,
                last_source_values: &mut self.last_source_values,
                toggle_states: &mut self.toggle_states,
                prev_raw_results: &mut self.prev_raw_results,
                script_updates: Vec::new(),
            };

            let conditions_mgr = eval.snapshot.find_child_by_decl_id(self_id, "conditions");
            let new_valid = conditions_mgr
                .map_or(false, |mgr_id| evaluate_condition_manager(&mut eval, mgr_id));

            let fire_decl_id = match (prev_valid, new_valid) {
                (false, true) => Some("true_consequences"),
                (true, false) => Some("false_consequences"),
                _ => None,
            };

            let actions = if let Some(decl) = fire_decl_id {
                eval.snapshot
                    .find_child_by_decl_id(self_id, decl)
                    .map(|mgr_id| collect_consequence_actions(eval.snapshot, mgr_id))
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            (new_valid, actions, eval.script_updates)
        };

        self.condition_valid = new_valid;
        ctx.set_node_script_property(self_id, "is_valid", ParamValue::Bool(new_valid));
        for (node_id, key, value) in script_updates {
            ctx.set_node_script_property(node_id, key, value);
        }
        for (target, value) in actions {
            ctx.set_param(target, value);
        }
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
