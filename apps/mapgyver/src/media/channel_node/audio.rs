//! Authoring adapter for independent Video audio settings and virtual-output choices.
use super::ChannelNode;
use crate::audio::voice::{self, Routing};
use crate::media::video::audio_tracks::AudioTrack;
use golden_audio::GainDb;
use golden_core::{
    edit::Edit,
    node::{Node, NodeId, ParameterHandle, ParameterValidationContext},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};
use std::sync::Arc;

impl ChannelNode {
    fn audio_option_parameters(&self) -> [&ParameterHandle<golden_core::parameter::Enum>; 3] {
        [&self.video_audio_track, &self.audio_left, &self.audio_right]
    }

    pub(super) fn bind_audio_options(&mut self, ctx: &ProcessCtx) {
        // Keep the identities of the declared leaves so a catalog wake never needs a tree snapshot.
        self.video_audio_option_ids = self.audio_option_parameters().map(|parameter| {
            ctx.tree_snapshot()
                .and_then(|tree| tree.node(parameter.id()))
                .map(|node| node.uuid)
        });
    }

    fn audio_routing(&self) -> Result<Routing, String> {
        Ok(Routing {
            enabled: false,
            destinations: [
                voice::parse_route(self.audio_left.get_ref().as_str())?,
                voice::parse_route(self.audio_right.get_ref().as_str())?,
            ],
            gain: GainDb::new(self.audio_gain.get() as f32).map_err(|error| error.to_string())?,
        })
    }

    pub(super) fn attach_video_audio(&self, ctx: &mut ProcessCtx) {
        let waker = ctx.node_waker().ok();
        if let Some(binding) = &self.video_binding {
            binding.audio().watch_options(waker);
        }
        self.publish_audio(ctx);
    }

    fn audio_binding(&self) -> Option<&Arc<voice::Binding>> {
        self.video_binding
            .as_ref()
            .map(|video| &video.audio().voice)
            .or(self.ndi_audio.get())
    }

    pub(crate) fn ndi_audio_binding(&self) -> &Arc<voice::Binding> {
        self.ndi_audio.get_or_init(Default::default)
    }

    pub(super) fn register_ndi_audio(&self, ctx: &mut ProcessCtx) {
        let binding = self.ndi_audio_binding();
        binding.watch_options(ctx.node_waker().ok());
        #[cfg(all(windows, target_arch = "x86_64", feature = "video-files"))]
        {
            let binding = binding.clone();
            let uuid = self.node_data().meta.uuid.0;
            ctx.register_ancestor_runtime(self.id(), move |owner| {
                let Some(manager) = owner.as_any().downcast_ref::<crate::media::MediaManagerNode>() else {
                    return Ok(false);
                };
                manager.ndi_inputs.register_audio(uuid, &binding);
                Ok(true)
            });
        }
        self.publish_audio(ctx);
        self.refresh_audio_options(ctx);
    }

    pub(super) fn publish_audio(&self, ctx: &mut ProcessCtx) {
        let Some(binding) = self.audio_binding().cloned() else {
            return;
        };
        if let Some(video) = &self.video_binding
            && let Ok(track) = AudioTrack::parse(self.video_audio_track.get_ref().as_str())
        {
            video.audio().set_selection(track);
        }
        let Ok(mut routing) = self.audio_routing() else {
            return;
        };
        // Effective enablement comes from the live folder after the edit has propagated.
        ctx.register_ancestor_runtime(self.audio_gain.id(), move |folder| {
            if folder.node_data().meta.decl_id.0 != "parameters/video_audio" {
                return Ok(false);
            }
            routing.enabled = folder.node_data().effective_enabled;
            binding.update_routing(routing);
            Ok(true)
        });
    }

    pub(super) fn refresh_audio_options(&self, ctx: &mut ProcessCtx) {
        let Some(binding) = self.audio_binding() else {
            return;
        };
        let voice_options = binding.options().unwrap_or_else(voice::default_options);
        for (value, expected_uuid) in self
            .audio_option_parameters()
            .into_iter()
            .zip(self.video_audio_option_ids)
        {
            let id = value.id();
            let Some(expected_uuid) = expected_uuid else {
                continue;
            };
            let choice = value.get_ref().as_str();
            let options = if id == self.video_audio_track.id() {
                let Some(video) = &self.video_binding else { continue };
                video.audio().track_options(choice)
            } else {
                voice::option_set(&voice_options, choice)
            };
            ctx.edits.push_from(
                self.id(),
                Edit::SetParamEnumOptions {
                    node: id,
                    expected_uuid,
                    expected_value: ParamValue::Enum(choice.into()),
                    selected_value: None,
                    options,
                },
            );
        }
    }

    pub(super) fn is_audio_parameter(&self, id: NodeId) -> bool {
        [
            self.video_audio_track.id(),
            self.audio_left.id(),
            self.audio_right.id(),
            self.audio_gain.id(),
        ]
        .contains(&id)
    }

    pub(super) fn validate_audio_parameter(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        match change.value {
            ParamValue::Enum(choice) if change.parameter == self.video_audio_track.id() => {
                AudioTrack::parse(choice).map(|_| ())
            }
            ParamValue::Float(value) if change.parameter == self.audio_gain.id() => GainDb::new(*value as f32)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            ParamValue::Enum(choice) if change.parameter != self.audio_gain.id() => {
                voice::parse_route(choice).map(|_| ())
            }
            _ => Err("Invalid audio Parameter type".into()),
        }
    }
}
