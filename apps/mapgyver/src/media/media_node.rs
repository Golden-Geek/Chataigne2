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
    "mapgyver.media.v11",
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
            enum_options = crate::parameters::enum_options(&["Picture", "Video", "SolidColor", "SharedTexture", "Composition", "Matrix"]));
        resolution: Vec2 = Vec2::new(1920.0, 1080.0) [(1.0, 1.0)..(16384.0, 16384.0)] (
            label = "Resolution", description = "Output width and height in pixels. Disable to use each source's native size.",
            enabled = false, can_be_disabled = true, step = 1.0, constraint_policy = "reject");
        fit_mode: Enum = "Fit" (label = "Fit Mode", dependency = |node: &Self| node.supports_fit_mode(),
            enum_options = crate::parameters::enum_options(&["Fit", "Fill", "Stretch"]), constraint_policy = "reject");
        sender_selection: Enum = "Manual" (label = "Sender selection", dependency = |node: &Self| node.kind() == super::protocol::MediaKind::SharedTexture,
            enum_options = crate::parameters::enum_options(&["Manual", "Auto"]), constraint_policy = "reject");
        empty_cell: Enum = "Transparent" (label = "Empty cell", enum_options = crate::parameters::enum_options(&["Transparent"]));
        stop_all: ParamValue = ParamValue::Trigger() (label = "Stop All", dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Matrix);
        video_play: ParamValue = ParamValue::Trigger() (label = "Play", dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Video);
        video_pause: ParamValue = ParamValue::Trigger() (label = "Pause", dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Video);
        video_stop: ParamValue = ParamValue::Trigger() (label = "Stop", dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Video);
        video_play_at_load: bool = false (label = "Play at Load", dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Video);
        video_current_position_ms: f64 = 0.0 [0.0..9000000000000.0] (label = "Current Position", widget = "time_slider", unit = "ms", transient_value = true,
            dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Video, constraint_policy = "clamp_adapt");
        video_speed: f64 = 1.0 [-16.0..16.0] (label = "Speed", step = 0.01,
            dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Video, constraint_policy = "reject");
        video_looping: bool = false (label = "Loop", dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Video);
        video_playing: bool = false (label = "Playing", read_only = true,
            dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Video);
        video_length_ms: f64 = 0.0 [0.0..9000000000000.0] (label = "Length", read_only = true, widget = "time", unit = "ms",
            dependency = |node: &Self| node.kind() == super::protocol::MediaKind::Video);
    }
)]
pub struct MediaNode {
    #[state(default = crate::matrix::controls::Pulses::default())]
    pulses: crate::matrix::controls::Pulses,
    #[state(default = RevisionSource::default())]
    membership: RevisionSource,
    #[state(default = OnceLock::new())]
    matrix_root: OnceLock<golden_core::node::NodeUuid>,
    #[state(default = OnceLock::new())]
    video_control: OnceLock<std::sync::Arc<super::video::VideoControl>>,
    #[state(default = false)]
    video_ready: bool,
    #[state(default = super::video::feedback::Inbox::default())]
    video_feedback: super::video::feedback::Inbox,
}
impl MediaNode {
    pub(crate) fn video_control(&self) -> Option<std::sync::Arc<super::video::VideoControl>> {
        (self.kind() == MediaKind::Video).then(|| self.video_control.get_or_init(Default::default).clone())
    }
    pub(crate) fn video_settings(&self) -> Result<super::video::protocol::VideoTransportSettings, String> {
        let mut settings = self.video_control().ok_or("media is not Video")?.settings()?;
        settings.speed = self.video_speed.get();
        settings.looping = self.video_looping.get();
        settings.play_at_load = self.video_play_at_load.get();
        settings.validate()?;
        Ok(settings)
    }
    pub(crate) fn video_parameter_values(
        &self,
        edit: &super::video::protocol::VideoEdit,
    ) -> Vec<(golden_core::node::NodeId, ParamValue)> {
        use super::video::protocol::VideoEdit;
        match edit {
            VideoEdit::Play => vec![(self.video_play.id(), ParamValue::Trigger())],
            VideoEdit::Pause | VideoEdit::Step { .. } => vec![(self.video_pause.id(), ParamValue::Trigger())],
            VideoEdit::Stop => vec![(self.video_stop.id(), ParamValue::Trigger())],
            VideoEdit::Seek { position_ms } => {
                vec![(self.video_current_position_ms.id(), ParamValue::Float(*position_ms))]
            }
            VideoEdit::Speed { speed } => vec![(self.video_speed.id(), ParamValue::Float(*speed))],
            VideoEdit::Loop { looping } => vec![(self.video_looping.id(), ParamValue::Bool(*looping))],
            VideoEdit::PlayAtLoad { enabled } => vec![(self.video_play_at_load.id(), ParamValue::Bool(*enabled))],
        }
    }
    fn video_parameter(&self, id: golden_core::node::NodeId) -> bool {
        [
            self.video_play.id(),
            self.video_pause.id(),
            self.video_stop.id(),
            self.video_play_at_load.id(),
            self.video_current_position_ms.id(),
            self.video_speed.id(),
            self.video_looping.id(),
        ]
        .contains(&id)
    }
    fn publish_video(&self, ctx: &mut ProcessCtx, parameter: Option<golden_core::node::NodeId>) {
        if self.video_ready
            && let Some(control) = self.video_control()
        {
            match self.video_settings().and_then(|mut settings| {
                use super::video::protocol::VideoPlaybackMode;
                match parameter {
                    None => {
                        settings.position_ms = 0.0;
                        ctx.set_param_feedback_from(
                            self.id(),
                            self.video_current_position_ms.id(),
                            Some(ParamValue::Float(0.0)),
                        );
                        control.initialize(settings)
                    }
                    Some(id) if id == self.video_play.id() => control.command(VideoPlaybackMode::Playing),
                    Some(id) if id == self.video_pause.id() => control.command(VideoPlaybackMode::Paused),
                    Some(id) if id == self.video_stop.id() => control.command(VideoPlaybackMode::Stopped),
                    Some(id) if id == self.video_current_position_ms.id() => {
                        settings.position_ms = self.video_current_position_ms.get();
                        if settings.mode == VideoPlaybackMode::Stopped {
                            settings.mode = VideoPlaybackMode::Paused;
                        }
                        control.configure(settings, true)
                    }
                    Some(_) => control.configure(settings, false),
                }
            }) {
                Ok(()) => ctx.clear_node_warning(self.id(), Some("video-transport")),
                Err(error) => ctx.set_node_warning_with(self.id(), Some("video-transport"), error, None),
            }
        }
    }
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
        matches!(
            self.kind(),
            MediaKind::Picture | MediaKind::Video | MediaKind::SharedTexture
        )
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
#[node("mapgyver.media.v11", from_struct)]
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
            "mapgyver.media.channel.v2",
            "mapgyver.media.v11",
            super::reference::NODE_TYPE,
            super::composition::PLACEMENT_TYPE,
        ]))
    }
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == "mapgyver.media.channel.v2").then(|| {
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
        if node_type == "mapgyver.media.channel.v2" && self.kind().has_authored_channel_variants() {
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
                UserCreatableItem::new("mapgyver.media.channel.v2", "mapgyver.media.channel.v2", "Channel")
                    .with_select_when_created(false),
            ]
        }
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.video_feedback.close();
        if let Some(control) = self.video_control.get() {
            control.retire_media();
        }
        self.pulses.destroy();
        self.membership.advance();
    }
    fn on_structure_changed(&mut self, _: &mut ProcessCtx) {
        self.membership.advance();
    }
    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _: golden_core::node::NodeCreationContext) {
        self.pulses.ready();
        self.video_ready = true;
        if let Some(control) = self.video_control() {
            control.activate_media();
            match self
                .video_feedback
                .register(ctx)
                .and_then(|publisher| control.set_feedback(publisher))
            {
                Ok(()) => ctx.clear_node_warning(self.id(), Some("video-feedback")),
                Err(error) => ctx.set_node_warning_with(self.id(), Some("video-feedback"), error, None),
            }
        }
        self.publish_video(ctx, None);
        #[cfg(feature = "video-files")]
        if let Some(control) = self.video_control() {
            let control = std::sync::Arc::downgrade(&control);
            let media = super::protocol::MediaId(self.node_data().meta.uuid);
            ctx.register_ancestor_runtime(self.id(), move |owner| {
                let Some(manager) = owner.as_any().downcast_ref::<crate::media::MediaManagerNode>() else {
                    return Ok(false);
                };
                manager.videos.register(media, control.clone())?;
                Ok(true)
            });
        }
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, enabled: bool) {
        if !enabled {
            self.pulses.revoke();
        }
    }
    fn worker_wake_when_disabled(&self) -> bool {
        true
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        if let Some(status) = self.video_feedback.take() {
            ctx.edits.push_from(
                self.id(),
                golden_core::edit::Edit::SetParamRuntimeRange {
                    node: self.video_current_position_ms.id(),
                    range: golden_core::parameter::RangeConstraint::uniform(Some(0.0), Some(status.length_ms)),
                },
            );
            ctx.set_param_feedback_from(
                self.id(),
                self.video_playing.id(),
                Some(ParamValue::Bool(status.playing)),
            );
            ctx.set_param_feedback_from(
                self.id(),
                self.video_length_ms.id(),
                Some(ParamValue::Float(status.length_ms)),
            );
        }
    }
    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        let id = self.id();
        self.pulses.result(ctx, id, &event);
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, parameter: golden_core::node::NodeId, _: ParamValue) {
        if parameter == self.video_current_position_ms.id()
            && ctx.current_event().is_some_and(|event| {
                matches!(&event.kind,
                golden_core::events::EventKind::ParamChanged { source: Some(owner), .. } if *owner == self.id())
            })
        {
            return;
        }
        if parameter == self.video_playing.id() || parameter == self.video_length_ms.id() {
            return;
        }
        if self.video_parameter(parameter) {
            self.publish_video(ctx, Some(parameter));
            return;
        }
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
        if self.kind() == MediaKind::Video {
            self.video_settings()?.validate()?;
            if self.video_current_position_ms.get() != 0.0 {
                return Err("Current Position is runtime state and must start at zero".into());
            }
        }
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
        if ![
            "Picture",
            "Video",
            "SolidColor",
            "SharedTexture",
            "Composition",
            "Matrix",
        ]
        .contains(&self.media_kind.get_ref().as_str())
        {
            return Err("unknown media kind".into());
        }
        Ok(())
    }
    fn normalize_parameter_change(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Option<ParamValue>, String> {
        if change.parameter != self.video_current_position_ms.id() {
            return Ok(None);
        }
        let ParamValue::Float(value) = change.value else {
            return Err("Current Position requires a time value".into());
        };
        let control = self.video_control().ok_or("Current Position belongs to Video media")?;
        control
            .bounded_position(*value)
            .map(|value| Some(ParamValue::Float(value)))
    }
    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if self.video_parameter(change.parameter) {
            let mut settings = self.video_settings()?;
            match (change.parameter, change.value) {
                (id, ParamValue::Trigger())
                    if [self.video_play.id(), self.video_pause.id(), self.video_stop.id()].contains(&id) => {}
                (id, ParamValue::Float(value)) if id == self.video_current_position_ms.id() => {
                    settings.position_ms = *value
                }
                (id, ParamValue::Float(value)) if id == self.video_speed.id() => settings.speed = *value,
                (id, ParamValue::Bool(value)) if id == self.video_looping.id() => settings.looping = *value,
                (id, ParamValue::Bool(value)) if id == self.video_play_at_load.id() => settings.play_at_load = *value,
                _ => return Err("invalid Video transport Parameter type".into()),
            }
            settings.validate()?;
        }
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
        MediaKind::Video => "Video",
        MediaKind::SolidColor => "SolidColor",
        MediaKind::SharedTexture => "SharedTexture",
        MediaKind::Composition => "Composition",
        MediaKind::Matrix => "Matrix",
    }
}
pub(super) fn parse_kind(kind: &str) -> MediaKind {
    match kind {
        "Picture" => MediaKind::Picture,
        "Video" => MediaKind::Video,
        "SharedTexture" => MediaKind::SharedTexture,
        "Composition" => MediaKind::Composition,
        "Matrix" => MediaKind::Matrix,
        _ => MediaKind::SolidColor,
    }
}
