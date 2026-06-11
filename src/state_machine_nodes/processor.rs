use golden_core::{
    node,
    node::{
        Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeReference, NodeUuid,
        NodeUserPermissions, UserContainerRules, UserCreatableItem,
    },
    parameter::{ParamValue, ReferenceTargetKind},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

const FORMULA_LIBRARY_NODE_TYPE: &str = "alchemist_formula_library";
const FORMULA_NODE_TYPE: &str = "alchemist_formula";
pub(crate) const PROCESSOR_ITEM_KIND: &str = "state_processor";
pub(crate) const PROCESSOR_FOLDER_ITEM_KIND: &str = "state_processor_folder";
pub(crate) const PROCESSOR_FOLDER_NODE_TYPE: &str = "state_processor_folder";

fn processor_container_rules() -> UserContainerRules {
    UserContainerRules::new(&[PROCESSOR_ITEM_KIND, PROCESSOR_FOLDER_ITEM_KIND])
}

fn processor_container_accepts(item_type: &str, item_kind: &str) -> bool {
    match item_kind {
        PROCESSOR_ITEM_KIND => {
            item_type == StateProcessor::NODE_TYPE || item_type.starts_with("state_processor:")
        }
        PROCESSOR_FOLDER_ITEM_KIND => item_type == PROCESSOR_FOLDER_NODE_TYPE,
        _ => false,
    }
}

fn initialize_processor_item(node: &mut dyn Node) {
    node.node_data_mut().meta.user_permissions = NodeUserPermissions::all();
}

fn build_formula_items(snapshot: &ProcessTreeSnapshot, library: NodeId) -> Vec<UserCreatableItem> {
    snapshot
        .child_ids(library)
        .into_iter()
        .filter_map(|formula_id| {
            let formula = snapshot.node(formula_id)?;
            (formula.node_type == FORMULA_NODE_TYPE).then(|| {
                UserCreatableItem::new(
                    format!("state_processor:{}", formula.uuid.0),
                    PROCESSOR_ITEM_KIND,
                    &formula.label,
                )
            })
        })
        .collect()
}

fn find_formula_library(snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
    snapshot
        .child_ids(snapshot.root())
        .into_iter()
        .find(|node| {
            snapshot
                .node(*node)
                .is_some_and(|snapshot_node| snapshot_node.node_type == FORMULA_LIBRARY_NODE_TYPE)
        })
}

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
        let Some(snapshot) = ctx.tree_snapshot() else {
            return;
        };
        let root = snapshot.root();
        let library = find_formula_library(snapshot);
        ctx.add_event_listener_subtree(self.id(), root, 1);
        if let Some(library) = library {
            ctx.add_event_listener_subtree(self.id(), library, 2);
        }
        self.refresh_formula_items(ctx);
    }

    fn on_node_created(&mut self, ctx: &mut ProcessCtx, node: NodeId) {
        let is_library = ctx.tree_snapshot().is_some_and(|snapshot| {
            snapshot
                .node(node)
                .is_some_and(|snapshot_node| snapshot_node.node_type == FORMULA_LIBRARY_NODE_TYPE)
        });
        if is_library {
            ctx.add_event_listener_subtree(self.id(), node, 2);
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
    fn refresh_formula_items(&mut self, ctx: &ProcessCtx) {
        self.formula_items = ctx
            .tree_snapshot()
            .and_then(|snapshot| {
                find_formula_library(snapshot)
                    .map(|library| build_formula_items(snapshot, library))
            })
            .unwrap_or_default();
    }
}

fn create_processor_for_formula_type(node_type: &str) -> Option<Box<dyn Node>> {
    let formula_uuid = node_type
        .strip_prefix("state_processor:")?
        .parse::<uuid::Uuid>()
        .ok()
        .map(NodeUuid)?;
    let mut processor = StateProcessor::new();
    processor.formula.apply_runtime_value(&ParamValue::Reference(
        NodeReference::new(formula_uuid),
    ));
    Some(Box::new(processor))
}

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
        initialize_processor_item(self);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("state_processor", label = "Processor")]
#[children(
    formula: NodeReference (
        label = "Formula",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![FORMULA_NODE_TYPE.to_owned()],
        reference_allow_projections = false
    );
)]
pub struct StateProcessor {}

#[node("state_processor", from_struct)]
impl Node for StateProcessor {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_processor_item(self);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[cfg(test)]
mod processor_tests;
