use golden_core::{
    node,
    node::{Node, NodeReference, ParameterHandle, UserContainerRules},
    parameter::ReferenceTargetKind,
};

pub(crate) const PRESET_TYPE: &str = "mapgyver.matrix.preset.v1";
pub(crate) const ENTRY_TYPE: &str = "mapgyver.matrix.preset_entry.v1";

#[node("mapgyver.matrix.presets.v1", label = "Configurations", can_be_disabled = false)]
pub struct PresetCollectionNode {}
#[node("mapgyver.matrix.presets.v1", from_struct)]
impl Node for PresetCollectionNode {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[PRESET_TYPE]))
    }
    fn create_user_item(&self, kind: &str) -> Option<Box<dyn Node>> {
        (kind == PRESET_TYPE).then(|| Box::new(PresetNode::new()) as Box<dyn Node>)
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
}

#[node("mapgyver.matrix.preset.v1", label = "Configuration", can_be_disabled = false, collapsed = true,
    user_permissions = golden_core::node::NodeUserPermissions { can_edit_name: true, can_remove_and_duplicate: true, ..Default::default() })]
pub struct PresetNode {}
#[node("mapgyver.matrix.preset.v1", from_struct)]
impl Node for PresetNode {
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[ENTRY_TYPE]))
    }
    fn create_user_item(&self, kind: &str) -> Option<Box<dyn Node>> {
        (kind == ENTRY_TYPE).then(|| Box::new(PresetEntryNode::new()) as Box<dyn Node>)
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
}

#[node("mapgyver.matrix.preset_entry.v1", label = "Member", can_be_disabled = false, show_in_nested_inspector = false,
    user_permissions = golden_core::node::NodeUserPermissions { can_edit_name: true, can_remove_and_duplicate: true, ..Default::default() })]
#[children(folder(parameters, label = "Parameters", can_be_disabled = false) {
    target: NodeReference = NodeReference::empty() (label = "Member", reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec!["mapgyver.matrix.layer.v4".into(), "mapgyver.matrix.column.v3".into()]);
    order: i32 = 0 [0..100000] (label = "Order", constraint_policy = "reject");
    member_enabled: bool = true (label = "Enabled");
})]
pub struct PresetEntryNode {}
impl PresetEntryNode {
    pub(super) fn from_member(target: golden_core::node::NodeUuid, order: usize, enabled: bool, label: String) -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.label = label;
        node.target = ParameterHandle::new(NodeReference::new(target));
        node.order = ParameterHandle::new(order as i32);
        node.member_enabled = ParameterHandle::new(enabled);
        node
    }
    pub(super) fn values(&self) -> (golden_core::node::NodeUuid, i32, bool) {
        (self.target.get_ref().uuid, self.order.get(), self.member_enabled.get())
    }
}
#[node("mapgyver.matrix.preset_entry.v1", from_struct)]
impl Node for PresetEntryNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
}
