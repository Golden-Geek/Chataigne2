//! Variant settings are real Parameters; media-owned settings join them only in runtime projections.
use super::{
    protocol::{ChannelDefinition, ChannelSource, MediaKind, SenderSelectionMode, SizePolicy},
    video::protocol::VideoSubtitleTrackInfo,
};
use crate::matrix::protocol::{GroupId, LayerId, MatrixDefinition, MatrixOutput};
use golden_core::{
    edit::Edit,
    node,
    node::{
        Node, NodeId, NodeReference, NodeUuid, ParameterHandle, ParameterValidationContext, UserContainerRules,
        UserCreatableItem,
    },
    parameter::{Color, Enum, File, ParamValue, ParameterEventBehaviour, ReferenceTargetKind, Vec2},
    process_ctx::ProcessCtx,
};
use golden_io::{RevisionGuard, RevisionSource};

use std::sync::{Arc, OnceLock};

mod audio;
mod subtitles;

#[node("mapgyver.media.channel.v2", label = "Channel", collapsed = false)]
#[children(
    folder(parameters, label = "Parameters") {
        source_kind: Enum = "SolidColor" (label = "Source kind", read_only = true, persist_read_only_value = true,
            enum_options = crate::parameters::enum_options(&["Picture", "Video", "SolidColor", "SharedTexture", "Ndi", "Composition", "Matrix"]));
        rgba: Color = Color::new(1.0, 1.0, 1.0, 1.0) [0.0..1.0] (label = "Color", description = "Straight linear-light RGBA.", constraint_policy = "reject");
        file: File = File::default() (label = "File", description = "This channel's project-relative or native absolute source path.", dependency = |node: &Self| matches!(node.kind(), super::protocol::MediaKind::Picture | super::protocol::MediaKind::Video));
        video_loop_file: File = File::default() (label = "Loop File", description = "Optional video played after this channel's intro when Loop is enabled.", dependency = |node: &Self| node.kind() == MediaKind::Video);
        sender: String = String::new() (label = "Spout sender");
        ndi_sender: String = String::new() (label = "NDI sender",
            dependency = |node: &Self| node.kind() == MediaKind::Ndi);
        sender_choice: Enum = "" (label = "Sender", dependency = |node: &Self| node.kind().has_sender_selection(),
            enum_domain = golden_core::parameter::ParameterEnumDomainPolicy::Open,
            enum_options = self.sender_options.clone());
        sender_external_only: bool = false (label = "External sender default", read_only = true,
            persist_read_only_value = true, show_in_nested_inspector = false, show_in_inspector_content = false,
            dependency = |node: &Self| node.kind().has_sender_selection());
        output_kind: Enum = "Final" (label = "Matrix output", read_only = true, persist_read_only_value = true, enum_options = crate::parameters::enum_options(&["Final", "Layer", "Group"]));
        output_item: NodeReference = NodeReference::empty() (label = "Output item", read_only = true, persist_read_only_value = true, reference_target_kind = ReferenceTargetKind::AnyNode, reference_allowed_node_types = vec!["mapgyver.matrix.layer.v4".into(), "mapgyver.matrix.group.v3".into()]);
        folder(video_audio, label = "Audio", enabled = true, can_be_disabled = true) {
            video_audio_track: Enum = super::video::audio_tracks::AUTOMATIC (
                label = "Track", description = "Embedded audio from this channel's Video File. Automatic follows the file's default. A missing selected track is silent. Separate loop files use Automatic.",
                enum_domain = golden_core::parameter::ParameterEnumDomainPolicy::Open,
                enum_options = super::video::audio_tracks::default_options(), dependency = |node: &Self| node.kind() == MediaKind::Video);
            audio_left: Enum = crate::audio::voice::LEFT (label = "Left output", enum_domain = golden_core::parameter::ParameterEnumDomainPolicy::Open,
                enum_options = crate::audio::voice::default_options(), dependency = |node: &Self| matches!(node.kind(), MediaKind::Video | MediaKind::Ndi));
            audio_right: Enum = crate::audio::voice::RIGHT (label = "Right output", enum_domain = golden_core::parameter::ParameterEnumDomainPolicy::Open,
                enum_options = crate::audio::voice::default_options(), dependency = |node: &Self| matches!(node.kind(), MediaKind::Video | MediaKind::Ndi));
            audio_gain: f64 = 0.0 [-120.0..24.0] (label = "Volume", unit = "dB", description = "Audio gain for this channel.",
                dependency = |node: &Self| matches!(node.kind(), MediaKind::Video | MediaKind::Ndi));
            video_loop_audio: File = File::default() (label = "Loop Audio File", description = "Optional audio played from the start of the repeat section through this channel's outputs.", dependency = |node: &Self| node.kind() == MediaKind::Video);
        }
        folder(video_subtitles, label = "Subtitles", description = "Subtitle settings for this Video channel.", enabled = false, can_be_disabled = true) {
            video_subtitle_track: Enum = "None" (
                label = "Track", description = "Choose None, a separate subtitle File, or a track embedded in this channel's Video File.",
                enum_domain = golden_core::parameter::ParameterEnumDomainPolicy::Open,
                enum_options = super::video::protocol::subtitle_track_options(&[]),
                dependency = |node: &Self| node.kind() == MediaKind::Video);
            video_subtitle_file: File = File::default() (
                label = "File", description = "UTF-8 SubRip (.srt) or WebVTT (.vtt), relative to the project or an absolute path.",
                file_allowed_extensions = vec!["srt".into(), "vtt".into()],
                dependency = |node: &Self| node.kind() == MediaKind::Video);
            video_subtitle_size: f64 = 5.0 [1.0..25.0] (
                label = "Size", description = "Font size as a percentage of source height.", unit = "%",
                constraint_policy = "reject", dependency = |node: &Self| node.kind() == MediaKind::Video);
            video_subtitle_font: String = "sans-serif".to_string() (
                label = "Font", description = "Font family. An available system font is used if this family is missing.",
                dependency = |node: &Self| node.kind() == MediaKind::Video);
            video_subtitle_position: Vec2 = Vec2::new(0.5, 0.9) [(0.0, 0.0)..(1.0, 1.0)] (
                label = "Positioning", description = "Normalized horizontal center and bottom edge, from the top left.",
                constraint_policy = "reject", dependency = |node: &Self| node.kind() == MediaKind::Video);
        }
    }
)]
pub struct ChannelNode {
    #[state(default = super::sender_catalog::Snapshot::default().options)]
    sender_options: golden_core::parameter::ParameterEnumOptions,
    #[state(default = RevisionSource::default())]
    revision: RevisionSource,
    #[state(default = RevisionSource::default())]
    source_lifecycle: RevisionSource,
    #[state(default = 0)]
    picture_reload: u64,
    #[state(default = OnceLock::new())]
    matrix_playback: OnceLock<crate::matrix::playback::PlaybackControl>,
    #[state(default = None)]
    video_binding: Option<Arc<super::video::bindings::Registration>>,
    #[state(default = OnceLock::new())]
    ndi_audio: OnceLock<Arc<crate::audio::voice::Binding>>,
    #[state(default = Default::default())]
    video_audio_option_ids: [Option<NodeUuid>; 3],
    #[state(default = None)]
    video_subtitle_binding: Option<(NodeId, NodeUuid)>,
    #[state(default = Default::default())]
    video_subtitle_tracks: Arc<[VideoSubtitleTrackInfo]>,
}
impl ChannelNode {
    fn register_video(&mut self, ctx: &mut ProcessCtx) {
        if let Some(binding) = self.video_binding.take() {
            binding.release();
        }
        if self.kind() == super::protocol::MediaKind::Video {
            let binding = super::video::bindings::Registration::new(
                super::protocol::ChannelId(self.node_data().meta.uuid),
                super::assets::parse_file(self.file.get_ref().as_str()).expect("validated Video File"),
                self.video_loop_files().expect("validated Video loop files"),
            );
            let watch = ctx
                .node_waker()
                .map_err(|error| error.to_string())
                .and_then(|waker| binding.watch_subtitle_tracks(waker));
            match watch {
                Ok(()) => ctx.clear_node_warning(self.id(), Some("video-subtitle-tracks")),
                Err(error) => ctx.set_node_warning_with(self.id(), Some("video-subtitle-tracks"), error, None),
            }
            if let Some(parent) = self.node_data().parent {
                binding.announce(ctx, self.id(), parent);
            }
            self.video_binding = Some(binding);
            self.attach_video_audio(ctx);
            self.publish_video_subtitles(ctx);
            self.refresh_subtitle_tracks(ctx);
            self.refresh_audio_options(ctx);
        }
    }

    fn video_loop_files(&self) -> Result<super::video::bindings::LoopFiles, String> {
        Ok(super::video::bindings::LoopFiles {
            picture: super::assets::parse_file(self.video_loop_file.get_ref().as_str())?,
            audio: super::assets::parse_file(self.video_loop_audio.get_ref().as_str())?,
        })
    }
    pub(super) fn initialize_sender_defaults(
        &mut self,
        options: golden_core::parameter::ParameterEnumOptions,
        external_only: bool,
    ) {
        assert!(
            !self.sender_choice.is_bound(),
            "sender defaults require a detached channel"
        );
        self.sender_options = options;
        self.sender_external_only = ParameterHandle::new(external_only);
    }
    pub(super) fn sender_external_only(&self) -> bool {
        self.sender_external_only.get()
    }
    pub(crate) fn sender_parameter(&self, mode: super::protocol::SenderSelectionMode) -> NodeId {
        match mode {
            super::protocol::SenderSelectionMode::Manual if self.kind() == MediaKind::Ndi => self.ndi_sender.id(),
            super::protocol::SenderSelectionMode::Manual => self.sender.id(),
            super::protocol::SenderSelectionMode::Auto => self.sender_choice.id(),
        }
    }
    pub(crate) fn sender_name(&self, mode: super::protocol::SenderSelectionMode) -> String {
        match mode {
            super::protocol::SenderSelectionMode::Manual if self.kind() == MediaKind::Ndi => {
                self.ndi_sender.get_ref().clone()
            }
            super::protocol::SenderSelectionMode::Manual => self.sender.get_ref().clone(),
            super::protocol::SenderSelectionMode::Auto => self.sender_choice.get_ref().as_str().to_owned(),
        }
    }
    pub(crate) fn from_definition(label: String, definition: ChannelDefinition) -> Self {
        let mut node = Self::new();
        let presentation = node.node_data().meta.presentation.clone();
        node.node_data_mut().meta = golden_core::node::NodeMeta::new(label).with_presentation(presentation);
        if definition.media_kind().has_authored_channel_variants() {
            let permissions = &mut node.node_data_mut().meta.user_permissions;
            permissions.can_edit_name = true;
            permissions.can_remove_and_duplicate = true;
        }
        macro_rules! seed {
            ($field:ident, $value:expr) => {
                node.$field = ParameterHandle::new($value);
            };
        }
        seed!(
            source_kind,
            Enum::new(super::media_node::kind_name(definition.media_kind()))
        );
        match definition.source {
            ChannelSource::Reference { .. } => {
                unreachable!("Reference outputs use an identity-only node")
            }
            ChannelSource::SolidColor { rgba } => {
                seed!(
                    rgba,
                    Color::new(rgba[0].into(), rgba[1].into(), rgba[2].into(), rgba[3].into())
                );
            }
            ChannelSource::Picture { file } | ChannelSource::Video { file } => {
                seed!(
                    file,
                    File::new(file.as_ref().map(super::assets::locator_path).unwrap_or_default())
                );
            }
            ChannelSource::Composition { .. } => {}
            ChannelSource::Spout { sender } => {
                let name = sender.unwrap_or_default();
                seed!(sender, name.clone());
                seed!(sender_choice, Enum::new(name));
            }
            ChannelSource::Ndi { sender } => {
                let name = sender.unwrap_or_default();
                seed!(ndi_sender, name.clone());
                seed!(sender_choice, Enum::new(name));
            }
            ChannelSource::Matrix { output } => match output {
                MatrixOutput::Final { .. } => {
                    let _ = node.matrix_playback.set(Default::default());
                }
                MatrixOutput::Layer { layer } => {
                    seed!(output_kind, Enum::new("Layer"));
                    seed!(output_item, NodeReference::new(layer.0));
                }
                MatrixOutput::Group { group } => {
                    seed!(output_kind, Enum::new("Group"));
                    seed!(output_item, NodeReference::new(group.0));
                }
            },
        }
        node
    }
    pub(crate) fn kind(&self) -> super::protocol::MediaKind {
        super::media_node::parse_kind(self.source_kind.get_ref().as_str())
    }
    pub(crate) fn is_matrix_final(&self) -> bool {
        self.kind() == super::protocol::MediaKind::Matrix && self.output_kind.get_ref().as_str() == "Final"
    }
    pub(crate) fn matrix_item(&self) -> Option<crate::matrix::protocol::MatrixItemId> {
        if self.kind() != super::protocol::MediaKind::Matrix {
            return None;
        }
        match self.output_kind.get_ref().as_str() {
            "Layer" => Some(crate::matrix::protocol::MatrixItemId::Layer {
                id: LayerId(self.output_item.get_ref().uuid),
            }),
            "Group" => Some(crate::matrix::protocol::MatrixItemId::Group {
                id: GroupId(self.output_item.get_ref().uuid),
            }),
            _ => None,
        }
    }
    pub(crate) fn definition(&self, media: &super::MediaNode, engine: &crate::app::AppEngine) -> ChannelDefinition {
        let size = media.size_policy(engine);
        let canvas = match size {
            SizePolicy::Automatic => super::resolution::default_canvas(),
            SizePolicy::Fixed { extent } => extent,
        };
        let settings = media.matrix_settings();
        let source = match self.source_kind.get_ref().as_str() {
            "Video" => ChannelSource::Video {
                file: super::assets::parse_file(self.file.get_ref().as_str()).expect("validated Video File"),
            },
            "Picture" => ChannelSource::Picture {
                file: super::assets::parse_file(self.file.get_ref().as_str()).expect("validated Picture File"),
            },
            "Composition" => ChannelSource::Composition { canvas },
            kind @ ("SharedTexture" | "Ndi") => {
                let name = self.sender_name(media.sender_selection().unwrap());
                let sender = (!name.is_empty()).then_some(name);
                if kind == "Ndi" {
                    ChannelSource::Ndi { sender }
                } else {
                    ChannelSource::Spout { sender }
                }
            }
            "Matrix" => ChannelSource::Matrix {
                output: match self.output_kind.get_ref().as_str() {
                    "Layer" => MatrixOutput::Layer {
                        layer: LayerId(self.output_item.get_ref().uuid),
                    },
                    "Group" => MatrixOutput::Group {
                        group: GroupId(self.output_item.get_ref().uuid),
                    },
                    _ => MatrixOutput::Final {
                        definition: MatrixDefinition {
                            canvas,
                            empty_cell: settings.empty_cell,
                        },
                    },
                },
            },
            _ => {
                let c = self.rgba.get();
                ChannelSource::SolidColor {
                    rgba: [c.red as f32, c.green as f32, c.blue as f32, c.alpha as f32],
                }
            }
        };
        ChannelDefinition {
            source,
            size,
            fit_mode: media.fit_mode(),
        }
    }
    pub(crate) fn parameter_edits(
        &self,
        source: ChannelSource,
        mode: super::protocol::SenderSelectionMode,
    ) -> Result<Vec<Edit>, String> {
        let mut values = Vec::new();
        match source {
            ChannelSource::SolidColor { rgba } => values.push((
                self.rgba.id(),
                ParamValue::Color(rgba[0].into(), rgba[1].into(), rgba[2].into(), rgba[3].into()),
            )),
            ChannelSource::Picture { file } | ChannelSource::Video { file } => values.push((
                self.file.id(),
                ParamValue::File(
                    file.as_ref()
                        .map(super::assets::locator_path)
                        .unwrap_or_default()
                        .to_owned(),
                ),
            )),
            ChannelSource::Reference { .. } | ChannelSource::Composition { .. } | ChannelSource::Matrix { .. } => {
                return Err("derived outputs have no authored channel settings".into());
            }
            ChannelSource::Spout { sender } | ChannelSource::Ndi { sender } => {
                let sender = sender.unwrap_or_default();
                values.push((
                    self.sender_parameter(mode),
                    match mode {
                        super::protocol::SenderSelectionMode::Manual => ParamValue::Str(sender),
                        super::protocol::SenderSelectionMode::Auto => ParamValue::Enum(sender),
                    },
                ))
            }
        }
        Ok(values
            .into_iter()
            .map(|(node, value)| Edit::SetParam {
                node,
                value,
                behaviour: ParameterEventBehaviour::Append,
            })
            .collect())
    }
    pub(crate) fn guard(&self) -> RevisionGuard {
        let reader = self.revision.reader();
        reader.guard_at(reader.current())
    }
    pub(crate) fn route_guard(&self) -> RevisionGuard {
        let reader = self.source_lifecycle.reader();
        reader.guard_at(reader.current())
    }
    pub(super) fn file_parameter(&self) -> NodeId {
        self.file.id()
    }
    pub(crate) fn picture_reload_generation(&self) -> u64 {
        self.picture_reload
    }
    pub(super) fn reload_picture(&mut self) {
        self.picture_reload = self
            .picture_reload
            .checked_add(1)
            .expect("Picture reload generation exhausted");
        self.revision.advance();
    }
    pub(crate) fn matrix_playback(&self) -> Option<&crate::matrix::playback::PlaybackControl> {
        (self.source_kind.get_ref().as_str() == "Matrix" && self.output_kind.get_ref().as_str() == "Final")
            .then(|| self.matrix_playback.get_or_init(Default::default))
    }
    fn invalidate(&self) {
        self.revision.advance();
        if let Some(control) = self.matrix_playback.get() {
            control.invalidate();
            control.binding().invalidate_videos();
            control.invalidate_applications();
        }
    }
}
#[node("mapgyver.media.channel.v2", from_struct)]
impl Node for ChannelNode {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        if matches!(self.kind(), MediaKind::Video | MediaKind::Ndi) {
            ctx.add_event_listener_subtree(self.id(), self.id(), 3);
        }
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: golden_core::node::NodeMetaPatch) {
        if patch.enabled.is_some() && self.video_subtitle_binding.is_some_and(|(folder, _)| node == folder) {
            self.publish_video_subtitles(ctx);
        }
        if patch.enabled.is_some() {
            self.publish_audio(ctx);
        }
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _: golden_core::node::NodeCreationContext) {
        #[cfg(not(all(windows, target_arch = "x86_64", feature = "native-video")))]
        if self.kind() == MediaKind::Ndi {
            ctx.set_node_warning_with(
                self.id(),
                Some("ndi-discovery"),
                "NDI requires the Windows x64 native video runtime",
                None,
            );
        }
        self.bind_audio_options(ctx);
        self.bind_subtitles(ctx);
        self.register_video(ctx);
        if self.kind() == MediaKind::Ndi {
            self.register_ndi_audio(ctx);
        }
        if let Some(control) = self.matrix_playback() {
            #[cfg(feature = "video-files")]
            {
                let id = self.node_data().meta.uuid.0;
                let audio = control.audio_reader();
                ctx.register_ancestor_runtime(self.id(), move |owner| {
                    let Some(manager) = owner.as_any().downcast_ref::<crate::media::MediaManagerNode>() else {
                        return Ok(false);
                    };
                    manager.matrix_audio.register_matrix(id, audio.clone());
                    Ok(true)
                });
            }
            match ctx.node_waker() {
                Ok(waker) => {
                    if let Err(error) = control.bind_video_waker(waker, self.node_data().effective_enabled) {
                        ctx.set_node_warning(self.id(), error);
                    }
                }
                Err(error) => ctx.set_node_warning(self.id(), error.to_string()),
            }
        }
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.refresh_audio_options(ctx);
        self.refresh_subtitle_tracks(ctx);
        if let Some(control) = self.matrix_playback() {
            if let Some(error) = control.service_videos() {
                ctx.set_node_warning_with(self.id(), Some("video-policy"), error, None);
            } else {
                ctx.clear_node_warning(self.id(), Some("video-policy"));
            }
            control.request_video_refresh(ctx, self.id(), self.node_data().meta.uuid);
            control.request_scheduled_navigation(ctx, self.id(), self.node_data().meta.uuid);
        }
    }
    fn worker_wake_when_disabled(&self) -> bool {
        true
    }
    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        if let Some(control) = self.matrix_playback() {
            control.video_refresh_result(ctx, self.id(), &event);
            control.scheduled_navigation_result(ctx, self.id(), &event);
        }
    }
    fn on_child_moved(&mut self, ctx: &mut ProcessCtx, child: NodeId, old_parent: NodeId, new_parent: NodeId) {
        if child == self.id() && old_parent != new_parent {
            self.register_video(ctx);
        }
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn on_structure_changed(&mut self, ctx: &mut ProcessCtx) {
        use golden_core::events::EventKind;
        // Params DSL observes two child levels to bind its own folder. That must
        // not broaden logical application membership to descendant Matrix cells.
        let owner = self.id();
        if ctx.events.iter().any(|event| match event.kind {
            EventKind::ChildAdded { parent, .. }
            | EventKind::ChildRemoved { parent, .. }
            | EventKind::ChildReplaced { parent, .. }
            | EventKind::ChildReordered { parent, .. } => parent == owner,
            EventKind::ChildMoved {
                old_parent, new_parent, ..
            } => old_parent == owner || new_parent == owner,
            EventKind::NodeCreated { node } | EventKind::NodeDeleted { node } => node == owner,
            _ => false,
        }) {
            self.invalidate();
        }
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, parameter: NodeId, _: ParamValue) {
        if self.is_audio_parameter(parameter) {
            self.publish_audio(ctx);
            if parameter != self.audio_gain.id() {
                self.refresh_audio_options(ctx);
            }
            return;
        }
        if self.is_subtitle_parameter(parameter) {
            if parameter == self.video_subtitle_track.id() {
                self.publish_subtitle_options(ctx);
            }
            self.publish_video_subtitles(ctx);
            return;
        }
        if parameter == self.file.id()
            && let Some(binding) = &self.video_binding
        {
            if let Err(error) =
                binding.set_file(super::assets::parse_file(self.file.get_ref().as_str()).expect("validated Video File"))
            {
                ctx.set_node_warning(self.id(), error);
            }
            self.refresh_subtitle_tracks(ctx);
        }
        if [self.video_loop_file.id(), self.video_loop_audio.id()].contains(&parameter)
            && let Some(binding) = &self.video_binding
            && let Err(error) = self.video_loop_files().and_then(|files| binding.set_loop_files(files))
        {
            ctx.set_node_warning(self.id(), error);
        }
        if [
            self.source_kind.id(),
            self.rgba.id(),
            self.file.id(),
            self.video_loop_file.id(),
            self.video_loop_audio.id(),
            self.sender.id(),
            self.ndi_sender.id(),
            self.output_kind.id(),
            self.output_item.id(),
        ]
        .contains(&parameter)
            || (self.kind().has_sender_selection()
                && [self.sender_choice.id(), self.sender_external_only.id()].contains(&parameter))
        {
            self.revision.advance();
            if let Some(control) = self.matrix_playback.get() {
                control.invalidate();
            }
        }
    }
    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.source_lifecycle.advance();
        self.invalidate();
        self.publish_audio(ctx);
        if enabled {
            self.publish_video_subtitles(ctx);
        }
        if let Some(control) = self.matrix_playback() {
            control.video_enabled(enabled);
        }
    }
    fn destroy(&mut self, ctx: &mut ProcessCtx) {
        self.source_lifecycle.advance();
        if let Some(binding) = self.ndi_audio.take() {
            binding.retire();
        }
        if let Some(binding) = self.video_binding.take() {
            if self.node_data().parent.is_some_and(|parent| ctx.is_destroying(parent)) {
                binding.retire_media();
            }
            // History may retain the node after destroy; release membership now.
            binding.release();
        }
        if let Some(control) = self.matrix_playback.get() {
            control.clear();
        }
        self.revision.advance();
    }
    fn user_container_rules(&self) -> Option<UserContainerRules> {
        // The sparse codec builds child schema baselines before replaying Source kind.
        Some(UserContainerRules::new(&[
            crate::matrix::node::LAYER_TYPE,
            crate::matrix::node::GROUP_TYPE,
            crate::matrix::node::COLUMN_TYPE,
            crate::matrix::column_group_node::COLUMN_GROUP_TYPE,
            crate::audio::ndi_route::TYPE,
        ]))
    }
    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        if self.kind() == MediaKind::Ndi {
            let kind = crate::audio::ndi_route::TYPE;
            return vec![UserCreatableItem::new(kind, kind, "Audio Route").with_select_when_created(false)];
        }
        Vec::new()
    }
    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if self.kind() == MediaKind::Ndi && node_type == crate::audio::ndi_route::TYPE {
            return Some(Box::new(crate::audio::NdiAudioRouteNode::new()));
        }
        if self.is_matrix_final() {
            if node_type == crate::matrix::column_node::COLUMN_TYPE {
                return Some(Box::new(crate::matrix::ColumnNode::new()));
            }
            if node_type == crate::matrix::column_group_node::COLUMN_GROUP_TYPE {
                return Some(Box::new(crate::matrix::ColumnGroupNode::new()));
            }
            crate::matrix::node::create(node_type).map(|node| Box::new(node) as Box<dyn Node>)
        } else {
            None
        }
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }
    fn parameter_companion_changes(
        &self,
        change: &ParameterValidationContext<'_>,
    ) -> Result<Vec<(NodeId, ParamValue)>, String> {
        if !self.sender_external_only()
            || ![
                self.sender_parameter(SenderSelectionMode::Manual),
                self.sender_choice.id(),
            ]
            .contains(&change.parameter)
        {
            return Ok(Vec::new());
        }
        let Some(media) = self
            .node_data()
            .parent
            .and_then(|id| change.node(id))
            .and_then(|node| node.as_any().downcast_ref::<super::MediaNode>())
        else {
            return Ok(Vec::new());
        };
        let mode = match media
            .sender_selection_parameter()
            .and_then(|id| change.proposed_value(id))
        {
            Some(ParamValue::Enum(value)) => SenderSelectionMode::parse(value)?,
            _ => media.sender_selection().ok_or("sender mode unavailable")?,
        };
        // Mode copies are derived, so only an explicit write to the active selector clears this intent.
        if change.parameter == self.sender_parameter(mode) {
            Ok(vec![(self.sender_external_only.id(), ParamValue::Bool(false))])
        } else {
            Ok(Vec::new())
        }
    }
    fn engine_validate_project_candidate(&self) -> Result<(), String> {
        super::assets::parse_file(self.file.get_ref().as_str())?;
        self.video_loop_files()?;
        if self.kind() == MediaKind::Video {
            self.subtitle_settings(false)?.validate()?;
        }
        for (value, allowed) in [
            (
                self.source_kind.get_ref().as_str(),
                &[
                    "Picture",
                    "Video",
                    "SolidColor",
                    "SharedTexture",
                    "Ndi",
                    "Composition",
                    "Matrix",
                ][..],
            ),
            (self.output_kind.get_ref().as_str(), &["Final", "Layer", "Group"][..]),
        ] {
            if !allowed.contains(&value) {
                return Err(format!("unknown channel setting '{value}'"));
            }
        }
        if self.kind() == super::protocol::MediaKind::SolidColor {
            let color = self.rgba.get();
            golden_video::LinearColor::from_straight(
                color.red as f32,
                color.green as f32,
                color.blue as f32,
                color.alpha as f32,
            )
            .map_err(|error| error.to_string())?;
        }
        if self.sender.get_ref().len() > 255 || self.sender.get_ref().contains('\0') {
            return Err("Spout selector must contain at most 255 UTF-8 bytes without NUL".into());
        }
        if self.kind() == super::protocol::MediaKind::SharedTexture {
            let name = self.sender_choice.get_ref().as_str();
            if name.len() > 255 || name.contains('\0') {
                return Err("Spout selector must contain at most 255 UTF-8 bytes without NUL".into());
            }
        }
        if self.kind() == MediaKind::Ndi
            && (self.ndi_sender.get_ref().contains('\0') || self.sender_choice.get_ref().as_str().contains('\0'))
        {
            return Err("NDI selector must not contain NUL".into());
        }
        Ok(())
    }

    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if self.is_audio_parameter(change.parameter) {
            return self.validate_audio_parameter(change);
        }
        if self.is_subtitle_parameter(change.parameter) {
            return self.validate_subtitle_parameter(change);
        }
        if [self.file.id(), self.video_loop_file.id(), self.video_loop_audio.id()].contains(&change.parameter)
            && let ParamValue::File(path) = change.value
        {
            super::assets::parse_file(path)?;
        }
        for (id, current) in [
            (
                self.source_kind.id(),
                ParamValue::Enum(self.source_kind.get_ref().0.clone()),
            ),
            (
                self.output_kind.id(),
                ParamValue::Enum(self.output_kind.get_ref().0.clone()),
            ),
            (
                self.output_item.id(),
                ParamValue::Reference(self.output_item.get_ref().clone()),
            ),
        ] {
            if change.parameter == id && change.value != &current {
                return Err("Channel source/output identity is owned by structural operations".into());
            }
        }
        if [self.sender.id(), self.ndi_sender.id()].contains(&change.parameter)
            || (self.kind().has_sender_selection() && change.parameter == self.sender_choice.id())
        {
            let value = match change.value {
                ParamValue::Str(value) | ParamValue::Enum(value) => value,
                _ => return Err("sender selector must be a String or Enum".into()),
            };
            let ndi = change.parameter != self.sender.id() && self.kind() == MediaKind::Ndi;
            if ndi && value.contains('\0') {
                return Err("NDI selector must not contain NUL".into());
            }
            if !ndi && (value.len() > 255 || value.contains('\0')) {
                return Err("Spout selector must contain at most 255 UTF-8 bytes without NUL".into());
            }
        }
        Ok(())
    }
}
