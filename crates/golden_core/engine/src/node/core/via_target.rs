use super::{Node, NodeData, NodeId, UserContainerRules, UserContextHostPolicy, UserCreatableItem};
use crate::{events::Event, parameter::ParamValue, process_ctx::ProcessCtx, script::ScriptHostPolicy};

#[allow(missing_docs)]
/// Adapter used by `#[node(..., via = ...)]` to access either a `NodeData` field directly or a composed node that owns the runtime identity.
pub trait ViaTarget {
    /// Returns the node data backing the via target.
    fn via_node_data(&self) -> &NodeData;
    /// Returns mutable node data backing the via target.
    fn via_node_data_mut(&mut self) -> &mut NodeData;

    fn via_engine_child_event_interest_depth(&self, _event: &Event) -> u32 {
        0
    }

    fn via_engine_on_attached(&mut self, _ctx: &mut ProcessCtx) {}

    fn via_engine_sync_param_handle_cache(&mut self, _param: NodeId, _new_value: &ParamValue) {}

    fn via_engine_sync_bound_param_handles(&mut self, _resolve: &mut dyn FnMut(NodeId) -> Option<ParamValue>) {}

    fn via_engine_materialize_declared_inbox(&mut self, _ctx: &mut ProcessCtx) {}

    fn via_engine_preprocess_inbox(&mut self, _ctx: &mut ProcessCtx) {}

    fn via_script_host_policy(&self) -> Option<ScriptHostPolicy> {
        None
    }

    fn via_user_context_host_policy(&self) -> Option<UserContextHostPolicy> {
        None
    }

    fn via_user_container_rules(&self) -> Option<UserContainerRules> {
        None
    }

    fn via_user_container_accepts_item(&self, _item_type: &str, _item_kind: &str) -> bool {
        false
    }

    fn via_user_creatable_items(&self) -> Vec<UserCreatableItem> {
        Vec::new()
    }

    fn via_create_user_item(&self, _node_type: &str) -> Option<Box<dyn Node>> {
        None
    }

    fn via_create_user_item_for_host(&self, host_node_type: &str, node_type: &str) -> Option<Box<dyn Node>> {
        let _ = host_node_type;
        self.via_create_user_item(node_type)
    }

    fn via_project_encode_data(&self) -> Result<serde_json::Value, String> {
        Ok(serde_json::Value::Null)
    }

    fn via_capture_authored_data(&self) -> Result<serde_json::Value, String> {
        self.via_project_encode_data()
    }

    fn via_restore_authored_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        self.via_project_decode_data(data)
    }

    fn via_project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        if data.is_null() {
            return Ok(());
        }

        Err("via target does not support persisted project data".to_string())
    }
}

impl ViaTarget for NodeData {
    fn via_node_data(&self) -> &NodeData {
        self
    }

    fn via_node_data_mut(&mut self) -> &mut NodeData {
        self
    }
}

impl<T: Node + ?Sized> ViaTarget for T {
    fn via_node_data(&self) -> &NodeData {
        self.node_data()
    }

    fn via_node_data_mut(&mut self) -> &mut NodeData {
        self.node_data_mut()
    }

    fn via_engine_child_event_interest_depth(&self, event: &Event) -> u32 {
        self.engine_child_event_interest_depth(event)
    }

    fn via_engine_on_attached(&mut self, ctx: &mut ProcessCtx) {
        self.engine_on_attached(ctx);
    }

    fn via_engine_sync_param_handle_cache(&mut self, param: NodeId, new_value: &ParamValue) {
        self.engine_sync_param_handle_cache(param, new_value);
    }

    fn via_engine_sync_bound_param_handles(&mut self, resolve: &mut dyn FnMut(NodeId) -> Option<ParamValue>) {
        self.engine_sync_bound_param_handles(resolve);
    }

    fn via_engine_materialize_declared_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.engine_materialize_declared_inbox(ctx);
    }

    fn via_engine_preprocess_inbox(&mut self, ctx: &mut ProcessCtx) {
        self.engine_preprocess_inbox(ctx);
    }

    fn via_script_host_policy(&self) -> Option<ScriptHostPolicy> {
        self.script_host_policy()
    }

    fn via_user_context_host_policy(&self) -> Option<UserContextHostPolicy> {
        self.user_context_host_policy()
    }

    fn via_user_container_rules(&self) -> Option<UserContainerRules> {
        self.user_container_rules()
    }

    fn via_user_container_accepts_item(&self, item_type: &str, item_kind: &str) -> bool {
        self.user_container_accepts_item(item_type, item_kind)
    }

    fn via_user_creatable_items(&self) -> Vec<UserCreatableItem> {
        self.user_creatable_items()
    }

    fn via_create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        self.create_user_item(node_type)
    }

    fn via_create_user_item_for_host(&self, host_node_type: &str, node_type: &str) -> Option<Box<dyn Node>> {
        self.create_user_item_for_host(host_node_type, node_type)
    }

    fn via_project_encode_data(&self) -> Result<serde_json::Value, String> {
        self.project_encode_data()
    }

    fn via_capture_authored_data(&self) -> Result<serde_json::Value, String> {
        self.engine_capture_authored_data()
    }

    fn via_restore_authored_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        self.engine_restore_authored_data(data)
    }

    fn via_project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        self.project_decode_data(data)
    }
}
