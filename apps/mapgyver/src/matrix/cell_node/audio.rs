use super::CellNode;
use crate::{
    audio::voice,
    matrix::audio::CellAudio,
    media::{locator_path, parse_file},
};
use golden_core::{
    edit::Edit,
    node::{Node, NodeId, ParameterValidationContext},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

impl CellNode {
    pub(super) fn audio_settings(&self) -> Result<CellAudio, String> {
        let override_media = self.audio_mode.get_ref().as_str() == "File";
        Ok(CellAudio {
            enabled: self.node_data().effective_enabled,
            override_media,
            file: parse_file(self.audio_file.get_ref().as_str())?,
            routing: voice::Routing {
                enabled: override_media,
                destinations: [
                    voice::parse_route(self.audio_left.get_ref().as_str())?,
                    voice::parse_route(self.audio_right.get_ref().as_str())?,
                ],
                gain: golden_audio::GainDb::new(self.audio_gain.get() as f32).map_err(|err| err.to_string())?,
            },
        })
    }

    pub(in crate::matrix) fn audio_binding(&self) -> std::sync::Arc<crate::matrix::audio::CellBinding> {
        self.audio_binding.clone()
    }

    pub(in crate::matrix) fn audio_label(&self) -> Option<String> {
        let settings = self.audio_settings().ok()?;
        if !settings.override_media {
            return None;
        }
        let path = locator_path(settings.file.as_ref()?);
        std::path::Path::new(path).file_name()?.to_str().map(str::to_owned)
    }

    pub(super) fn is_audio_parameter(&self, parameter: NodeId) -> bool {
        [
            self.audio_mode.id(),
            self.audio_file.id(),
            self.audio_left.id(),
            self.audio_right.id(),
            self.audio_gain.id(),
        ]
        .contains(&parameter)
    }

    pub(super) fn validate_audio_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if change.parameter == self.audio_file.id()
            && let ParamValue::File(path) = change.value
        {
            parse_file(path)?;
        }
        if [self.audio_left.id(), self.audio_right.id()].contains(&change.parameter)
            && let ParamValue::Enum(choice) = change.value
        {
            voice::parse_route(choice)?;
        }
        Ok(())
    }

    pub(super) fn publish_audio(&self) {
        if let Ok(settings) = self.audio_settings() {
            self.audio_binding.publish(settings);
        }
    }

    pub(super) fn register_audio(&self, ctx: &mut ProcessCtx) {
        let _ = &ctx;
        #[cfg(feature = "video-files")]
        {
            self.audio_binding.options.watch_options(ctx.node_waker().ok());
            let id = self.node_data().meta.uuid.0;
            let binding = std::sync::Arc::downgrade(&self.audio_binding);
            ctx.register_ancestor_runtime(self.id(), move |owner| {
                let Some(manager) = owner.as_any().downcast_ref::<crate::media::MediaManagerNode>() else {
                    return Ok(false);
                };
                manager.matrix_audio.register_cell(id, binding.clone());
                Ok(true)
            });
        }
    }

    pub(super) fn refresh_audio_options(&self, ctx: &mut ProcessCtx) {
        let Some(options) = self.audio_binding.options.options() else {
            return;
        };
        for route in [&self.audio_left, &self.audio_right] {
            let id = route.id();
            let Some(parameter) = ctx.tree_snapshot().and_then(|tree| tree.node(id)) else {
                continue;
            };
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
                    expected_uuid: parameter.uuid,
                    expected_value: ParamValue::Enum(choice.into()),
                    selected_value: None,
                    options,
                },
            );
        }
    }
}
