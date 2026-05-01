use golden_core::{
    node::{Node, NodeData, NodeUserPermissions},
    process_ctx::ProcessCtx,
};

pub(crate) fn enable_module_authoring(node_data: &mut NodeData) {
    node_data.meta.user_permissions = NodeUserPermissions::all();
}

pub(crate) fn enable_module_manager_authoring(node_data: &mut NodeData) {
    let mut permissions = NodeUserPermissions::all();
    permissions.can_remove_and_duplicate = false;
    node_data.meta.user_permissions = permissions;
}

pub(crate) fn initialize_module_root<N: Node + ?Sized>(node: &mut N, ctx: &mut ProcessCtx) {
    enable_module_authoring(node.node_data_mut());
    node.set_child_warning_depth(ctx, 4);
}
