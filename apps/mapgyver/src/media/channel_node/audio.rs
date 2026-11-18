//! Authoring adapter for independent Video audio settings and virtual-output choices.
use super::ChannelNode;
use crate::audio::voice::{self, Routing};
use golden_audio::GainDb;
use golden_core::{
    edit::Edit,
    node::{Node, NodeId, ParameterValidationContext},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

impl ChannelNode {
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
        self.publish_video_audio(ctx);
    }

    pub(super) fn publish_video_audio(&self, ctx: &mut ProcessCtx) {
        let Some(binding) = self.video_binding.clone() else {
            return;
        };
        let Ok(mut routing) = self.audio_routing() else {
            return;
        };
        // Effective enablement comes from the live folder after the edit has propagated.
        ctx.register_ancestor_runtime(self.audio_gain.id(), move |folder| {
            if folder.node_data().meta.decl_id.0 != "parameters/video_audio" {
                return Ok(false);
            }
            routing.enabled = folder.node_data().effective_enabled;
            binding.audio().update_routing(routing);
            Ok(true)
        });
    }

    pub(super) fn refresh_audio_options(&self, ctx: &mut ProcessCtx) {
        let Some(binding) = &self.video_binding else {
            return;
        };
        let options = binding.audio().options().unwrap_or_else(voice::default_options);
        for route in [&self.audio_left, &self.audio_right] {
            let id = route.id();
            let Some(parameter) = ctx.tree_snapshot().and_then(|tree| tree.node(id)) else {
                continue;
            };
            let expected_uuid = parameter.uuid;
            let choice = route.get_ref().as_str();
            let options = voice::option_set(&options, choice);
            if parameter
                .param_constraints
                .as_ref()
                .is_some_and(|constraints| constraints.enum_options == options)
            {
                continue;
            }
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
        [self.audio_left.id(), self.audio_right.id(), self.audio_gain.id()].contains(&id)
    }

    pub(super) fn validate_audio_parameter(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        match change.value {
            ParamValue::Float(value) if change.parameter == self.audio_gain.id() => GainDb::new(*value as f32)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            ParamValue::Enum(choice) if change.parameter != self.audio_gain.id() => {
                voice::parse_route(choice).map(|_| ())
            }
            _ => Err("Invalid Video audio Parameter type".into()),
        }
    }
}
