//! Media identity and kind use Golden metadata and a declared typed parameter.
use super::{
    ChannelNode,
    protocol::{ChannelDefinition, MediaFitMode, MediaKind},
};
use crate::matrix::protocol::{EmptyCellPolicy, MatrixSettings};
use golden_core::{
    node,
    node::{Node, ParameterHandle, ParameterValidationContext, UserContainerRules, UserCreatableItem},
    parameter::{Enum, ParamValue, Vec2},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};
use std::sync::OnceLock;

#[node(
    "mapgyver.media.v8",
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
        resolution: Vec2 = Vec2::new(1920.0, 1080.0) [(1.0, 1.0)..(16384.0, 16384.0)] (
            label = "Resolution", description = "Output width and height in pixels. Disable to use each source's native size.",
            enabled = false, can_be_disabled = true, step = 1.0, constraint_policy = "reject");
        fit_mode: Enum = "Fit" (label = "Fit Mode", dependency = |node: &Self| node.supports_fit_mode(),
            enum_options = crate::parameters::enum_options(&["Fit", "Fill", "Stretch"]), constraint_policy = "reject");
        sender_selection: Enum = "Manual" (label = "Sender selection", dependency = |node: &Self| node.kind() == super::protocol::MediaKind::SharedTexture,
            enum_options = crate::parameters::enum_options(&["Manual", "Auto"]), constraint_policy = "reject");
        empty_cell: Enum = "Transparent" (label = "Empty cell", enum_options = crate::parameters::enum_options(&["Transparent"]));
        stop_all: ParamValue = ParamValue::Trigger() (label = "Stop All", dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Matrix);
    }
)]
pub struct MediaNode {
    #[state(default = crate::matrix::controls::Pulses::default())]
    pulses: crate::matrix::controls::Pulses,
    #[state(default = RevisionSource::default())]
    membership: RevisionSource,
    #[state(default = OnceLock::new())]
    matrix_root: OnceLock<golden_core::node::NodeUuid>,
}
impl MediaNode {
    pub(crate) fn stop_all_parameter(&self) -> Option<golden_core::node::NodeId> {
        (self.kind() == MediaKind::Matrix).then(|| self.stop_all.id())
    }
    pub(crate) fn claim_control(&mut self, action: crate::matrix::controls::Action, token: uuid::Uuid) -> bool {
        self.pulses.claim(action, token)
    }
    pub(super) fn initialize_auto_sender_selection(&mut self) {
        assert!(
            !self.sender_selection.is_bound(),
            "sender defaults require detached media"
        );
        // Keep the intrinsic Manual codec baseline so older sparse projects retain their mode.
        self.sender_selection = ParameterHandle::new(Enum::new("Auto"));
    }
    pub(crate) fn sender_selection(&self) -> Option<super::protocol::SenderSelectionMode> {
        (self.kind() == super::protocol::MediaKind::SharedTexture).then(|| {
            super::protocol::SenderSelectionMode::parse(self.sender_selection.get_ref().as_str())
                .expect("validated sender selection")
        })
    }
    pub(crate) fn sender_selection_parameter(&self) -> Option<golden_core::node::NodeId> {
        (self.kind() == super::protocol::MediaKind::SharedTexture).then(|| self.sender_selection.id())
    }
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
    pub(crate) fn resolution_parameter(&self) -> golden_core::node::NodeId {
        self.resolution.id()
    }
    pub(super) fn from_definition(label: String, definition: &ChannelDefinition) -> Self {
        let mut node = Self::from_kind(label, definition.media_kind());
        node.fit_mode = ParameterHandle::new(Enum::new(definition.fit_mode.name()));
        node
    }
    pub(crate) fn supports_fit_mode(&self) -> bool {
        matches!(self.kind(), MediaKind::Picture | MediaKind::SharedTexture)
    }
    pub(crate) fn fit_mode_parameter(&self) -> Option<golden_core::node::NodeId> {
        self.supports_fit_mode().then(|| self.fit_mode.id())
    }
    pub(crate) fn fit_mode(&self) -> MediaFitMode {
        if self.supports_fit_mode() {
            MediaFitMode::parse(self.fit_mode.get_ref().as_str()).expect("validated media Fit Mode")
        } else {
            MediaFitMode::Fit
        }
    }
    pub(crate) fn matrix_settings(&self) -> MatrixSettings {
        MatrixSettings {
            empty_cell: match self.empty_cell.get_ref().as_str() {
                "Transparent" => EmptyCellPolicy::Transparent,
                _ => EmptyCellPolicy::default(),
            },
        }
    }
    pub(crate) fn matrix_setting_edits(&self, settings: MatrixSettings) -> Vec<golden_core::edit::Edit> {
        [(
            self.empty_cell.id(),
            ParamValue::Enum(
                match settings.empty_cell {
                    EmptyCellPolicy::Transparent => "Transparent",
                }
                .into(),
            ),
        )]
        .into_iter()
        .map(|(node, value)| golden_core::edit::Edit::SetParam {
            node,
            value,
            behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
        })
        .collect()
    }
    pub(crate) fn size_policy(&self, engine: &crate::app::AppEngine) -> super::protocol::SizePolicy {
        if engine
            .nodes
            .get(self.resolution.id())
            .is_some_and(|node| node.node_data().meta.enabled)
        {
            let value = self.resolution.get();
            super::protocol::SizePolicy::Fixed {
                extent: golden_video::Extent2D::new(value.x as u32, value.y as u32)
                    .unwrap_or_else(|_| super::resolution::default_canvas()),
            }
        } else {
            super::protocol::SizePolicy::Automatic
        }
    }
    pub(crate) fn invalidate(&self) {
        self.membership.advance();
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
#[node("mapgyver.media.v8", from_struct)]
impl Node for MediaNode {
    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }
    fn init(&mut self, ctx: &mut ProcessCtx) {
        ctx.add_event_listener_subtree(self.id(), self.id(), 2);
    }
    fn on_meta_changed(
        &mut self,
        _: &mut ProcessCtx,
        node: golden_core::node::NodeId,
        patch: golden_core::node::NodeMetaPatch,
    ) {
        if node == self.resolution.id() && patch.enabled.is_some() {
            self.invalidate();
        }
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        // Container shape is stable while parameter values bind during project decode.
        // Catalog creation and app admission apply the source-specific policy.
        Some(UserContainerRules::new(&[
            "mapgyver.media.channel",
            "mapgyver.media.v8",
            super::reference::NODE_TYPE,
            super::composition::PLACEMENT_TYPE,
        ]))
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
        if node_type == "mapgyver.media.channel" && self.kind().has_authored_channel_variants() {
            Some(golden_core::edit::NodeTree::new(ChannelNode::from_definition(
                "Channel".into(),
                ChannelDefinition::for_media(self.kind()),
            )))
        } else {
            None
        }
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        if !self.kind().has_authored_channel_variants() {
            Vec::new()
        } else {
            vec![
                UserCreatableItem::new("mapgyver.media.channel", "mapgyver.media.channel", "Channel")
                    .with_select_when_created(false),
            ]
        }
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.pulses.destroy();
        self.membership.advance();
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.membership.advance();
    }
    fn on_node_ready(&mut self, _: &mut ProcessCtx, _: golden_core::node::NodeCreationContext) {
        self.pulses.ready();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, enabled: bool) {
        if !enabled {
            self.pulses.revoke();
        }
    }
    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        let id = self.id();
        self.pulses.result(ctx, id, &event);
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, parameter: golden_core::node::NodeId, _: ParamValue) {
        if parameter == self.stop_all.id() {
            if self.kind() == MediaKind::Matrix {
                self.pulses
                    .invoke(ctx, &self.node_data, crate::matrix::controls::Action::StopAll);
            }
            return;
        }
        self.membership.advance();
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        if self.kind() == super::protocol::MediaKind::SharedTexture {
            super::protocol::SenderSelectionMode::parse(self.sender_selection.get_ref().as_str())?;
        }
        validate_resolution(self.resolution.get())?;
        if self.empty_cell.get_ref().as_str() != "Transparent" {
            return Err("invalid shared Matrix settings".into());
        }
        if self.supports_fit_mode() {
            MediaFitMode::parse(self.fit_mode.get_ref().as_str())?;
        }
        if !["Picture", "SolidColor", "SharedTexture", "Composition", "Matrix"]
            .contains(&self.media_kind.get_ref().as_str())
        {
            return Err("unknown media kind".into());
        }
        Ok(())
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if self.sender_selection_parameter() == Some(change.parameter) {
            let ParamValue::Enum(mode) = change.value else {
                return Err("sender selection must be an Enum".into());
            };
            super::protocol::SenderSelectionMode::parse(mode)?;
        }
        if change.parameter == self.resolution.id() {
            let ParamValue::Vec2(x, y) = change.value else {
                return Err("Resolution must be a Vec2 in pixels".into());
            };
            validate_resolution(Vec2::new(*x, *y))?;
        }
        if change.parameter == self.media_kind.id() && change.value != &ParamValue::Enum(kind_name(self.kind()).into())
        {
            return Err("Media kind is immutable; create another media node".into());
        }
        Ok(())
    }
    fn parameter_companion_changes(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Vec<(golden_core::node::NodeId, ParamValue)>, String> {
        if self.sender_selection_parameter() != Some(change.parameter) {
            return Ok(Vec::new());
        }
        let ParamValue::Enum(value) = change.value else {
            return Err("sender selection must be an Enum".into());
        };
        let mode = super::protocol::SenderSelectionMode::parse(value)?;
        let previous = self.sender_selection().unwrap();
        if mode == previous {
            return Ok(Vec::new());
        }
        let mut values = Vec::new();
        let mut child = self.node_data().first_child;
        while let Some(id) = child {
            let node = change.node(id).ok_or("media child missing")?;
            child = node.node_data().next_sibling;
            if let Some(channel) = node.as_any().downcast_ref::<super::ChannelNode>() {
                let destination = channel.sender_parameter(mode);
                if !change.is_proposed(destination) {
                    let name = channel.sender_name(previous);
                    values.push((
                        destination,
                        match mode {
                            super::protocol::SenderSelectionMode::Manual => ParamValue::Str(name),
                            super::protocol::SenderSelectionMode::Auto => ParamValue::Enum(name),
                        },
                    ));
                }
            }
        }
        Ok(values)
    }
}
pub(super) fn validate_resolution(value: Vec2) -> Result<(), String> {
    if [value.x, value.y]
        .iter()
        .any(|axis| !axis.is_finite() || !(1.0..=16384.0).contains(axis) || axis.fract() != 0.0)
    {
        return Err("Resolution must contain whole pixels in 1..16384 per axis".into());
    }
    Ok(())
}
pub(super) fn kind_name(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Reference => "Reference",
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
