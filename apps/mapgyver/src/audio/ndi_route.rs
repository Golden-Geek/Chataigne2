//! Typed routes for NDI inputs beyond the channel's built-in Left and Right.
use super::voice;
use crate::{
    app::AppEngine,
    media::{ChannelNode, protocol::MediaKind},
};
use golden_audio::AudioChannelId;
use golden_core::{
    edit::Edit,
    engine::{NodeExecutionRule, NodeWaker},
    node,
    node::{Node, NodeCreationContext, NodeId, NodeUuid, ParameterValidationContext},
    parameter::{Enum, ParamValue, ParameterEnumDomainPolicy, ParameterEnumOptions},
    process_ctx::ProcessCtx,
};
use std::sync::{Arc, Mutex};

pub(crate) const TYPE: &str = "mapgyver.ndi.audio_route";
type Route = (usize, AudioChannelId);

#[derive(Default)]
struct RouteState {
    route: Option<Route>,
    options: ParameterEnumOptions,
}

pub(crate) struct RouteBinding {
    state: Mutex<RouteState>,
    waker: Option<NodeWaker>,
}

impl RouteBinding {
    pub(super) fn update_options(&self, options: &ParameterEnumOptions) {
        let mut state = self.state.lock().unwrap();
        if state.options == *options {
            return;
        }
        state.options = options.clone();
        drop(state);
        if let Some(waker) = &self.waker {
            let _ = waker.wake();
        }
    }

    fn set_route(&self, route: Option<Route>) {
        let mut state = self.state.lock().unwrap();
        if state.route != route {
            state.route = route;
        }
    }

    #[cfg(all(windows, target_arch = "x86_64", feature = "video-files"))]
    pub(super) fn route(&self) -> Option<Route> {
        self.state.lock().unwrap().route
    }
}

#[node("mapgyver.ndi.audio_route", label = "Audio Route", collapsed = false, can_be_disabled = true,
    user_permissions = golden_core::node::NodeUserPermissions {
        can_edit_name: true, can_remove_and_duplicate: true, ..Default::default()
    })]
#[children(
    source_channel: i32 = 3 [3..256] (label = "Input channel", constraint_policy = "reject");
    destination: Enum = voice::UNASSIGNED (label = "Virtual output",
        enum_domain = ParameterEnumDomainPolicy::Open, enum_options = voice::default_options());
)]
pub struct NdiAudioRouteNode {
    #[state(default = None)]
    binding: Option<Arc<RouteBinding>>,
    #[state(default = None)]
    destination_uuid: Option<NodeUuid>,
}

impl NdiAudioRouteNode {
    pub(crate) fn validate(&self, engine: &AppEngine) -> Result<(), String> {
        let parameters = [self.source_channel.id(), self.destination.id()];
        let mut child = self.node_data().first_child;
        while let Some(id) = child {
            let node = engine.nodes.get(id).ok_or("NDI audio route Parameter is missing")?;
            if !parameters.contains(&id) {
                return Err("an NDI audio route contains only its declared Parameters".into());
            }
            child = node.node_data().next_sibling;
        }
        if !(3..=256).contains(&self.source_channel.get()) {
            return Err("NDI additional input channel must be between 3 and 256".into());
        }
        voice::parse_route(self.destination.get_ref().as_str()).map(|_| ())
    }

    fn publish(&self) {
        if let Some(binding) = &self.binding {
            let input = self.source_channel.get();
            let destination = voice::parse_route(self.destination.get_ref().as_str()).ok().flatten();
            binding.set_route(
                destination
                    .filter(|_| self.node_data().effective_enabled && (3..=256).contains(&input))
                    .map(|destination| (input as usize - 1, destination)),
            );
        }
    }
}

#[node("mapgyver.ndi.audio_route", from_struct)]
impl Node for NdiAudioRouteNode {
    fn needs_update(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive()
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _: NodeCreationContext) {
        self.destination_uuid = ctx
            .tree_snapshot()
            .and_then(|tree| tree.node(self.destination.id()))
            .map(|node| node.uuid);
        let binding = Arc::new(RouteBinding {
            state: Mutex::new(RouteState {
                options: voice::default_options(),
                ..Default::default()
            }),
            waker: ctx.node_waker().ok(),
        });
        self.binding = Some(binding.clone());
        self.publish();
        ctx.register_ancestor_runtime(self.id(), move |owner| {
            let Some(channel) = owner.as_any().downcast_ref::<ChannelNode>() else {
                return Ok(false);
            };
            if channel.kind() != MediaKind::Ndi {
                return Err("additional routes require an NDI channel".into());
            }
            channel.ndi_audio_binding().register_ndi_route(&binding);
            Ok(true)
        });
        self.on_worker_wake(ctx);
    }

    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        let (Some(binding), Some(uuid)) = (&self.binding, self.destination_uuid) else {
            return;
        };
        let choice = self.destination.get_ref().as_str();
        let options = voice::option_set(&binding.state.lock().unwrap().options, choice);
        ctx.edits.push_from(
            self.id(),
            Edit::SetParamEnumOptions {
                node: self.destination.id(),
                expected_uuid: uuid,
                expected_value: ParamValue::Enum(choice.into()),
                selected_value: None,
                options,
            },
        );
    }

    fn worker_wake_when_disabled(&self) -> bool {
        true
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, parameter: NodeId, _: ParamValue) {
        self.publish();
        if parameter == self.destination.id() {
            self.on_worker_wake(ctx);
        }
    }

    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, _: bool) {
        self.publish();
    }

    fn destroy(&mut self, _: &mut ProcessCtx) {
        if let Some(binding) = self.binding.take() {
            binding.set_route(None);
        }
    }

    fn project_decode_data(&mut self, data: &serde_json::Value) -> Result<(), String> {
        crate::parameters::reject_opaque(data)
    }

    fn validate_parameter_change(&self, change: &ParameterValidationContext<'_>) -> Result<(), String> {
        if change.parameter == self.destination.id()
            && let ParamValue::Enum(value) = change.value
        {
            voice::parse_route(value)?;
        }
        Ok(())
    }
}
