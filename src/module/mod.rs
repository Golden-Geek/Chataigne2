use golden_core::{
    engine::Engine,
    node,
    node::{Node, NodeData, NodeId, NodeUserPermissions},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

pub const MODULE_ITEM_KIND: &str = "module";
pub const MODULE_VALUES_REFERENCE_FILTER_KEY: &str = "module_values_parameters";

pub fn enable_module_authoring(node_data: &mut NodeData) {
    node_data.meta.user_permissions = NodeUserPermissions::all();
}

pub fn initialize_module_root<N: Node + ?Sized>(node: &mut N, ctx: &mut ProcessCtx) {
    enable_module_authoring(node.node_data_mut());
    node.set_child_warning_depth(ctx, 4);
}

pub fn resolve_enclosing_module_root(snapshot: &ProcessTreeSnapshot, start: NodeId) -> Option<NodeId> {
    let mut current = Some(start);

    while let Some(node_id) = current {
        if snapshot.find_child(node_id, "infos").is_some()
            && snapshot.find_child(node_id, "parameters").is_some()
            && snapshot.find_child(node_id, "values").is_some()
        {
            return Some(node_id);
        }

        current = snapshot.node(node_id).and_then(|node| node.parent);
    }

    None
}

pub fn register_module_reference_filters<T: Node>(engine: &mut Engine<T>) {
    engine.register_reference_filter(
        MODULE_VALUES_REFERENCE_FILTER_KEY,
        |engine, _param_node, _root, candidate| candidate_is_module_values_parameter(engine, candidate),
    );
}

fn candidate_is_module_values_parameter<T: Node>(engine: &Engine<T>, candidate: NodeId) -> bool {
    let Some(candidate_node) = engine.nodes.get(candidate) else {
        return false;
    };
    if candidate_node.engine_param_snapshot().is_none() {
        return false;
    }

    let Some(mut current) = candidate_node.node_data().parent else {
        return false;
    };

    let mut has_values_ancestor = false;
    let mut has_module_ancestor = false;
    let mut has_module_manager_ancestor = false;
    loop {
        let Some(node) = engine.nodes.get(current) else {
            return false;
        };
        if node.node_data().meta.decl_id.0 == "values" {
            has_values_ancestor = true;
        }
        if node.user_item_kind() == MODULE_ITEM_KIND {
            has_module_ancestor = true;
        }
        if node.get_type() == "module_manager" {
            has_module_manager_ancestor = true;
        }
        let Some(parent) = node.node_data().parent else {
            break;
        };
        current = parent;
    }

    has_values_ancestor && has_module_ancestor && has_module_manager_ancestor
}

#[node("module_base", label = "Module")]
#[children(
    folder(infos, label = "Infos") {
        connected: bool = false (
            label = "Connected",
            description = "Whether the module is currently connected to its remote interface.",
            read_only = true
        );
        log_incoming: bool = false (
            label = "Log Incoming",
            description = "Whether incoming module traffic should be recorded in logs."
        );
        log_outgoing: bool = false (
            label = "Log Outgoing",
            description = "Whether outgoing module traffic should be recorded in logs."
        );
    }
    folder(parameters, label = "Parameters") {}
    folder(values, label = "Values") {}
)]
pub struct ModuleBase {}

#[node("module_base", from_struct, scriptable, contextualizable)]
impl Node for ModuleBase {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        initialize_module_root(self, ctx);
    }

    fn user_item_kind(&self) -> &str {
        MODULE_ITEM_KIND
    }
}
