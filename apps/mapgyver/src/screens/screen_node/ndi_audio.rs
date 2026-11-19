use std::sync::Arc;

use golden_core::{
    edit::Edit,
    node::{Node, NodeId, ParameterHandle},
    parameter::{Enum, ParamValue},
    process_ctx::ProcessCtx,
};

use crate::audio::voice::{self, Routing};

use super::ScreenNode;

#[derive(Default)]
pub(crate) struct ScreenAudio {
    pub routing: voice::Binding,
    #[cfg(all(windows, target_arch = "x86_64", feature = "video-files"))]
    pub stream: golden_video::ndi::NdiOutputAudio,
}

impl ScreenNode {
    pub(crate) fn is_ndi(&self) -> bool {
        self.target_kind.get_ref().as_str() == "NDI"
    }

    pub(crate) fn ndi_audio(&self) -> &Arc<ScreenAudio> {
        self.ndi_audio_binding.get_or_init(Default::default)
    }

    fn ndi_audio_parameters(&self) -> [&ParameterHandle<Enum>; 2] {
        [&self.ndi_left, &self.ndi_right]
    }

    pub(super) fn is_ndi_audio_parameter(&self, parameter: NodeId) -> bool {
        self.ndi_audio_parameters()
            .iter()
            .any(|handle| handle.id() == parameter)
    }

    pub(super) fn bind_ndi_audio(&mut self, ctx: &mut ProcessCtx) {
        self.ndi_audio_option_ids = self.ndi_audio_parameters().map(|handle| {
            ctx.tree_snapshot()
                .and_then(|tree| tree.node(handle.id()))
                .map(|node| node.uuid)
        });
        self.ndi_audio().routing.watch_options(ctx.node_waker().ok());
        self.publish_ndi_audio();
        self.advertise_ndi_audio(ctx);
    }

    pub(super) fn advertise_ndi_audio(&self, ctx: &mut ProcessCtx) {
        #[cfg(all(windows, target_arch = "x86_64", feature = "video-files"))]
        if self.is_ndi() {
            let audio = Arc::downgrade(self.ndi_audio());
            let uuid = self.node_data().meta.uuid;
            ctx.register_ancestor_runtime(self.id(), move |owner| {
                let Some(manager) = owner.as_any().downcast_ref::<crate::screens::ScreenManagerNode>() else {
                    return Ok(false);
                };
                manager.ndi_audio.0.lock().unwrap().insert(uuid, audio.clone());
                Ok(true)
            });
        }
        #[cfg(not(all(windows, target_arch = "x86_64", feature = "video-files")))]
        let _ = ctx;
    }

    pub(super) fn publish_ndi_audio(&self) {
        let destinations = self
            .ndi_audio_parameters()
            .map(|handle| voice::parse_route(handle.get_ref().as_str()).ok().flatten());
        self.ndi_audio().routing.update_routing(Routing {
            enabled: self.is_ndi() && self.node_data().effective_enabled,
            destinations,
            gain: golden_audio::GainDb::UNITY,
        });
    }

    pub(super) fn refresh_ndi_audio_options(&self, ctx: &mut ProcessCtx) {
        let options = self
            .ndi_audio()
            .routing
            .options()
            .unwrap_or_else(voice::default_options);
        for (handle, uuid) in self.ndi_audio_parameters().into_iter().zip(self.ndi_audio_option_ids) {
            let Some(expected_uuid) = uuid else { continue };
            let choice = handle.get_ref().as_str();
            ctx.edits.push_from(
                self.id(),
                Edit::SetParamEnumOptions {
                    node: handle.id(),
                    expected_uuid,
                    expected_value: ParamValue::Enum(choice.into()),
                    selected_value: None,
                    options: voice::option_set(&options, choice),
                },
            );
        }
    }
}

#[cfg(all(windows, target_arch = "x86_64", feature = "video-files"))]
#[derive(Default)]
pub(crate) struct Registry(
    std::sync::Mutex<std::collections::HashMap<golden_core::node::NodeUuid, std::sync::Weak<ScreenAudio>>>,
);

#[cfg(all(windows, target_arch = "x86_64", feature = "video-files"))]
impl Registry {
    pub fn capture(engine: &crate::app::AppEngine) -> Option<Arc<Self>> {
        let id = crate::screens::authoring::manager(engine).ok()?;
        let crate::app::AppNode::ScreenManager(manager) = engine.nodes.get(id)? else {
            return None;
        };
        Some(manager.ndi_audio.clone())
    }

    pub fn owners(&self) -> Vec<Arc<ScreenAudio>> {
        let mut owners = self.0.lock().unwrap();
        owners.retain(|_, owner| owner.strong_count() != 0);
        owners.values().filter_map(std::sync::Weak::upgrade).collect()
    }
}
