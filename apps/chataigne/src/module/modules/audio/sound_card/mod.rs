mod choice_sync;
mod command_service;
mod connection_feedback;
mod event_delivery;
mod integration;
mod native_logging;
mod observation_warning;
mod prepared_event;
mod routes;
mod runtime;
mod script;
mod settings;
mod structure;
#[cfg(test)]
mod tests;
mod traffic_logging;

use std::time::{Duration, Instant};

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event, EventFrame, EventKind},
    node,
    node::{Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::ReconnectBackoff;

pub(crate) use crate::app::module_modules_audio_sound_card_commands::SOUND_CARD_COMMAND_TYPES;
use crate::app::module_modules_audio_sound_card_schema::{
    SoundCardInputParameters, SoundCardInputRouting, SoundCardOutputParameters, SoundCardOutputRouting,
};
use settings::NO_AUDIO_DRIVER;
pub(crate) use settings::{
    backend_options, buffer_size_options, default_audio_driver, device_options, input_device_options,
    output_device_options, sample_rate_options,
};

pub(crate) const ASIO_AUDIO_DRIVER: &str = "asio";
pub(crate) const NO_AUDIO_DEVICE: &str = "none";
pub(crate) const SYSTEM_DEFAULT_DEVICE: &str = "system_default";
pub(crate) const AUTOMATIC_CONFIGURATION: &str = "automatic";
pub(crate) const MAX_PLAYBACK_SOURCE_CHANNELS: u16 = 256;
pub(crate) const LEVELS_UPDATE_RATE_MIN_HZ: i32 = 1;
pub(crate) const LEVELS_UPDATE_RATE_DEFAULT_HZ: i32 = 5;
pub(crate) const LEVELS_UPDATE_RATE_MAX_HZ: i32 = 30;

pub(crate) const INPUT_CHANNELS_PATH: &str = "parameters/input/channels";
pub(crate) const OUTPUT_CHANNELS_PATH: &str = "parameters/output/channels";
pub(crate) const INPUT_ROUTES_PATH: &str = "connection/input_routing/routes";
pub(crate) const OUTPUT_ROUTES_PATH: &str = "connection/output_routing/routes";
pub(crate) const PITCH_VALUES_PATH: &str = "values/pitch_detection";

#[node("sound_card_module", label = "Sound Card")]
#[children(
    folder(connection) {
        [base_children];
        audio_driver: Enum = default_audio_driver() (
            label = "Audio Driver",
            enum_options = backend_options(),
            description = "Only the selected native audio driver is initialized."
        );
        device: Enum = NO_AUDIO_DEVICE (
            label = "Device",
            enum_options = device_options(),
            description = "ASIO uses one duplex driver device for both input and output.",
            dependency = audio_driver == "asio"
        );
        input_device: Enum = NO_AUDIO_DEVICE (
            label = "Input Device",
            enum_options = input_device_options(),
            dependency = audio_driver != "asio"
        );
        output_device: Enum = SYSTEM_DEFAULT_DEVICE (
            label = "Output Device",
            enum_options = output_device_options(),
            dependency = audio_driver != "asio"
        );
        sample_rate: Enum = AUTOMATIC_CONFIGURATION (
            label = "Sample Rate",
            enum_options = sample_rate_options()
        );
        buffer_size: Enum = AUTOMATIC_CONFIGURATION (
            label = "Buffer Size",
            enum_options = buffer_size_options()
        );
        node input_routing: SoundCardInputRouting = SoundCardInputRouting::new() (
            label = "Input Routing"
        );
        node output_routing: SoundCardOutputRouting = SoundCardOutputRouting::new() (
            label = "Output Routing"
        );
    }
    folder(parameters) {
        node input: SoundCardInputParameters = SoundCardInputParameters::new() (
            label = "Input"
        );
        node output: SoundCardOutputParameters = SoundCardOutputParameters::new() (
            label = "Output"
        );
        folder(processing, label = "Processing") {
            levels_update_rate_hz: i32 = LEVELS_UPDATE_RATE_DEFAULT_HZ [LEVELS_UPDATE_RATE_MIN_HZ..LEVELS_UPDATE_RATE_MAX_HZ] (
                label = "Levels Update Rate",
                description = "Rate in hertz used to publish audio levels and output activity.",
                widget = "text"
            );
            pitch_detection: bool = false (
                label = "Pitch Detection"
            );
        }
        [base_children];
    }
    folder(values) {
        folder(levels, label = "Levels") {}
        folder(playback_status, label = "Playback Status") {
            active_voices: i32 = 0 [0..2147483647] (
                label = "Active Voices",
                read_only = true
            );
            loading_voices: i32 = 0 [0..2147483647] (
                label = "Loading Voices",
                read_only = true
            );
        }
        [base_children];
    }
)]
pub struct SoundCardModule {
    base: crate::app::ModuleBase,
    runtime: Option<runtime::SoundCardRuntime>,
    commands: command_service::CommandService,
    routes: routes::RouteWork,
    runtime_worker: Option<runtime::SoundCardRuntimeWorker>,
    runtime_request: Option<runtime::SoundCardRuntimeRequest>,
    runtime_wake: Option<runtime::RuntimeWakeSender>,
    runtime_retry: ReconnectBackoff,
    runtime_retry_at: Option<(golden_audio::SampleRate, Option<golden_audio::BackendId>, Instant)>,
    automatic_sample_rate: Option<golden_audio::SampleRate>,
    input_default_routes_pending: bool,
    output_default_routes_pending: bool,
    input_route_reset_pending: bool,
    output_route_reset_pending: bool,
    configuration_dirty: bool,
    choices: choice_sync::ChoiceSync,
    outgoing_activity_active: bool,
    runtime_error_node: Option<NodeId>,
    pending_events: event_delivery::PendingAudioEvents,
    observation_warning: observation_warning::ObservationWarning,
}

impl SoundCardModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::create_with_command_types(SOUND_CARD_COMMAND_TYPES),
            None,
            command_service::CommandService::default(),
            routes::RouteWork::default(),
            None,
            None,
            None,
            ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(8)),
            None,
            None,
            false,
            false,
            false,
            false,
            true,
            choice_sync::ChoiceSync::default(),
            false,
            None,
            event_delivery::PendingAudioEvents::default(),
            observation_warning::ObservationWarning::default(),
        )
    }

    fn inbox_drives_runtime(&self, events: &EventFrame) -> bool {
        events.iter().any(|event| match &event.kind {
            EventKind::ParamChanged { param, .. } | EventKind::ParamControlChanged { param, .. } => !self
                .runtime
                .as_ref()
                .is_some_and(|runtime| runtime.bindings().is_runtime_value(*param)),
            EventKind::ParamConstraintsChanged { .. } => false,
            EventKind::MetaChanged { patch, .. } => {
                patch.label.is_some() || patch.short_name.is_some() || patch.enabled.is_some()
            }
            EventKind::ChildAdded { .. }
            | EventKind::ChildRemoved { .. }
            | EventKind::ChildReplaced { .. }
            | EventKind::ChildMoved { .. }
            | EventKind::ChildReordered { .. }
            | EventKind::NodeCreated { .. }
            | EventKind::NodeDeleted { .. }
            | EventKind::GraphTransaction { .. } => true,
            EventKind::Custom(custom) => custom.topic == chataigne_sound_card_protocol::SOUND_CARD_UI_CONTROL_TOPIC,
        })
    }

    fn uses_duplex_device(&self) -> bool {
        self.audio_driver.get_ref().0 == ASIO_AUDIO_DRIVER
    }

    fn selected_device_values(&self) -> (&str, &str) {
        if self.uses_duplex_device() {
            let device = self.device.get_ref().0.as_str();
            (device, device)
        } else {
            (
                self.input_device.get_ref().0.as_str(),
                self.output_device.get_ref().0.as_str(),
            )
        }
    }

    fn synchronize_runtime_projection(&mut self, ctx: &mut ProcessCtx) {
        let rate_hz = runtime::levels_update_rate_hz(self.levels_update_rate_hz.get());
        let outgoing_activity = match self.runtime.as_mut() {
            Some(runtime) => {
                runtime.set_levels_update_rate_hz(rate_hz);
                runtime.take_outgoing_activity_change()
            }
            None => Some(false),
        };
        if let Some(active) = outgoing_activity {
            if active != self.outgoing_activity_active {
                self.outgoing_activity_active = active;
                self.emit_outgoing_activity(ctx, active);
            }
        }
    }

    fn emit_outgoing_activity(&self, ctx: &mut ProcessCtx, active: bool) {
        ctx.emit_custom_event(CustomEvent::latest(
            chataigne_sound_card_protocol::SOUND_CARD_OUTPUT_ACTIVITY_TOPIC,
            Some(self.id()),
            serde_json::Value::Bool(active),
        ));
    }
}

#[golden_core::item(
    "module",
    node = "sound_card_module",
    via = base,
    from_struct,
    menu_path = ["Audio"]
)]
impl Node for SoundCardModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, SOUND_CARD_COMMAND_TYPES);
        self.base
            .set_data_capabilities(ctx, crate::app::module::ModuleDataCapabilities::new(false, false));
        self.base.set_connected(ctx, false);
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let waker = match ctx.node_waker() {
            Ok(waker) => waker,
            Err(error) => {
                self.set_runtime_error(ctx, self.id(), &error.to_string());
                return;
            }
        };
        if !self.runtime_wake.as_ref().is_some_and(|wake| wake.matches(&waker)) {
            match runtime::RuntimeWakeSender::new(waker) {
                Ok(wake) => self.runtime_wake = Some(wake),
                Err(error) => {
                    self.set_runtime_error(ctx, self.id(), &error);
                    return;
                }
            }
        }
        let fresh = context == NodeCreationContext::Fresh;
        self.routes = routes::RouteWork::default();
        self.input_default_routes_pending = fresh;
        self.output_default_routes_pending = fresh;
        self.input_route_reset_pending = false;
        self.output_route_reset_pending = false;
        self.synchronize_derived_structure(ctx, &snapshot);
        self.configuration_dirty = true;
        self.choices.invalidate();
        self.request_runtime_start(ctx);
    }

    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service_route_intents(ctx);
        self.drive_runtime(ctx);
        self.service_commands(ctx);
        self.synchronize_runtime_projection(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_runtime();
        self.runtime_wake = None;
    }

    fn needs_update(&self) -> bool {
        false
    }

    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.routes.prune_needs_snapshot()
            || (self.node_data().effective_enabled
                && (self.routes.lookup_needs_snapshot()
                    || self.commands.needs_snapshot()
                    || (self.configuration_dirty && !self.routes.prune_waiting())
                    || self.input_route_reset_pending
                    || self.output_route_reset_pending
                    || self.runtime.as_ref().is_some_and(|runtime| {
                        self.choices.needs_snapshot(runtime.device_revision()) && runtime.device_inventory().is_some()
                    })
                    || self
                        .runtime_request
                        .as_ref()
                        .is_some_and(|request| request.driver().is_some())))
    }

    fn worker_wake_when_disabled(&self) -> bool {
        self.routes.pruning()
            || self.choices.has_pending()
            || !self.pending_events.is_empty()
            || self.observation_warning.has_pending()
            || self.runtime.is_some()
            || self.runtime_request.is_some()
            || self
                .runtime_worker
                .as_ref()
                .is_some_and(runtime::SoundCardRuntimeWorker::has_pending)
    }

    fn attached_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn init_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive()
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        self.sound_card_script_descriptor()
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(result) = self.call_sound_card_script_method(ctx, method, args) {
            result?;
            return Ok(true);
        }
        self.base.engine_call_script_method(ctx, method, args)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ParamChanged { .. }
            | EventKind::ChildAdded { .. }
            | EventKind::ChildRemoved { .. }
            | EventKind::ChildReplaced { .. }
            | EventKind::MetaChanged { .. } => u32::MAX,
            _ => 1,
        }
    }

    fn inbox_requires_tree_snapshot(&self, events: &golden_core::events::EventFrame) -> bool {
        events.iter().any(|event| match &event.kind {
            EventKind::ParamChanged { param, .. } => !self
                .runtime
                .as_ref()
                .is_some_and(|runtime| runtime.bindings().is_runtime_value(*param)),
            EventKind::ChildAdded { .. } | EventKind::ChildRemoved { .. } | EventKind::ChildReplaced { .. } => true,
            EventKind::MetaChanged { patch, .. } => {
                patch.label.is_some() || patch.short_name.is_some() || patch.enabled.is_some()
            }
            EventKind::Custom(custom) => custom.topic == chataigne_sound_card_protocol::SOUND_CARD_UI_CONTROL_TOPIC,
            _ => false,
        })
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if self
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.bindings().is_runtime_value(param))
        {
            return;
        }
        if param == self.audio_driver.id() || param == self.device.id() {
            self.clear_device_connection_warnings(ctx);
        } else if param == self.input_device.id() {
            self.clear_device_connection_warning(ctx, golden_audio::AudioDirection::Input);
        } else if param == self.output_device.id() {
            self.clear_device_connection_warning(ctx, golden_audio::AudioDirection::Output);
        }
        if [
            self.audio_driver.id(),
            self.device.id(),
            self.input_device.id(),
            self.output_device.id(),
            self.sample_rate.id(),
        ]
        .contains(&param)
        {
            self.automatic_sample_rate = None;
        }
        if param == self.audio_driver.id() || param == self.device.id() {
            self.input_default_routes_pending = true;
            self.output_default_routes_pending = true;
            self.input_route_reset_pending = true;
            self.output_route_reset_pending = true;
        } else if param == self.input_device.id() {
            self.input_default_routes_pending = true;
            self.input_route_reset_pending = true;
        } else if param == self.output_device.id() {
            self.output_default_routes_pending = true;
            self.output_route_reset_pending = true;
        }
        if param == self.levels_update_rate_hz.id() {
            self.synchronize_runtime_projection(ctx);
        } else {
            self.configuration_dirty = true;
        }
        if [
            self.audio_driver.id(),
            self.device.id(),
            self.input_device.id(),
            self.output_device.id(),
            self.sample_rate.id(),
            self.buffer_size.id(),
        ]
        .contains(&param)
        {
            self.choices.invalidate();
            if let Some(wake) = &self.runtime_wake {
                wake.wake();
            }
        }
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.base
                .emit_script_param_callback(ctx, snapshot.as_ref(), param, &old_value);
        }
    }

    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, patch: NodeMetaPatch) {
        if patch.label.is_none() && patch.short_name.is_none() && patch.enabled.is_none() {
            return;
        }
        self.configuration_dirty = true;
    }

    fn on_structure_changed(&mut self, _ctx: &mut ProcessCtx) {
        self.configuration_dirty = true;
    }

    fn on_child_added(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.configuration_dirty = true;
    }

    fn on_child_removed(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.configuration_dirty = true;
    }

    fn on_child_replaced(&mut self, _ctx: &mut ProcessCtx, _parent: NodeId, _old: NodeId, _new: NodeId) {
        self.configuration_dirty = true;
    }

    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, enabled: bool) {
        self.configuration_dirty = true;
        self.choices.invalidate();
        if !enabled {
            self.routes.invalidate();
            self.commands = command_service::CommandService::default();
            self.choices.pause();
        }
        if let Some(worker) = &self.runtime_worker {
            worker.set_enabled(enabled);
        }
        if !enabled {
            self.runtime_request = None;
        }
        if let Some(runtime) = self.runtime.as_mut() {
            let _ = runtime.set_enabled(enabled);
        } else {
            self.configuration_dirty = true;
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.handle_sound_card_ui_control_event(ctx, &event);
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        let drive_runtime = self.inbox_drives_runtime(&ctx.events);
        self.dispatch_inbox(ctx);
        self.service_route_intents(ctx);
        if drive_runtime {
            self.drive_runtime(ctx);
            self.synchronize_runtime_projection(ctx);
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

pub(super) fn find_path(snapshot: &ProcessTreeSnapshot, start: NodeId, path: &str) -> Option<NodeId> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .try_fold(start, |parent, segment| find_child_by_key(snapshot, parent, segment))
}

pub(super) fn find_child_by_key(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str) -> Option<NodeId> {
    snapshot
        .find_child_by_decl_id(parent, key)
        .or_else(|| snapshot.find_child(parent, key))
}
