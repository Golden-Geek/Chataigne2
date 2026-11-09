use crate::{app::AppEngine, media::protocol::*};
use golden_core::{
    node,
    node::{Node, NodeId, NodeReference, ParameterHandle, ParameterValidationContext, UserContainerRules},
    parameter::{Enum, ParamValue, ReferenceTargetKind, Vec2},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};
use golden_video::Extent2D;

#[node("mapgyver.media.reference.v4", label = "Reference", user_permissions = golden_core::node::NodeUserPermissions {
    can_edit_name: true, can_remove_and_duplicate: true, ..Default::default()
})]
#[children(folder(parameters, label = "Parameters") {
    target: NodeReference = NodeReference::empty() (
        label = "Media", reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec!["mapgyver.media.v11".into(), "mapgyver.media.reference.v4".into()]
    );
    channel: NodeReference = NodeReference::empty() (
        label = "Channel", description = "Empty follows the first available output; an explicit missing identity never reroutes.",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec!["mapgyver.media.channel.v2".into(), crate::media::reference::OUTPUT_TYPE.into()]
    );
    resolution: Vec2 = Vec2::new(1920.0, 1080.0) [(1.0, 1.0)..(16384.0, 16384.0)] (
        label = "Resolution", enabled = false, can_be_disabled = true, step = 1.0, constraint_policy = "reject"
    );
    fit_mode: Enum = "Fit" (label = "Fit Mode",
        enum_options = crate::parameters::enum_options(&["Fit", "Fill", "Stretch"]), constraint_policy = "reject");
}
node output: super::ReferenceOutputNode = (self.create_output()) (label = "Output");
)]
pub struct ReferenceNode {
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
}
impl ReferenceNode {
    fn create_output(&self) -> super::ReferenceOutputNode {
        let mut output = super::ReferenceOutputNode::new();
        output.node_data_mut().meta.can_be_disabled = false;
        output
    }
    pub(crate) fn from_source(label: String, source: Option<super::super::MediaSource>) -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.label = label;
        node.target =
            ParameterHandle::new(source.map_or_else(NodeReference::empty, |source| NodeReference::new(source.media.0)));
        node.channel = ParameterHandle::new(
            source
                .and_then(|source| source.channel)
                .map_or_else(NodeReference::empty, |channel| NodeReference::new(channel.0)),
        );
        node
    }
    pub(crate) fn output_reference(&self, engine: &AppEngine) -> ChannelReference {
        let id = self.output.current_id().expect("materialized Reference output");
        ChannelReference {
            media: MediaId(self.node_data().meta.uuid),
            channel: ChannelId(engine.nodes.get(id).expect("Reference output").node_data().meta.uuid),
        }
    }
    pub(crate) fn source(&self, engine: &AppEngine) -> Option<ChannelReference> {
        let media = self.target()?;
        let channel = if self.channel.get_ref().uuid.0.is_nil() {
            super::Resolver::new(engine)
                .channels(media)
                .ok()?
                .next()
                .map(|node| ChannelId(node.node_data().meta.uuid))?
        } else {
            ChannelId(self.channel.get_ref().uuid)
        };
        Some(ChannelReference { media, channel })
    }
    pub(crate) fn source_for_validation(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Option<ChannelReference>, String> {
        let Some(media) = self.target_for_validation(change) else {
            return Ok(None);
        };
        let Some(ParamValue::Reference(channel)) = change.parameter_value(self.channel.id()) else {
            return Ok(None);
        };
        let output = if channel.uuid.0.is_nil() {
            super::super::targeting::first_output(change, media)?
        } else {
            Some(channel.uuid)
        };
        Ok(output.map(|channel| ChannelReference {
            media: MediaId(media),
            channel: ChannelId(channel),
        }))
    }
    pub(crate) fn target(&self) -> Option<MediaId> {
        (!self.target.get_ref().uuid.0.is_nil()).then_some(MediaId(self.target.get_ref().uuid))
    }
    pub(crate) fn selected_channel(&self) -> Option<ChannelId> {
        (!self.channel.get_ref().uuid.0.is_nil()).then_some(ChannelId(self.channel.get_ref().uuid))
    }
    pub(crate) fn target_parameter(&self) -> NodeId {
        self.target.id()
    }
    pub(crate) fn channel_parameter(&self) -> NodeId {
        self.channel.id()
    }
    pub(crate) fn resolution_parameter(&self) -> NodeId {
        self.resolution.id()
    }
    pub(crate) fn fit_mode_parameter(&self) -> NodeId {
        self.fit_mode.id()
    }
    pub(crate) fn target_for_validation(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Option<golden_core::node::NodeUuid> {
        let ParamValue::Reference(value) = change.parameter_value(self.target.id())? else {
            return None;
        };
        (!value.uuid.0.is_nil()).then_some(value.uuid)
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(crate) fn definition(&self, engine: &AppEngine, _channel: ChannelId) -> ChannelDefinition {
        let size = if engine
            .nodes
            .get(self.resolution.id())
            .is_some_and(|node| node.node_data().meta.enabled)
        {
            let value = self.resolution.get();
            SizePolicy::Fixed {
                extent: Extent2D::new(value.x as u32, value.y as u32).expect("validated Reference Resolution"),
            }
        } else {
            SizePolicy::Automatic
        };
        ChannelDefinition {
            fit_mode: MediaFitMode::parse(self.fit_mode.get_ref().as_str()).expect("validated Reference Fit Mode"),
            source: ChannelSource::Reference {
                target: self.source(engine),
            },
            size,
        }
    }
}
#[node("mapgyver.media.reference.v4", from_struct)]
impl Node for ReferenceNode {
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[super::super::composition::PLACEMENT_TYPE]))
    }
    fn on_param_change(&mut self, _: &mut ProcessCtx, _: NodeId, _: ParamValue) {
        self.revision.advance();
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.revision.advance();
    }
    fn on_meta_changed(&mut self, _: &mut ProcessCtx, _: NodeId, _: golden_core::node::NodeMetaPatch) {
        self.revision.advance();
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.revision.advance();
    }
    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        super::super::media_node::validate_resolution(self.resolution.get())?;
        MediaFitMode::parse(self.fit_mode.get_ref().as_str())?;
        Ok(())
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if change.parameter == self.resolution.id()
            && let ParamValue::Vec2(x, y) = change.value
        {
            super::super::media_node::validate_resolution(Vec2::new(*x, *y))?;
        }
        if change.parameter == self.target.id() || change.parameter == self.channel.id() {
            if self.target_for_validation(change).is_none()
                && matches!(change.parameter_value(self.channel.id()), Some(ParamValue::Reference(value)) if !value.uuid.0.is_nil())
            {
                return Err("Reference Channel requires a Media target".into());
            }
            if let Some(source) = self.source_for_validation(change)? {
                crate::media::targeting::validate_output_membership(change, source.media.0, source.channel.0)?;
            }
            let output = self
                .output
                .current_id()
                .and_then(|id| change.node(id))
                .ok_or("Reference output is missing")?
                .node_data()
                .meta
                .uuid;
            crate::parameters::reference_cycle::validate_output(change, self.node_data().meta.uuid, output)?;
        }
        Ok(())
    }
    fn parameter_companion_changes(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Vec<(NodeId, ParamValue)>, String> {
        if change.parameter != self.target.id() || change.is_proposed(self.channel.id()) {
            return Ok(Vec::new());
        }
        let ParamValue::Reference(target) = change.value else {
            return Err("Reference Media must be a Reference".into());
        };
        if target.uuid == self.target.get_ref().uuid {
            return Ok(Vec::new());
        }
        let output = if target.uuid.0.is_nil() {
            None
        } else {
            super::super::targeting::first_output(change, target.uuid)?
        };
        Ok(vec![(
            self.channel.id(),
            ParamValue::Reference(output.map_or_else(NodeReference::empty, NodeReference::new)),
        )])
    }
}
