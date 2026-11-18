//! The authored subtitle folder belongs to one Video channel.
use super::ChannelNode;
use crate::{
    app::AppEngine,
    media::video::protocol::{self, VideoSubtitleSettings, VideoSubtitleTrack, VideoSubtitleTrackInfo},
};
use golden_core::{
    edit::Edit,
    node::{Node, NodeId, ParameterValidationContext},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

impl ChannelNode {
    pub(crate) fn subtitle_folder(&self, engine: &AppEngine) -> Option<NodeId> {
        // The declared Track handle is bound during project reconstruction, before runtime
        // activation populates the worker binding.
        let track = engine.nodes.get(self.video_subtitle_track.id())?;
        let folder = track.node_data().parent?;
        (engine.nodes.get(folder)?.node_data().meta.decl_id.0 == "parameters/video_subtitles").then_some(folder)
    }

    pub(super) fn subtitle_settings(&self, enabled: bool) -> Result<VideoSubtitleSettings, String> {
        let position = self.video_subtitle_position.get();
        Ok(VideoSubtitleSettings {
            enabled,
            track: VideoSubtitleTrack::parse(self.video_subtitle_track.get_ref().as_str())?,
            file: self.video_subtitle_file.get_ref().as_str().into(),
            size: self.video_subtitle_size.get(),
            font: self.video_subtitle_font.get_ref().clone(),
            position: [position.x, position.y],
        })
    }

    pub(crate) fn video_subtitles(&self, engine: &AppEngine) -> Result<VideoSubtitleSettings, String> {
        let folder = self
            .subtitle_folder(engine)
            .and_then(|folder| engine.nodes.get(folder))
            .ok_or("Video Subtitles folder is not bound")?;
        self.subtitle_settings(folder.node_data().meta.enabled)
    }

    pub(crate) fn subtitle_parameter_values(&self, settings: &VideoSubtitleSettings) -> Vec<(NodeId, ParamValue)> {
        vec![
            (self.video_subtitle_track.id(), ParamValue::Enum(settings.track.key())),
            (self.video_subtitle_file.id(), ParamValue::File(settings.file.clone())),
            (self.video_subtitle_size.id(), ParamValue::Float(settings.size)),
            (self.video_subtitle_font.id(), ParamValue::Str(settings.font.clone())),
            (
                self.video_subtitle_position.id(),
                ParamValue::Vec2(settings.position[0], settings.position[1]),
            ),
        ]
    }

    pub(crate) fn video_subtitle_tracks(&self) -> &[VideoSubtitleTrackInfo] {
        &self.video_subtitle_tracks
    }

    pub(super) fn is_subtitle_parameter(&self, id: NodeId) -> bool {
        [
            self.video_subtitle_track.id(),
            self.video_subtitle_file.id(),
            self.video_subtitle_size.id(),
            self.video_subtitle_font.id(),
            self.video_subtitle_position.id(),
        ]
        .contains(&id)
    }

    pub(super) fn publish_subtitle_options(&self, ctx: &mut ProcessCtx) {
        let Some((_, expected_uuid)) = self.video_subtitle_binding else {
            return;
        };
        ctx.edits.push_from(
            self.id(),
            Edit::SetParamEnumOptions {
                node: self.video_subtitle_track.id(),
                expected_uuid,
                expected_value: ParamValue::Enum(self.video_subtitle_track.get_ref().as_str().into()),
                selected_value: None,
                options: protocol::subtitle_track_options(self.video_subtitle_tracks()),
            },
        );
    }

    pub(super) fn publish_video_subtitles(&self, ctx: &mut ProcessCtx) {
        let Some((folder, _)) = self.video_subtitle_binding else {
            return;
        };
        let Some(binding) = self.video_binding.clone() else {
            return;
        };
        let Ok(mut settings) = self.subtitle_settings(false) else {
            return;
        };
        // The folder flag is authored metadata, independent of inherited Channel enablement.
        ctx.register_ancestor_runtime(self.video_subtitle_track.id(), move |owner| {
            if owner.id() != folder {
                return Ok(false);
            }
            settings.enabled = owner.node_data().meta.enabled;
            binding.set_subtitle_settings(settings.clone())?;
            Ok(true)
        });
    }

    pub(super) fn validate_subtitle_parameter(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        let mut settings = self.subtitle_settings(false)?;
        match (change.parameter, change.value) {
            (id, ParamValue::Enum(value)) if id == self.video_subtitle_track.id() => {
                settings.track = VideoSubtitleTrack::parse(value)?;
            }
            (id, ParamValue::File(value)) if id == self.video_subtitle_file.id() => settings.file = value.clone(),
            (id, ParamValue::Float(value)) if id == self.video_subtitle_size.id() => settings.size = *value,
            (id, ParamValue::Str(value)) if id == self.video_subtitle_font.id() => settings.font = value.clone(),
            (id, ParamValue::Vec2(x, y)) if id == self.video_subtitle_position.id() => settings.position = [*x, *y],
            _ => return Err("Invalid Video subtitle Parameter type".into()),
        }
        settings.validate()
    }

    pub(super) fn bind_subtitles(&mut self, ctx: &ProcessCtx) {
        self.video_subtitle_binding = ctx
            .tree_snapshot()
            .and_then(|tree| tree.node(self.video_subtitle_track.id()))
            .and_then(|track| track.parent.map(|folder| (folder, track.uuid)));
    }

    pub(super) fn refresh_subtitle_tracks(&mut self, ctx: &mut ProcessCtx) {
        let Some(binding) = &self.video_binding else {
            return;
        };
        let tracks = binding.subtitle_tracks();
        if self.video_subtitle_tracks != tracks {
            self.video_subtitle_tracks = tracks;
            self.publish_subtitle_options(ctx);
        }
    }
}
