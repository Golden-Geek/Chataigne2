use crate::{app::AppEngine, media::protocol::*};
use golden_core::{
    node,
    node::{Node, NodeId, NodeReference, ParameterHandle, ParameterValidationContext, UserContainerRules},
    parameter::{Enum, ParamValue, ReferenceTargetKind, Vec2},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};
use golden_video::{Extent2D, plan::SampleFilter};

#[node("mapgyver.media.reference.v2", label = "Reference", user_permissions = golden_core::node::NodeUserPermissions {
    can_edit_name: true, can_remove_and_duplicate: true, ..Default::default()
})]
#[children(folder(parameters, label = "Parameters") {
    target: NodeReference = NodeReference::empty() (
        label = "Target", reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec!["mapgyver.media.v6".into(), "mapgyver.media.reference.v2".into()]
    );
    resolution: Vec2 = Vec2::new(1920.0, 1080.0) [(1.0, 1.0)..(16384.0, 16384.0)] (
        label = "Resolution", enabled = false, can_be_disabled = true, step = 1.0, constraint_policy = "reject"
    );
    filter: Enum = "Linear" (label = "Sampling", enum_options = crate::parameters::enum_options(&["Linear", "Nearest"]));
    fit_mode: Enum = "Fit" (label = "Fit Mode",
        enum_options = crate::parameters::enum_options(&["Fit", "Fill", "Stretch"]), constraint_policy = "reject");
})]
pub struct ReferenceNode {
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
}
impl ReferenceNode {
    pub(crate) fn from_target(label: String, target: Option<MediaId>) -> Self {
        let mut node = Self::new();
        node.node_data_mut().meta.label = label;
        node.target = ParameterHandle::new(target.map_or_else(NodeReference::empty, |id| NodeReference::new(id.0)));
        node
    }
    pub(crate) fn target(&self) -> Option<MediaId> {
        (!self.target.get_ref().uuid.0.is_nil()).then_some(MediaId(self.target.get_ref().uuid))
    }
    pub(crate) fn target_parameter(&self) -> NodeId {
        self.target.id()
    }
    pub(crate) fn resolution_parameter(&self) -> NodeId {
        self.resolution.id()
    }
    pub(crate) fn sampling_parameter(&self) -> NodeId {
        self.filter.id()
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
    pub(crate) fn definition(&self, engine: &AppEngine, channel: ChannelId) -> ChannelDefinition {
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
                target: self.target().map(|media| ChannelReference { media, channel }),
            },
            size,
            filter: if self.filter.get_ref().as_str() == "Nearest" {
                SampleFilter::Nearest
            } else {
                SampleFilter::Linear
            },
        }
    }
}
#[node("mapgyver.media.reference.v2", from_struct)]
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
        if !["Linear", "Nearest"].contains(&self.filter.get_ref().as_str()) {
            return Err("unknown Reference sampling filter".into());
        }
        Ok(())
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if change.parameter == self.resolution.id()
            && let ParamValue::Vec2(x, y) = change.value
        {
            super::super::media_node::validate_resolution(Vec2::new(*x, *y))?;
        }
        if change.parameter == self.target.id()
            && let Some(target) = self.target_for_validation(change)
        {
            crate::parameters::reference_cycle::validate_media(change, target)?;
        }
        Ok(())
    }
}
