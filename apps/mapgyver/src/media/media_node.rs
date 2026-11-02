//! Media identity and kind use Golden metadata and a declared typed parameter.
use super::{
    ChannelNode,
    protocol::{ChannelDefinition, MediaKind},
};
use golden_core::{
    node,
    node::{Node, ParameterHandle, ParameterValidationContext, UserContainerRules, UserCreatableItem},
    parameter::{Enum, ParamValue},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};
use std::sync::OnceLock;

#[node(
    "mapgyver.media",
    label = "Media",
    user_permissions = golden_core::node::NodeUserPermissions {
        can_edit_name: true,
        can_remove_and_duplicate: true,
        ..Default::default()
    }
)]
#[children(
    folder(parameters, label = "Parameters") {
        media_kind: Enum = "SolidColor" (label = "Kind", read_only = true, persist_read_only_value = true,
            enum_options = crate::parameters::enum_options(&["Picture", "SolidColor", "SharedTexture", "Composition", "Matrix"]));
    }
)]
pub struct MediaNode {
    #[state(default = RevisionSource::default())]
    membership: RevisionSource,
    #[state(default = OnceLock::new())]
    matrix_root: OnceLock<golden_core::node::NodeUuid>,
}
impl MediaNode {
    pub(crate) fn from_kind(label: String, kind: MediaKind) -> Self {
        let mut node = Self::new();
        let permissions = node.node_data().meta.user_permissions.clone();
        node.node_data_mut().meta = golden_core::node::NodeMeta::new(label).with_user_permissions(permissions);
        node.media_kind = ParameterHandle::new(Enum::new(kind_name(kind)));
        node
    }
    pub(crate) fn kind(&self) -> MediaKind {
        parse_kind(self.media_kind.get_ref().as_str())
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.membership.reader();
        reader.guard_at(reader.current())
    }
    pub(crate) fn matrix_root(&self) -> Option<golden_core::node::NodeUuid> {
        self.matrix_root.get().copied()
    }
    pub(crate) fn cache_matrix_root(&self, root: golden_core::node::NodeUuid) {
        let _ = self.matrix_root.set(root);
    }
}
#[node("mapgyver.media", from_struct)]
impl Node for MediaNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        // Container shape is stable while parameter values bind during project decode.
        // Catalog creation and app admission apply the source-specific policy.
        Some(UserContainerRules::new(&["mapgyver.media.channel"]))
    }
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == "mapgyver.media.channel").then(|| {
            // Sparse encode and decode must see the same baseline, including before
            // the parent's declared kind parameter has been materialized or bound.
            Box::new(ChannelNode::new()) as Box<dyn Node>
        })
    }
    fn create_user_item_tree_with_context(
        &self,
        node_type: &str,
        _context: &golden_core::node::UserItemCreationContext<'_>,
    ) -> Option<golden_core::edit::NodeTree> {
        if node_type == "mapgyver.media.channel" && self.kind() != MediaKind::Matrix {
            Some(golden_core::edit::NodeTree::new(ChannelNode::from_definition(
                "Channel".into(),
                ChannelDefinition::for_media(self.kind()),
            )))
        } else {
            None
        }
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        if self.kind() == MediaKind::Matrix {
            Vec::new()
        } else {
            vec![UserCreatableItem::new(
                "mapgyver.media.channel",
                "mapgyver.media.channel",
                "Channel",
            )]
        }
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.membership.advance();
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.membership.advance();
    }
    fn on_param_change(&mut self, _: &mut ProcessCtx, _: golden_core::node::NodeId, _: ParamValue) {
        self.membership.advance();
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        if !["Picture", "SolidColor", "SharedTexture", "Composition", "Matrix"]
            .contains(&self.media_kind.get_ref().as_str())
        {
            return Err("unknown media kind".into());
        }
        Ok(())
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if change.parameter == self.media_kind.id() && change.value != &ParamValue::Enum(kind_name(self.kind()).into())
        {
            return Err("Media kind is immutable; create another media node".into());
        }
        Ok(())
    }
}
pub(super) fn kind_name(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Picture => "Picture",
        MediaKind::SolidColor => "SolidColor",
        MediaKind::SharedTexture => "SharedTexture",
        MediaKind::Composition => "Composition",
        MediaKind::Matrix => "Matrix",
    }
}
pub(super) fn parse_kind(kind: &str) -> MediaKind {
    match kind {
        "Picture" => MediaKind::Picture,
        "SharedTexture" => MediaKind::SharedTexture,
        "Composition" => MediaKind::Composition,
        "Matrix" => MediaKind::Matrix,
        _ => MediaKind::SolidColor,
    }
}
