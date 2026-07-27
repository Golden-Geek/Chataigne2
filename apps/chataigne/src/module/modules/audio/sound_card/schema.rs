use golden_core::{
    node,
    node::{Node, NodeReference, UserContainerRules, UserCreatableItem},
    parameter::{Enum, ParamValue, ReferenceTargetKind},
    process_ctx::ProcessCtx,
};

pub(crate) const SOUND_CARD_VIRTUAL_INPUT_FILTER_KEY: &str =
    "sound_card_same_module_virtual_input";
pub(crate) const SOUND_CARD_VIRTUAL_OUTPUT_FILTER_KEY: &str =
    "sound_card_same_module_virtual_output";

const VIRTUAL_INPUT_ITEM_KIND: &str = "sound_card_virtual_input";
const VIRTUAL_OUTPUT_ITEM_KIND: &str = "sound_card_virtual_output";
const INPUT_PROFILE_ITEM_KIND: &str = "sound_card_input_profile";
const OUTPUT_PROFILE_ITEM_KIND: &str = "sound_card_output_profile";
const INPUT_PATCH_ROUTE_ITEM_KIND: &str = "sound_card_input_patch_route";
const OUTPUT_PATCH_ROUTE_ITEM_KIND: &str = "sound_card_output_patch_route";
const MONITOR_ROUTE_ITEM_KIND: &str = "sound_card_monitor_route";
const PLAYBACK_ROUTE_ITEM_KIND: &str = "sound_card_playback_route";
const PITCH_ANALYZER_ITEM_KIND: &str = "sound_card_pitch_analyzer";
const SPECTRUM_ANALYZER_ITEM_KIND: &str = "sound_card_spectrum_analyzer";

fn input_reference() -> NodeReference {
    NodeReference::default()
}

fn output_reference() -> NodeReference {
    NodeReference::default()
}

#[node("sound_card_virtual_input_list", label = "Virtual Inputs")]
pub struct SoundCardVirtualInputList {}

#[node("sound_card_virtual_input_list", from_struct)]
impl Node for SoundCardVirtualInputList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[VIRTUAL_INPUT_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SoundCardVirtualInput::NODE_TYPE,
                VIRTUAL_INPUT_ITEM_KIND,
                "Virtual Input",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SoundCardVirtualInput::NODE_TYPE
            || node_type == VIRTUAL_INPUT_ITEM_KIND)
            .then(|| Box::new(SoundCardVirtualInput::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_virtual_output_list", label = "Virtual Outputs")]
pub struct SoundCardVirtualOutputList {}

#[node("sound_card_virtual_output_list", from_struct)]
impl Node for SoundCardVirtualOutputList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[VIRTUAL_OUTPUT_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SoundCardVirtualOutput::NODE_TYPE,
                VIRTUAL_OUTPUT_ITEM_KIND,
                "Virtual Output",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SoundCardVirtualOutput::NODE_TYPE
            || node_type == VIRTUAL_OUTPUT_ITEM_KIND)
            .then(|| Box::new(SoundCardVirtualOutput::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_input_profile_list", label = "Input Profiles")]
pub struct SoundCardInputProfileList {}

#[node("sound_card_input_profile_list", from_struct)]
impl Node for SoundCardInputProfileList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[INPUT_PROFILE_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SoundCardInputProfile::NODE_TYPE,
                INPUT_PROFILE_ITEM_KIND,
                "Input Profile",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SoundCardInputProfile::NODE_TYPE
            || node_type == INPUT_PROFILE_ITEM_KIND)
            .then(|| Box::new(SoundCardInputProfile::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_output_profile_list", label = "Output Profiles")]
pub struct SoundCardOutputProfileList {}

#[node("sound_card_output_profile_list", from_struct)]
impl Node for SoundCardOutputProfileList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[OUTPUT_PROFILE_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SoundCardOutputProfile::NODE_TYPE,
                OUTPUT_PROFILE_ITEM_KIND,
                "Output Profile",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SoundCardOutputProfile::NODE_TYPE
            || node_type == OUTPUT_PROFILE_ITEM_KIND)
            .then(|| Box::new(SoundCardOutputProfile::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_monitor_route_list", label = "Monitoring Routes")]
pub struct SoundCardMonitorRouteList {}

#[node("sound_card_monitor_route_list", from_struct)]
impl Node for SoundCardMonitorRouteList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[MONITOR_ROUTE_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SoundCardMonitorRoute::NODE_TYPE,
                MONITOR_ROUTE_ITEM_KIND,
                "Monitor Route",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SoundCardMonitorRoute::NODE_TYPE
            || node_type == MONITOR_ROUTE_ITEM_KIND)
            .then(|| Box::new(SoundCardMonitorRoute::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_playback_route_list", label = "Playback Routes")]
pub struct SoundCardPlaybackRouteList {}

#[node("sound_card_playback_route_list", from_struct)]
impl Node for SoundCardPlaybackRouteList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[PLAYBACK_ROUTE_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SoundCardPlaybackRoute::NODE_TYPE,
                PLAYBACK_ROUTE_ITEM_KIND,
                "Playback Route",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SoundCardPlaybackRoute::NODE_TYPE
            || node_type == PLAYBACK_ROUTE_ITEM_KIND)
            .then(|| Box::new(SoundCardPlaybackRoute::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_analysis_list", label = "Analysis")]
pub struct SoundCardAnalysisList {}

#[node("sound_card_analysis_list", from_struct)]
impl Node for SoundCardAnalysisList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_manager_authoring(self.node_data_mut());
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[
            PITCH_ANALYZER_ITEM_KIND,
            SPECTRUM_ANALYZER_ITEM_KIND,
        ]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SoundCardPitchAnalyzer::NODE_TYPE,
                PITCH_ANALYZER_ITEM_KIND,
                "Pitch Analyzer",
            )
            .with_select_when_created(false),
            UserCreatableItem::new(
                SoundCardSpectrumAnalyzer::NODE_TYPE,
                SPECTRUM_ANALYZER_ITEM_KIND,
                "Spectrum Analyzer",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        if node_type == SoundCardPitchAnalyzer::NODE_TYPE {
            Some(Box::new(SoundCardPitchAnalyzer::new()))
        } else if node_type == SoundCardSpectrumAnalyzer::NODE_TYPE {
            Some(Box::new(SoundCardSpectrumAnalyzer::new()))
        } else {
            None
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_virtual_input", label = "Virtual Input")]
pub struct SoundCardVirtualInput {}

#[node("sound_card_virtual_input", from_struct)]
impl Node for SoundCardVirtualInput {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_virtual_output", label = "Virtual Output")]
#[children(
    volume_db: f64 = 0.0 [-120.0..24.0] (
        label = "Volume",
        description = "Post-route virtual output fader in decibels."
    );
)]
pub struct SoundCardVirtualOutput {}

#[node("sound_card_virtual_output", from_struct)]
impl Node for SoundCardVirtualOutput {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_input_profile", label = "Input Profile")]
#[children(
    backend: Enum = "platform_default" (
        label = "Backend",
        enum_options = crate::app::module_modules_audio_sound_card::backend_options()
    );
    device_id: String = "system_default".to_string() (
        label = "Device ID",
        description = "Stable backend-owned device identity."
    );
    fallback_fingerprint: String = String::new() (
        label = "Fallback Fingerprint"
    );
    last_known_label: String = "System Default Input".to_string() (
        label = "Last Known Label"
    );
    profile_key: String = "platform_default:system_default:input".to_string() (
        label = "Profile Key",
        read_only = true
    );
)]
pub struct SoundCardInputProfile {}

#[node("sound_card_input_profile", from_struct)]
impl Node for SoundCardInputProfile {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[INPUT_PATCH_ROUTE_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SoundCardInputPatchRoute::NODE_TYPE,
                INPUT_PATCH_ROUTE_ITEM_KIND,
                "Input Patch Route",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SoundCardInputPatchRoute::NODE_TYPE
            || node_type == INPUT_PATCH_ROUTE_ITEM_KIND)
            .then(|| Box::new(SoundCardInputPatchRoute::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_output_profile", label = "Output Profile")]
#[children(
    backend: Enum = "platform_default" (
        label = "Backend",
        enum_options = crate::app::module_modules_audio_sound_card::backend_options()
    );
    device_id: String = "system_default".to_string() (
        label = "Device ID",
        description = "Stable backend-owned device identity."
    );
    fallback_fingerprint: String = String::new() (
        label = "Fallback Fingerprint"
    );
    last_known_label: String = "System Default Output".to_string() (
        label = "Last Known Label"
    );
    profile_key: String = "platform_default:system_default:output".to_string() (
        label = "Profile Key",
        read_only = true
    );
)]
pub struct SoundCardOutputProfile {}

#[node("sound_card_output_profile", from_struct)]
impl Node for SoundCardOutputProfile {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn user_container_rules(&self) -> Option<UserContainerRules> {
        Some(UserContainerRules::new(&[OUTPUT_PATCH_ROUTE_ITEM_KIND]))
    }

    fn user_creatable_items(&self) -> Vec<UserCreatableItem> {
        vec![
            UserCreatableItem::new(
                SoundCardOutputPatchRoute::NODE_TYPE,
                OUTPUT_PATCH_ROUTE_ITEM_KIND,
                "Output Patch Route",
            )
            .with_select_when_created(false),
        ]
    }

    fn create_user_item(&self, node_type: &str) -> Option<Box<dyn Node>> {
        (node_type == SoundCardOutputPatchRoute::NODE_TYPE
            || node_type == OUTPUT_PATCH_ROUTE_ITEM_KIND)
            .then(|| Box::new(SoundCardOutputPatchRoute::new()) as Box<dyn Node>)
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_input_patch_route", label = "Input Patch Route")]
#[children(
    physical_channel: String = "channel_1".to_string() (
        label = "Physical Input"
    );
    virtual_input: NodeReference = input_reference() (
        label = "Virtual Input",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![SoundCardVirtualInput::NODE_TYPE.to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_VIRTUAL_INPUT_FILTER_KEY.to_string())
    );
    gain_db: f64 = 0.0 [-120.0..24.0] (
        label = "Gain"
    );
)]
pub struct SoundCardInputPatchRoute {}

impl SoundCardInputPatchRoute {
    pub(crate) fn with_target(
        physical_channel: impl Into<String>,
        virtual_input: NodeReference,
    ) -> Self {
        let mut route = Self::new();
        route
            .physical_channel
            .apply_runtime_value(&ParamValue::Str(physical_channel.into()));
        route
            .virtual_input
            .apply_runtime_value(&ParamValue::Reference(virtual_input));
        route
    }
}

#[node("sound_card_input_patch_route", from_struct)]
impl Node for SoundCardInputPatchRoute {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_output_patch_route", label = "Output Patch Route")]
#[children(
    virtual_output: NodeReference = output_reference() (
        label = "Virtual Output",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![SoundCardVirtualOutput::NODE_TYPE.to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_VIRTUAL_OUTPUT_FILTER_KEY.to_string())
    );
    physical_channel: String = "channel_1".to_string() (
        label = "Physical Output"
    );
    gain_db: f64 = 0.0 [-120.0..24.0] (
        label = "Gain"
    );
)]
pub struct SoundCardOutputPatchRoute {}

impl SoundCardOutputPatchRoute {
    pub(crate) fn with_target(
        virtual_output: NodeReference,
        physical_channel: impl Into<String>,
    ) -> Self {
        let mut route = Self::new();
        route
            .virtual_output
            .apply_runtime_value(&ParamValue::Reference(virtual_output));
        route
            .physical_channel
            .apply_runtime_value(&ParamValue::Str(physical_channel.into()));
        route
    }
}

#[node("sound_card_output_patch_route", from_struct)]
impl Node for SoundCardOutputPatchRoute {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_monitor_route", label = "Monitor Route")]
#[children(
    virtual_input: NodeReference = input_reference() (
        label = "Virtual Input",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![SoundCardVirtualInput::NODE_TYPE.to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_VIRTUAL_INPUT_FILTER_KEY.to_string())
    );
    virtual_output: NodeReference = output_reference() (
        label = "Virtual Output",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![SoundCardVirtualOutput::NODE_TYPE.to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_VIRTUAL_OUTPUT_FILTER_KEY.to_string())
    );
    gain_db: f64 = 0.0 [-120.0..24.0] (
        label = "Gain"
    );
)]
pub struct SoundCardMonitorRoute {}

impl SoundCardMonitorRoute {
    #[cfg(test)]
    pub(crate) fn with_targets(
        virtual_input: NodeReference,
        virtual_output: NodeReference,
    ) -> Self {
        let mut route = Self::new();
        route
            .virtual_input
            .apply_runtime_value(&ParamValue::Reference(virtual_input));
        route
            .virtual_output
            .apply_runtime_value(&ParamValue::Reference(virtual_output));
        route
    }
}

#[node("sound_card_monitor_route", from_struct)]
impl Node for SoundCardMonitorRoute {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_playback_route", label = "Playback Route")]
#[children(
    source_channel: i32 = 1 [1..256] (
        label = "Source Channel"
    );
    virtual_output: NodeReference = output_reference() (
        label = "Virtual Output",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![SoundCardVirtualOutput::NODE_TYPE.to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_VIRTUAL_OUTPUT_FILTER_KEY.to_string())
    );
    gain_db: f64 = 0.0 [-120.0..24.0] (
        label = "Gain"
    );
)]
pub struct SoundCardPlaybackRoute {}

impl SoundCardPlaybackRoute {
    pub(crate) fn with_target(
        source_channel: i32,
        virtual_output: NodeReference,
    ) -> Self {
        let mut route = Self::new();
        route
            .source_channel
            .apply_runtime_value(&ParamValue::Int(source_channel));
        route
            .virtual_output
            .apply_runtime_value(&ParamValue::Reference(virtual_output));
        route
    }
}

#[node("sound_card_playback_route", from_struct)]
impl Node for SoundCardPlaybackRoute {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_channel_meter", label = "Channel Meter")]
#[children(
    channel: NodeReference = NodeReference::default() (
        label = "Channel",
        read_only = true,
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![
            SoundCardVirtualInput::NODE_TYPE.to_string(),
            SoundCardVirtualOutput::NODE_TYPE.to_string()
        ]
    );
    linear_rms: f64 = 0.0 (
        label = "Linear RMS",
        read_only = true
    );
    rms_dbfs: f64 = -120.0 (
        label = "RMS dBFS",
        read_only = true
    );
    peak_dbfs: f64 = -120.0 (
        label = "Peak dBFS",
        read_only = true
    );
    clipped: bool = false (
        label = "Clipped",
        read_only = true
    );
)]
pub struct SoundCardChannelMeter {}

impl SoundCardChannelMeter {
    pub(crate) fn for_channel(channel: NodeReference) -> Self {
        let mut meter = Self::new();
        meter
            .channel
            .apply_runtime_value(&ParamValue::Reference(channel));
        meter
    }
}

#[node("sound_card_channel_meter", from_struct)]
impl Node for SoundCardChannelMeter {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions =
            golden_core::node::NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_pitch_analyzer", label = "Pitch Analyzer")]
#[children(
    source: NodeReference = input_reference() (
        label = "Virtual Input",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![SoundCardVirtualInput::NODE_TYPE.to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_VIRTUAL_INPUT_FILTER_KEY.to_string())
    );
    frame_size: i32 = 2048 [256..16384] (
        label = "Frame Size"
    );
    minimum_frequency_hz: f64 = 50.0 [1.0..24000.0] (
        label = "Minimum Frequency"
    );
    maximum_frequency_hz: f64 = 2000.0 [1.0..24000.0] (
        label = "Maximum Frequency"
    );
    power_threshold: f64 = 0.0001 [0.0..1.0] (
        label = "Power Threshold"
    );
    yin_threshold: f64 = 0.15 [0.0..1.0] (
        label = "YIN Threshold"
    );
    confidence_threshold: f64 = 0.75 [0.0..1.0] (
        label = "Confidence Threshold"
    );
)]
pub struct SoundCardPitchAnalyzer {}

impl SoundCardPitchAnalyzer {
    pub(crate) fn for_source(source: NodeReference) -> Self {
        let mut analyzer = Self::new();
        analyzer
            .source
            .apply_runtime_value(&ParamValue::Reference(source));
        analyzer
    }
}

#[node("sound_card_pitch_analyzer", from_struct)]
impl Node for SoundCardPitchAnalyzer {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_spectrum_analyzer", label = "Spectrum Analyzer")]
#[children(
    source: NodeReference = input_reference() (
        label = "Virtual Input",
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec![SoundCardVirtualInput::NODE_TYPE.to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_VIRTUAL_INPUT_FILTER_KEY.to_string())
    );
    fft_size: i32 = 2048 [256..16384] (
        label = "FFT Size"
    );
    window: Enum = "hann" (
        label = "Window",
        enum_options = crate::app::module_modules_audio_sound_card::spectrum_window_options()
    );
    overlap: Enum = "half" (
        label = "Overlap",
        enum_options = crate::app::module_modules_audio_sound_card::spectrum_overlap_options()
    );
    spacing: Enum = "logarithmic" (
        label = "Band Spacing",
        enum_options = crate::app::module_modules_audio_sound_card::spectrum_spacing_options()
    );
    minimum_frequency_hz: f64 = 20.0 [0.0..24000.0] (
        label = "Minimum Frequency"
    );
    maximum_frequency_hz: f64 = 20000.0 [1.0..24000.0] (
        label = "Maximum Frequency"
    );
    band_count: i32 = 64 [1..256] (
        label = "Bands"
    );
    attack: f64 = 0.35 [0.0..1.0] (
        label = "Attack"
    );
    release: f64 = 0.12 [0.0..1.0] (
        label = "Release"
    );
)]
pub struct SoundCardSpectrumAnalyzer {}

impl SoundCardSpectrumAnalyzer {
    pub(crate) fn for_source(source: NodeReference) -> Self {
        let mut analyzer = Self::new();
        analyzer
            .source
            .apply_runtime_value(&ParamValue::Reference(source));
        analyzer
    }
}

#[node("sound_card_spectrum_analyzer", from_struct)]
impl Node for SoundCardSpectrumAnalyzer {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("sound_card_spectrum_band", label = "Spectrum Band")]
#[children(
    index: i32 = 0 (
        label = "Index",
        read_only = true
    );
    low_hz: f64 = 0.0 (
        label = "Low Frequency",
        read_only = true
    );
    center_hz: f64 = 0.0 (
        label = "Center Frequency",
        read_only = true
    );
    high_hz: f64 = 0.0 (
        label = "High Frequency",
        read_only = true
    );
    linear_amplitude: f64 = 0.0 (
        label = "Linear Amplitude",
        read_only = true
    );
    dbfs: f64 = -120.0 (
        label = "dBFS",
        read_only = true
    );
)]
pub struct SoundCardSpectrumBand {}

impl SoundCardSpectrumBand {
    pub(crate) fn for_index(index: usize) -> Self {
        let mut band = Self::new();
        band
            .index
            .apply_runtime_value(&ParamValue::Int(index as i32));
        band
    }
}

#[node("sound_card_spectrum_band", from_struct)]
impl Node for SoundCardSpectrumBand {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        self.node_data_mut().meta.user_permissions =
            golden_core::node::NodeUserPermissions::none();
        self.node_data_mut().meta.can_be_disabled = false;
    }

    fn lifecycle_requires_tree_snapshot(&self) -> bool {
        false
    }

    fn inbox_requires_tree_snapshot(&self, _events: &golden_core::events::EventFrame) -> bool {
        false
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}
