use golden_core::{
    node,
    node::{
        Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeUuid, NodeUserPermissions,
        UserContainerRules, UserCreatableItem,
    },
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

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
pub struct StateProcessor {}

#[node("state_processor", from_struct)]
impl Node for StateProcessor {
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
