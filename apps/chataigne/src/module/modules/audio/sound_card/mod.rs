mod integration;
mod runtime;
mod script;
mod settings;
mod structure;
#[cfg(test)]
mod tests;

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event, EventFrame, EventKind},
    node,
    node::{Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{Enum, ParamValue, Parameter},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use golden_io::ReconnectBackoff;

pub(crate) use crate::app::module_modules_audio_sound_card_commands::SOUND_CARD_COMMAND_TYPES;
use crate::app::module_modules_audio_sound_card_schema::{
    SoundCardInputParameters, SoundCardInputRouting, SoundCardOutputParameters, SoundCardOutputRouting,
};
use settings::{NO_AUDIO_DRIVER, enum_option};
pub(crate) use settings::{
    backend_options, buffer_size_options, default_audio_driver, device_options, input_device_options,
    output_device_options, sample_rate_options,
};

pub(crate) const ASIO_AUDIO_DRIVER: &str = "asio";
pub(crate) const NO_AUDIO_DEVICE: &str = "none";
pub(crate) const SYSTEM_DEFAULT_DEVICE: &str = "system_default";
pub(crate) const AUTOMATIC_CONFIGURATION: &str = "automatic";
pub(crate) const DEFAULT_SPECTRUM_BANDS: usize = 64;
pub(crate) const MAX_PLAYBACK_SOURCE_CHANNELS: u16 = 256;

pub(crate) const INPUT_CHANNELS_PATH: &str = "parameters/input/channels";
pub(crate) const OUTPUT_CHANNELS_PATH: &str = "parameters/output/channels";
pub(crate) const INPUT_ROUTES_PATH: &str = "connection/input_routing/routes";
pub(crate) const OUTPUT_ROUTES_PATH: &str = "connection/output_routing/routes";
pub(crate) const PITCH_VALUES_PATH: &str = "values/pitch_detection";
pub(crate) const SPECTRAL_VALUES_PATH: &str = "values/spectral_analysis";

#[node("sound_card_module", label = "Sound Card")]
#[children(
    folder(connection) {
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
        [base_children];
    }
    folder(parameters) {
        node input: SoundCardInputParameters = SoundCardInputParameters::new() (
            label = "Input"
        );
        node output: SoundCardOutputParameters = SoundCardOutputParameters::new() (
            label = "Output"
        );
        folder(processing, label = "Processing") {
            pitch_detection: bool = false (
                label = "Pitch Detection"
            );
            spectral_analysis: bool = false (
                label = "Spectral Analysis"
            );
        }
        [base_children];
    }
    folder(values) {
        [base_children];
    }
)]
pub struct SoundCardModule {
    base: crate::app::ModuleBase,
    runtime: Option<runtime::SoundCardRuntime>,
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
    active_runtime_warnings: HashSet<(NodeId, String)>,
    runtime_error_node: Option<NodeId>,
}

impl SoundCardModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::create_with_command_types(SOUND_CARD_COMMAND_TYPES),
            None,
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
            HashSet::new(),
            None,
        )
    }

    fn inbox_drives_runtime(&self, events: &EventFrame) -> bool {
        events.iter().any(|event| match &event.kind {
            EventKind::ParamChanged { param, .. } | EventKind::ParamControlChanged { param, .. } => !self
                .runtime
                .as_ref()
                .is_some_and(|runtime| runtime.bindings().is_runtime_value(*param)),
            EventKind::ParamConstraintsChanged { .. } => false,
            EventKind::ChildAdded { .. }
            | EventKind::ChildRemoved { .. }
            | EventKind::ChildReplaced { .. }
            | EventKind::ChildMoved { .. }
            | EventKind::ChildReordered { .. }
            | EventKind::NodeCreated { .. }
            | EventKind::NodeDeleted { .. }
            | EventKind::MetaChanged { .. }
            | EventKind::GraphTransaction { .. } => true,
            EventKind::Custom(custom) => {
                custom.topic == runtime::SOUND_CARD_RUNTIME_WAKE_TOPIC
                    || custom.topic == crate::app::module_command::MODULE_COMMAND_REQUEST_TOPIC
                    || custom.topic == chataigne_sound_card_protocol::SOUND_CARD_UI_CONTROL_TOPIC
            }
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

    fn selected_device_parameter_ids(&self) -> Vec<NodeId> {
        if self.uses_duplex_device() {
            vec![self.device.id()]
        } else {
            vec![self.input_device.id(), self.output_device.id()]
        }
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
        self.runtime_wake = ctx
            .external_edit_sender()
            .map(|sender| runtime::RuntimeWakeSender::new(sender, self.id()));
        let fresh = context == NodeCreationContext::Fresh;
        self.input_default_routes_pending = fresh;
        self.output_default_routes_pending = fresh;
        self.input_route_reset_pending = false;
        self.output_route_reset_pending = false;
        self.synchronize_derived_structure(ctx, snapshot.as_ref());
        self.sync_device_choices(ctx, snapshot.as_ref());
        self.configuration_dirty = true;
        self.request_runtime_start(ctx);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.drive_runtime(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_runtime();
    }

    fn needs_update(&self) -> bool {
        false
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        false
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
            EventKind::ChildAdded { .. }
            | EventKind::ChildRemoved { .. }
            | EventKind::ChildReplaced { .. }
            | EventKind::MetaChanged { .. } => true,
            EventKind::Custom(custom) => {
                if custom.topic == runtime::SOUND_CARD_RUNTIME_WAKE_TOPIC {
                    self.configuration_dirty || self.runtime_request.is_some()
                } else {
                    custom.topic == crate::app::module_command::MODULE_COMMAND_REQUEST_TOPIC
                        || custom.topic == chataigne_sound_card_protocol::SOUND_CARD_UI_CONTROL_TOPIC
                }
            }
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
        self.configuration_dirty = true;
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            if param == self.audio_driver.id() {
                self.sync_device_choices(ctx, snapshot.as_ref());
            } else if [
                self.device.id(),
                self.input_device.id(),
                self.output_device.id(),
                self.sample_rate.id(),
                self.buffer_size.id(),
            ]
            .contains(&param)
            {
                if let Some(inspector) = self.runtime.as_ref().map(runtime::SoundCardRuntime::inspector_state) {
                    self.sync_live_device_choices(ctx, &inspector);
                }
            }
            self.base
                .emit_script_param_callback(ctx, snapshot.as_ref(), param, &old_value);
        }
    }

    fn on_meta_changed(&mut self, _ctx: &mut ProcessCtx, _node: NodeId, patch: NodeMetaPatch) {
        if patch.label.is_none() {
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
        if let Some(runtime) = self.runtime.as_mut() {
            let _ = runtime.set_enabled(enabled);
        } else {
            self.configuration_dirty = true;
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        if event.topic == runtime::SOUND_CARD_RUNTIME_WAKE_TOPIC {
            if let Some(wake) = &self.runtime_wake {
                wake.acknowledge();
            }
            return;
        }
        if self.handle_sound_card_ui_control_event(ctx, &event) {
            return;
        }
        self.handle_sound_card_command_event(ctx, &event);
    }

    fn on_inbox(&mut self, ctx: &mut ProcessCtx) {
        let drive_runtime = self.inbox_drives_runtime(&ctx.events);
        self.dispatch_inbox(ctx);
        if drive_runtime {
            self.drive_runtime(ctx);
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

fn sync_enum_options(
    ctx: &mut ProcessCtx,
    parameter_id: NodeId,
    mut options: Vec<golden_core::parameter::ParameterEnumOption>,
) {
    if parameter_id.0 == 0 {
        return;
    }
    ctx.call_node_mutation_without_snapshot(parameter_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("Sound Card device selector is not a parameter".to_owned());
        };
        let current = parameter
            .value
            .as_enum()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_default();
        if !options.iter().any(|option| option.variant_id == current) {
            let mut missing = enum_option(current.as_str(), format!("Missing: {current}").as_str(), 1);
            missing.tags.push("missing".to_owned());
            options.push(missing);
        }
        if parameter.constraints.enum_options == options {
            return Ok(());
        }
        let mut constraints = parameter.constraints.clone();
        constraints.enum_options = options;
        inner_ctx.edits.push(Edit::SetParamConstraints {
            node: parameter_id,
            constraints,
        });
        Ok(())
    });
}

pub(super) fn find_path(snapshot: &ProcessTreeSnapshot, start: NodeId, path: &str) -> Option<NodeId> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .try_fold(start, |parent, segment| {
            find_child_by_key(snapshot, parent, segment)
        })
}

pub(super) fn find_child_by_key(snapshot: &ProcessTreeSnapshot, parent: NodeId, key: &str) -> Option<NodeId> {
    snapshot
        .find_child_by_decl_id(parent, key)
        .or_else(|| snapshot.find_child(parent, key))
}
