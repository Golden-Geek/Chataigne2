#[cfg(test)]
mod tests;
mod integration;
mod runtime;

use std::collections::HashSet;

use golden_core::{
    edit::{Edit, NodeTree},
    engine::NodeExecutionRule,
    events::{Event, EventKind},
    node,
    node::{
        DeclId, Folder, Node, NodeCreationContext, NodeHandle, NodeId, NodeMetaPatch,
        NodeReference, NodeUserPermissions, NodeUuid,
    },
    parameter::{
        Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use uuid::Uuid;

pub(crate) use crate::app::module_modules_audio_sound_card_commands::SOUND_CARD_COMMAND_TYPES;
use crate::app::module_modules_audio_sound_card_schema::{
    SoundCardAnalysisList, SoundCardChannelMeter, SoundCardInputPatchRoute,
    SoundCardInputProfile, SoundCardInputProfileList, SoundCardMonitorRouteList,
    SoundCardMonitorRoute, SoundCardOutputPatchRoute, SoundCardOutputProfile,
    SoundCardOutputProfileList, SoundCardPitchAnalyzer, SoundCardPlaybackRoute,
    SoundCardPlaybackRouteList, SoundCardSpectrumAnalyzer, SoundCardSpectrumBand,
    SoundCardVirtualInput, SoundCardVirtualInputList, SoundCardVirtualOutput,
    SoundCardVirtualOutputList,
};

const SYSTEM_DEFAULT_INPUT: &str = "platform_default:system_default:input";
const SYSTEM_DEFAULT_OUTPUT: &str = "platform_default:system_default:output";
const DEFAULT_SPECTRUM_BANDS: usize = 64;

const VIRTUAL_INPUTS_PATH: &str = "parameters/virtual_inputs";
const VIRTUAL_OUTPUTS_PATH: &str = "parameters/virtual_outputs";
const INPUT_PROFILES_PATH: &str = "parameters/device_profiles/input_profiles";
const OUTPUT_PROFILES_PATH: &str = "parameters/device_profiles/output_profiles";
const ANALYSIS_PATH: &str = "parameters/analysis";
const INPUT_LEVELS_PATH: &str = "values/input_levels";
const OUTPUT_LEVELS_PATH: &str = "values/output_levels";
const PITCH_RESULTS_PATH: &str = "values/pitch_results";
const SPECTRUM_RESULTS_PATH: &str = "values/spectrum_bands";

#[node("sound_card_module", label = "Sound Card")]
#[children(
    folder(connection) {
        input_enabled: bool = false (
            label = "Input Enabled",
            description = "Open the selected input stream. Disabled by default to avoid surprise capture."
        );
        input_device: Enum = SYSTEM_DEFAULT_INPUT (
            label = "Input Device",
            enum_options = device_options(SYSTEM_DEFAULT_INPUT, "System Default Input")
        );
        output_enabled: bool = true (
            label = "Output Enabled",
            description = "Open the selected output stream."
        );
        output_device: Enum = SYSTEM_DEFAULT_OUTPUT (
            label = "Output Device",
            enum_options = device_options(SYSTEM_DEFAULT_OUTPUT, "System Default Output")
        );
        recovery_policy: Enum = "wait_for_selected" (
            label = "Recovery Policy",
            enum_options = recovery_policy_options()
        );
        engine_sample_rate: i32 = 48000 [8000..384000] (
            label = "Engine Sample Rate"
        );
        buffer_policy: Enum = "automatic" (
            label = "Buffer Policy",
            enum_options = buffer_policy_options()
        );
        fixed_buffer_frames: i32 = 128 [16..8192] (
            label = "Fixed Buffer Frames"
        );
        refresh_devices: ParamValue = ParamValue::Trigger() (
            label = "Refresh Devices",
            description = "Request a nonblocking audio backend discovery refresh."
        );
        input_readiness: String = "disabled".to_string() (
            label = "Input Readiness",
            read_only = true
        );
        output_readiness: String = "unavailable".to_string() (
            label = "Output Readiness",
            read_only = true
        );
        negotiated_input_format: String = String::new() (
            label = "Negotiated Input Format",
            read_only = true
        );
        negotiated_output_format: String = String::new() (
            label = "Negotiated Output Format",
            read_only = true
        );
        [base_children];
    }
    folder(parameters) {
        master_volume_db: f64 = 0.0 [-120.0..24.0] (
            label = "Master Volume",
            description = "Master output gain in decibels."
        );
        node virtual_inputs: SoundCardVirtualInputList = SoundCardVirtualInputList::new() (
            label = "Virtual Inputs"
        );
        node virtual_outputs: SoundCardVirtualOutputList = SoundCardVirtualOutputList::new() (
            label = "Virtual Outputs"
        );
        folder(device_profiles, label = "Device Profiles") {
            node input_profiles: SoundCardInputProfileList = SoundCardInputProfileList::new() (
                label = "Input Profiles"
            );
            node output_profiles: SoundCardOutputProfileList = SoundCardOutputProfileList::new() (
                label = "Output Profiles"
            );
        }
        node monitoring_routes: SoundCardMonitorRouteList = SoundCardMonitorRouteList::new() (
            label = "Monitoring Routes"
        );
        node playback_routes: SoundCardPlaybackRouteList = SoundCardPlaybackRouteList::new() (
            label = "Playback Routes"
        );
        node analysis: SoundCardAnalysisList = SoundCardAnalysisList::new() (
            label = "Analysis"
        );
        [base_children];
    }
    folder(values) {
        folder(input_levels, label = "Input Levels") {}
        folder(output_levels, label = "Output Levels") {}
        folder(global_levels, label = "Global Levels") {
            input_global_max_rms: f64 = 0.0 (
                label = "Input Global Max RMS",
                read_only = true
            );
            output_global_max_rms: f64 = 0.0 (
                label = "Output Global Max RMS",
                read_only = true
            );
            global_max_rms: f64 = 0.0 (
                label = "Global Max RMS",
                read_only = true
            );
        }
        folder(pitch_results, label = "Pitch Results") {}
        folder(spectrum_bands, label = "Spectrum Bands") {}
        folder(playback_status, label = "Playback Status") {
            active_voices: i32 = 0 (
                label = "Active Voices",
                read_only = true
            );
            loading_voices: i32 = 0 (
                label = "Loading Voices",
                read_only = true
            );
        }
        folder(diagnostics, label = "Diagnostics") {
            xruns: i32 = 0 (
                label = "XRuns",
                read_only = true
            );
            dropped_analysis_frames: i32 = 0 (
                label = "Dropped Analysis Frames",
                read_only = true
            );
            last_error: String = String::new() (
                label = "Last Error",
                read_only = true
            );
        }
        [base_children];
    }
)]
pub struct SoundCardModule {
    base: crate::app::ModuleBase,
    runtime: Option<runtime::SoundCardRuntime>,
    configuration_dirty: bool,
    active_runtime_warnings: HashSet<(NodeId, String)>,
    runtime_error_node: Option<NodeId>,
}

impl SoundCardModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::create_with_command_types(SOUND_CARD_COMMAND_TYPES),
            None,
            true,
            HashSet::new(),
            None,
        )
    }

    fn initialize_fresh_defaults(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
    ) {
        let Some(input_list) = find_path(snapshot, self.id(), VIRTUAL_INPUTS_PATH) else {
            return;
        };
        let Some(output_list) = find_path(snapshot, self.id(), VIRTUAL_OUTPUTS_PATH) else {
            return;
        };

        let mut inputs = Vec::with_capacity(2);
        for index in 0..2 {
            let mut channel = SoundCardVirtualInput::new();
            set_authored_identity(
                &mut channel,
                format!("Input {}", index + 1),
                format!("input_{}", index + 1),
            );
            let reference = reference_to_detached(&channel);
            inputs.push(reference.clone());
            ctx.add_child_tree(input_list, NodeTree::new(channel), None);
        }

        let mut outputs = Vec::with_capacity(2);
        for index in 0..2 {
            let mut channel = SoundCardVirtualOutput::new();
            set_authored_identity(
                &mut channel,
                format!("Output {}", index + 1),
                format!("output_{}", index + 1),
            );
            let reference = reference_to_detached(&channel);
            outputs.push(reference.clone());
            ctx.add_child_tree(output_list, NodeTree::new(channel), None);
        }

        self.add_default_profiles(ctx, snapshot, inputs.as_slice(), outputs.as_slice());
        self.add_default_analyzers(ctx, snapshot, &inputs[0]);
        self.add_default_meter_projections(ctx, snapshot, inputs.as_slice(), outputs.as_slice());
    }

    fn add_default_profiles(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        inputs: &[NodeReference],
        outputs: &[NodeReference],
    ) {
        if let Some(parent) = find_path(snapshot, self.id(), INPUT_PROFILES_PATH) {
            let mut profile = SoundCardInputProfile::new();
            set_authored_identity(
                &mut profile,
                "System Default Input",
                "system_default_input",
            );
            let mut tree = NodeTree::new(profile);
            for (index, channel) in inputs.iter().enumerate() {
                let mut route = SoundCardInputPatchRoute::with_target(
                    format!("channel_{}", index + 1),
                    channel.clone(),
                );
                set_authored_identity(
                    &mut route,
                    format!("Input {}", index + 1),
                    format!("input_patch_{}", index + 1),
                );
                tree.push_child(NodeTree::new(route));
            }
            ctx.add_child_tree(parent, tree, None);
        }
        if let Some(parent) = find_path(snapshot, self.id(), OUTPUT_PROFILES_PATH) {
            let mut profile = SoundCardOutputProfile::new();
            set_authored_identity(
                &mut profile,
                "System Default Output",
                "system_default_output",
            );
            let mut tree = NodeTree::new(profile);
            for (index, channel) in outputs.iter().enumerate() {
                let mut route = SoundCardOutputPatchRoute::with_target(
                    channel.clone(),
                    format!("channel_{}", index + 1),
                );
                set_authored_identity(
                    &mut route,
                    format!("Output {}", index + 1),
                    format!("output_patch_{}", index + 1),
                );
                tree.push_child(NodeTree::new(route));
            }
            ctx.add_child_tree(parent, tree, None);
        }
    }

    fn add_default_analyzers(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        source: &NodeReference,
    ) {
        let Some(parent) = find_path(snapshot, self.id(), ANALYSIS_PATH) else {
            return;
        };
        let mut pitch = SoundCardPitchAnalyzer::for_source(source.clone());
        set_authored_identity(&mut pitch, "Pitch Analyzer", "pitch_analyzer_1");
        let pitch_uuid = pitch.node_data().meta.uuid;
        ctx.add_child_tree(parent, NodeTree::new(pitch), None);

        let mut spectrum = SoundCardSpectrumAnalyzer::for_source(source.clone());
        set_authored_identity(
            &mut spectrum,
            "Spectrum Analyzer",
            "spectrum_analyzer_1",
        );
        let spectrum_uuid = spectrum.node_data().meta.uuid;
        ctx.add_child_tree(parent, NodeTree::new(spectrum), None);

        if let Some(results) = find_path(snapshot, self.id(), PITCH_RESULTS_PATH) {
            ctx.add_child_tree(results, pitch_result_tree(pitch_uuid, "Pitch Analyzer"), None);
        }
        if let Some(results) = find_path(snapshot, self.id(), SPECTRUM_RESULTS_PATH) {
            ctx.add_child_tree(
                results,
                spectrum_result_tree(
                    spectrum_uuid,
                    "Spectrum Analyzer",
                    DEFAULT_SPECTRUM_BANDS,
                ),
                None,
            );
        }
    }

    fn add_default_meter_projections(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        inputs: &[NodeReference],
        outputs: &[NodeReference],
    ) {
        if let Some(parent) = find_path(snapshot, self.id(), INPUT_LEVELS_PATH) {
            for channel in inputs {
                ctx.add_child_tree(parent, meter_tree(channel), None);
            }
        }
        if let Some(parent) = find_path(snapshot, self.id(), OUTPUT_LEVELS_PATH) {
            for channel in outputs {
                ctx.add_child_tree(parent, meter_tree(channel), None);
            }
        }
    }

    fn synchronize_derived_structure(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
    ) {
        let inputs = typed_child_references(
            snapshot,
            find_path(snapshot, self.id(), VIRTUAL_INPUTS_PATH),
            SoundCardVirtualInput::NODE_TYPE,
        );
        let outputs = typed_child_references(
            snapshot,
            find_path(snapshot, self.id(), VIRTUAL_OUTPUTS_PATH),
            SoundCardVirtualOutput::NODE_TYPE,
        );
        sync_meters(
            ctx,
            snapshot,
            find_path(snapshot, self.id(), INPUT_LEVELS_PATH),
            inputs.as_slice(),
        );
        sync_meters(
            ctx,
            snapshot,
            find_path(snapshot, self.id(), OUTPUT_LEVELS_PATH),
            outputs.as_slice(),
        );

        let analyzers = find_path(snapshot, self.id(), ANALYSIS_PATH)
            .map(|parent| snapshot.child_ids(parent))
            .unwrap_or_default();
        sync_analysis_results(ctx, snapshot, self.id(), analyzers.as_slice());
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
        if context == NodeCreationContext::Fresh {
            self.initialize_fresh_defaults(ctx, snapshot.as_ref());
        } else {
            self.synchronize_derived_structure(ctx, snapshot.as_ref());
        }
        self.sync_device_choices(ctx, snapshot.as_ref());
        self.configuration_dirty = true;
        self.ensure_runtime(ctx);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        if self.configuration_dirty {
            if let Some(snapshot) = ctx.tree_snapshot_arc() {
                self.refresh_configuration(ctx, snapshot.as_ref());
            }
        }
        self.poll_runtime(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_runtime();
    }

    fn needs_update(&self) -> bool {
        self.runtime.is_some() || self.configuration_dirty
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.configuration_dirty
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(runtime::SOUND_CARD_UPDATE_RATE_HZ)
            .with_compiled_kernel(runtime::SOUND_CARD_COMPILED_KERNEL)
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
        events.iter().any(|event| {
            match event.kind {
                EventKind::ParamChanged { param, .. } => !self
                    .runtime
                    .as_ref()
                    .is_some_and(|runtime| runtime.bindings().is_runtime_value(param)),
                EventKind::ChildAdded { .. }
                    | EventKind::ChildRemoved { .. }
                    | EventKind::ChildReplaced { .. }
                    | EventKind::MetaChanged { .. } => true,
                _ => false,
            }
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
        self.configuration_dirty = true;
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.synchronize_derived_structure(ctx, snapshot.as_ref());
            self.sync_device_choices(ctx, snapshot.as_ref());
            self.base
                .emit_script_param_callback(ctx, snapshot.as_ref(), param, &old_value);
        }
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, _node: NodeId, patch: NodeMetaPatch) {
        if patch.label.is_none() {
            return;
        }
        self.configuration_dirty = true;
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.synchronize_derived_structure(ctx, snapshot.as_ref());
        }
    }

    fn on_child_added(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.configuration_dirty = true;
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.synchronize_derived_structure(ctx, snapshot.as_ref());
        }
    }

    fn on_child_removed(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _child: NodeId) {
        self.configuration_dirty = true;
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.synchronize_derived_structure(ctx, snapshot.as_ref());
        }
    }

    fn on_child_replaced(&mut self, ctx: &mut ProcessCtx, _parent: NodeId, _old: NodeId, _new: NodeId) {
        self.configuration_dirty = true;
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.synchronize_derived_structure(ctx, snapshot.as_ref());
        }
    }

    fn on_effective_enabled_changed(&mut self, _ctx: &mut ProcessCtx, enabled: bool) {
        if let Some(runtime) = self.runtime.as_mut() {
            let _ = runtime.set_enabled(enabled);
        } else {
            self.configuration_dirty = true;
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

pub(crate) fn backend_options() -> Vec<ParameterEnumOption> {
    vec![enum_option("platform_default", "Platform Default", 0)]
}

pub(crate) fn recovery_policy_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option("wait_for_selected", "Wait for Selected", 0),
        enum_option("follow_system_default", "Follow System Default", 1),
    ]
}

pub(crate) fn buffer_policy_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option("automatic", "Automatic", 0),
        enum_option("fixed", "Fixed", 1),
    ]
}

pub(crate) fn spectrum_window_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option("hann", "Hann", 0),
        enum_option("blackman_harris", "Blackman-Harris", 1),
    ]
}

pub(crate) fn spectrum_overlap_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option("none", "None", 0),
        enum_option("half", "50%", 1),
        enum_option("three_quarters", "75%", 2),
    ]
}

pub(crate) fn spectrum_spacing_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option("linear", "Linear", 0),
        enum_option("logarithmic", "Logarithmic", 1),
    ]
}

fn device_options(value: &str, label: &str) -> Vec<ParameterEnumOption> {
    vec![enum_option(value, label, 0)]
}

fn enum_option(value: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: value.to_string(),
        value: ParamValue::Enum(value.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}

fn sync_device_enum(
    ctx: &mut ProcessCtx,
    parameter_id: NodeId,
    default_value: &'static str,
    default_label: &'static str,
) {
    if parameter_id.0 == 0 {
        return;
    }
    ctx.call_node_mutation_without_snapshot(parameter_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("Sound Card device selector is not a parameter".to_string());
        };
        let current = parameter
            .value
            .as_enum()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_value.to_string());
        let mut options = device_options(default_value, default_label);
        if current != default_value {
            let mut missing = enum_option(
                current.as_str(),
                format!("Missing: {current}").as_str(),
                1,
            );
            missing.tags.push("missing".to_string());
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

fn sync_meters(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent: Option<NodeId>,
    channels: &[NodeReference],
) {
    let Some(parent) = parent else {
        return;
    };
    let desired = channels
        .iter()
        .map(|channel| meter_uuid(channel.uuid))
        .collect::<HashSet<_>>();
    let existing = snapshot.child_ids(parent);
    let existing_uuids = existing
        .iter()
        .filter_map(|id| snapshot.node(*id).map(|node| node.uuid))
        .collect::<HashSet<_>>();

    for channel in channels {
        if !existing_uuids.contains(&meter_uuid(channel.uuid)) {
            ctx.add_child_tree(parent, meter_tree(channel), None);
        }
    }
    for id in existing {
        let Some(node) = snapshot.node(id) else {
            continue;
        };
        if node.node_type == SoundCardChannelMeter::NODE_TYPE
            && !desired.contains(&node.uuid)
        {
            NodeHandle::new(id).remove(ctx);
        }
    }
}

fn sync_analysis_results(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    module: NodeId,
    analyzers: &[NodeId],
) {
    let pitch_parent = find_path(snapshot, module, PITCH_RESULTS_PATH);
    let spectrum_parent = find_path(snapshot, module, SPECTRUM_RESULTS_PATH);
    let mut pitch_uuids = HashSet::new();
    let mut spectrum_uuids = HashSet::new();

    for analyzer_id in analyzers {
        let Some(analyzer) = snapshot.node(*analyzer_id) else {
            continue;
        };
        if analyzer.node_type == SoundCardPitchAnalyzer::NODE_TYPE {
            let result_uuid = pitch_result_uuid(analyzer.uuid);
            pitch_uuids.insert(result_uuid);
            if let Some(parent) = pitch_parent {
                if !child_uuid_exists(snapshot, parent, result_uuid) {
                    ctx.add_child_tree(
                        parent,
                        pitch_result_tree(analyzer.uuid, analyzer.label.as_str()),
                        None,
                    );
                }
            }
        } else if analyzer.node_type == SoundCardSpectrumAnalyzer::NODE_TYPE {
            let result_uuid = spectrum_result_uuid(analyzer.uuid);
            spectrum_uuids.insert(result_uuid);
            let band_count = child_int(snapshot, *analyzer_id, "band_count")
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or(DEFAULT_SPECTRUM_BANDS)
                .clamp(1, 256);
            if let Some(parent) = spectrum_parent {
                if let Some(result) = child_by_uuid(snapshot, parent, result_uuid) {
                    sync_spectrum_bands(
                        ctx,
                        snapshot,
                        result,
                        analyzer.uuid,
                        band_count,
                    );
                } else {
                    ctx.add_child_tree(
                        parent,
                        spectrum_result_tree(
                            analyzer.uuid,
                            analyzer.label.as_str(),
                            band_count,
                        ),
                        None,
                    );
                }
            }
        }
    }

    remove_stale_result_folders(ctx, snapshot, pitch_parent, &pitch_uuids);
    remove_stale_result_folders(ctx, snapshot, spectrum_parent, &spectrum_uuids);
}

fn sync_spectrum_bands(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    analyzer_uuid: NodeUuid,
    band_count: usize,
) {
    let desired = (0..band_count)
        .map(|index| spectrum_band_uuid(analyzer_uuid, index))
        .collect::<HashSet<_>>();
    let existing = snapshot.child_ids(parent);
    let existing_uuids = existing
        .iter()
        .filter_map(|id| snapshot.node(*id).map(|node| node.uuid))
        .collect::<HashSet<_>>();

    for index in 0..band_count {
        let uuid = spectrum_band_uuid(analyzer_uuid, index);
        if !existing_uuids.contains(&uuid) {
            ctx.add_child_tree(parent, spectrum_band_tree(analyzer_uuid, index), None);
        }
    }
    for id in existing {
        let Some(node) = snapshot.node(id) else {
            continue;
        };
        if node.node_type == SoundCardSpectrumBand::NODE_TYPE
            && !desired.contains(&node.uuid)
        {
            NodeHandle::new(id).remove(ctx);
        }
    }
}

fn remove_stale_result_folders(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    parent: Option<NodeId>,
    desired: &HashSet<NodeUuid>,
) {
    let Some(parent) = parent else {
        return;
    };
    for child in snapshot.child_ids(parent) {
        let Some(node) = snapshot.node(child) else {
            continue;
        };
        if !desired.contains(&node.uuid) {
            NodeHandle::new(child).remove(ctx);
        }
    }
}

fn meter_tree(channel: &NodeReference) -> NodeTree {
    let mut meter = SoundCardChannelMeter::for_channel(channel.clone());
    let label = channel
        .cached_name
        .clone()
        .unwrap_or_else(|| "Channel".to_string());
    let data = meter.node_data_mut();
    data.meta.label = label;
    data.meta.uuid = meter_uuid(channel.uuid);
    data.meta.decl_id = DeclId(format!("meter_{}", channel.uuid.0.simple()));
    data.meta.short_name = data.meta.decl_id.0.clone();
    NodeTree::new(meter)
}

fn pitch_result_tree(analyzer_uuid: NodeUuid, label: &str) -> NodeTree {
    let mut folder = derived_folder(label, pitch_result_uuid(analyzer_uuid));
    folder.node_data_mut().meta.decl_id =
        DeclId(format!("pitch_{}", analyzer_uuid.0.simple()));
    folder.node_data_mut().meta.short_name =
        folder.node_data().meta.decl_id.0.clone();
    let mut tree = NodeTree::new(folder);
    for (decl_id, field_label, value) in [
        ("valid", "Valid", ParamValue::Bool(false)),
        ("frequency_hz", "Frequency", ParamValue::Float(0.0)),
        ("confidence", "Confidence", ParamValue::Float(0.0)),
        ("midi_note", "MIDI Note", ParamValue::Float(0.0)),
        ("note_name", "Note Name", ParamValue::Str(String::new())),
        ("cents", "Cents", ParamValue::Float(0.0)),
    ] {
        tree.push_child(NodeTree::new(derived_parameter(
            analyzer_uuid,
            decl_id,
            field_label,
            value,
        )));
    }
    tree
}

fn spectrum_result_tree(
    analyzer_uuid: NodeUuid,
    label: &str,
    band_count: usize,
) -> NodeTree {
    let mut folder = derived_folder(label, spectrum_result_uuid(analyzer_uuid));
    folder.node_data_mut().meta.decl_id =
        DeclId(format!("spectrum_{}", analyzer_uuid.0.simple()));
    folder.node_data_mut().meta.short_name =
        folder.node_data().meta.decl_id.0.clone();
    let mut tree = NodeTree::new(folder);
    for index in 0..band_count {
        tree.push_child(spectrum_band_tree(analyzer_uuid, index));
    }
    tree
}

fn spectrum_band_tree(analyzer_uuid: NodeUuid, index: usize) -> NodeTree {
    let mut band = SoundCardSpectrumBand::for_index(index);
    let data = band.node_data_mut();
    data.meta.label = format!("Band {}", index + 1);
    data.meta.uuid = spectrum_band_uuid(analyzer_uuid, index);
    data.meta.decl_id = DeclId(format!("band_{index}"));
    data.meta.short_name = data.meta.decl_id.0.clone();
    NodeTree::new(band)
}

fn derived_folder(label: &str, uuid: NodeUuid) -> Folder {
    let mut folder = Folder::new(label);
    folder.node_data_mut().meta.uuid = uuid;
    folder.node_data_mut().meta.user_permissions = NodeUserPermissions::none();
    folder
}

fn derived_parameter(
    owner_uuid: NodeUuid,
    decl_id: &str,
    label: &str,
    value: ParamValue,
) -> Parameter {
    let mut parameter =
        Parameter::new(label, value, ParameterChangeCheck::ValueChange);
    let data = parameter.node_data_mut();
    data.meta.uuid = derived_uuid(owner_uuid, decl_id.as_bytes());
    data.meta.decl_id = DeclId(decl_id.to_string());
    data.meta.short_name = decl_id.to_string();
    data.meta.user_permissions = NodeUserPermissions::none();
    parameter.read_only = true;
    parameter
}

fn reference_to_detached(node: &impl Node) -> NodeReference {
    let data = node.node_data();
    let mut reference = NodeReference::new(data.meta.uuid);
    reference.cached_name = Some(data.meta.label.clone());
    reference
}

fn set_authored_identity(
    node: &mut impl Node,
    label: impl Into<String>,
    decl_id: impl Into<String>,
) {
    let data = node.node_data_mut();
    data.meta.label = label.into();
    data.meta.decl_id = DeclId(decl_id.into());
    data.meta.short_name = data.meta.decl_id.0.clone();
}

fn typed_child_references(
    snapshot: &ProcessTreeSnapshot,
    parent: Option<NodeId>,
    node_type: &str,
) -> Vec<NodeReference> {
    let Some(parent) = parent else {
        return Vec::new();
    };
    snapshot
        .child_ids(parent)
        .into_iter()
        .filter_map(|id| {
            let node = snapshot.node(id)?;
            (node.node_type == node_type).then(|| {
                let mut reference =
                    NodeReference::with_cached_id(node.uuid, Some(id));
                reference.cached_name = Some(node.label.clone());
                reference
            })
        })
        .collect()
}

fn child_int(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    decl_id: &str,
) -> Option<i32> {
    find_child_by_key(snapshot, parent, decl_id)
        .and_then(|id| snapshot.node(id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_int)
}

fn find_path(
    snapshot: &ProcessTreeSnapshot,
    start: NodeId,
    path: &str,
) -> Option<NodeId> {
    path.split('/')
        .try_fold(start, |parent, segment| find_child_by_key(snapshot, parent, segment))
}

fn find_child_by_key(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    key: &str,
) -> Option<NodeId> {
    snapshot.child_ids(parent).into_iter().find(|child| {
        snapshot.node(*child).is_some_and(|node| {
            node.decl_id == key
                || node.decl_id.rsplit('/').next() == Some(key)
                || node.short_name == key
                || node.label == key
        })
    })
}

fn child_uuid_exists(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    uuid: NodeUuid,
) -> bool {
    child_by_uuid(snapshot, parent, uuid).is_some()
}

fn child_by_uuid(
    snapshot: &ProcessTreeSnapshot,
    parent: NodeId,
    uuid: NodeUuid,
) -> Option<NodeId> {
    snapshot
        .child_ids(parent)
        .into_iter()
        .find(|id| snapshot.node(*id).is_some_and(|node| node.uuid == uuid))
}

fn meter_uuid(channel_uuid: NodeUuid) -> NodeUuid {
    derived_uuid(channel_uuid, b"sound-card-meter")
}

fn pitch_result_uuid(analyzer_uuid: NodeUuid) -> NodeUuid {
    derived_uuid(analyzer_uuid, b"sound-card-pitch-result")
}

fn spectrum_result_uuid(analyzer_uuid: NodeUuid) -> NodeUuid {
    derived_uuid(analyzer_uuid, b"sound-card-spectrum-result")
}

fn spectrum_band_uuid(analyzer_uuid: NodeUuid, index: usize) -> NodeUuid {
    derived_uuid(analyzer_uuid, format!("sound-card-band-{index}").as_bytes())
}

fn derived_uuid(namespace: NodeUuid, name: &[u8]) -> NodeUuid {
    NodeUuid(Uuid::new_v5(&namespace.0, name))
}
