use golden_core::{
    node,
    node::{Node, NodeReference, NodeUserPermissions},
    parameter::{ParamValue, ReferenceTargetKind},
    process_ctx::ProcessCtx,
};

pub(crate) const SOUND_CARD_INPUT_GAIN_FILTER_KEY: &str = "sound_card_same_module_input_gain";
pub(crate) const SOUND_CARD_OUTPUT_GAIN_FILTER_KEY: &str = "sound_card_same_module_output_gain";

fn input_channel_reference() -> NodeReference {
    NodeReference::default()
}

fn output_channel_reference() -> NodeReference {
    NodeReference::default()
}

fn initialize_backend_owned(node: &mut dyn Node) {
    let data = node.node_data_mut();
    data.meta.user_permissions = NodeUserPermissions::none();
    data.meta.can_be_disabled = false;
}

#[node("sound_card_input_routing", label = "Input Routing")]
#[children(
    channel_count: i32 = 2 [1..256] (
        label = "Input Channels",
        description = "Number of application input channels."
    );
    node routes: SoundCardInputRouteList = SoundCardInputRouteList::new() (
        label = "Routes"
    );
)]
pub struct SoundCardInputRouting {}

#[node("sound_card_input_routing", from_struct)]
impl Node for SoundCardInputRouting {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
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

#[node("sound_card_output_routing", label = "Output Routing")]
#[children(
    channel_count: i32 = 2 [1..256] (
        label = "Output Channels",
        description = "Number of application output channels."
    );
    node routes: SoundCardOutputRouteList = SoundCardOutputRouteList::new() (
        label = "Routes"
    );
)]
pub struct SoundCardOutputRouting {}

#[node("sound_card_output_routing", from_struct)]
impl Node for SoundCardOutputRouting {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
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

#[node("sound_card_input_route_list", label = "Input Routes")]
pub struct SoundCardInputRouteList {}

#[node("sound_card_input_route_list", from_struct)]
impl Node for SoundCardInputRouteList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_output_route_list", label = "Output Routes")]
pub struct SoundCardOutputRouteList {}

#[node("sound_card_output_route_list", from_struct)]
impl Node for SoundCardOutputRouteList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_input_route", label = "Input Route")]
#[children(
    physical_channel: String = "input:0".to_string() (
        label = "Device Channel",
        read_only = true
    );
    channel: NodeReference = input_channel_reference() (
        label = "Input Channel",
        read_only = true,
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec!["float".to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_INPUT_GAIN_FILTER_KEY.to_string())
    );
)]
pub struct SoundCardInputRoute {}

impl SoundCardInputRoute {
    pub(crate) fn connected(physical_channel: impl Into<String>, channel: NodeReference) -> Self {
        let mut route = Self::new();
        route
            .physical_channel
            .apply_runtime_value(&ParamValue::Str(physical_channel.into()));
        route.channel.apply_runtime_value(&ParamValue::Reference(channel));
        route
    }
}

#[node("sound_card_input_route", from_struct)]
impl Node for SoundCardInputRoute {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_output_route", label = "Output Route")]
#[children(
    channel: NodeReference = output_channel_reference() (
        label = "Output Channel",
        read_only = true,
        reference_target_kind = ReferenceTargetKind::AnyNode,
        reference_allowed_node_types = vec!["float".to_string()],
        reference_custom_filter_key = Some(SOUND_CARD_OUTPUT_GAIN_FILTER_KEY.to_string())
    );
    physical_channel: String = "output:0".to_string() (
        label = "Device Channel",
        read_only = true
    );
)]
pub struct SoundCardOutputRoute {}

impl SoundCardOutputRoute {
    pub(crate) fn connected(channel: NodeReference, physical_channel: impl Into<String>) -> Self {
        let mut route = Self::new();
        route.channel.apply_runtime_value(&ParamValue::Reference(channel));
        route
            .physical_channel
            .apply_runtime_value(&ParamValue::Str(physical_channel.into()));
        route
    }
}

#[node("sound_card_output_route", from_struct)]
impl Node for SoundCardOutputRoute {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_input_parameters", label = "Input")]
#[children(
    master_gain_db: f64 = 0.0 [-120.0..24.0] (
        label = "Master Input Gain"
    );
    node channels: SoundCardInputChannelList = SoundCardInputChannelList::new() (
        label = "Channels"
    );
)]
pub struct SoundCardInputParameters {}

#[node("sound_card_input_parameters", from_struct)]
impl Node for SoundCardInputParameters {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_output_parameters", label = "Output")]
#[children(
    master_gain_db: f64 = 0.0 [-120.0..24.0] (
        label = "Master Output Gain"
    );
    node channels: SoundCardOutputChannelList = SoundCardOutputChannelList::new() (
        label = "Channels"
    );
)]
pub struct SoundCardOutputParameters {}

#[node("sound_card_output_parameters", from_struct)]
impl Node for SoundCardOutputParameters {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_input_channel_list", label = "Input Channels")]
pub struct SoundCardInputChannelList {}

#[node("sound_card_input_channel_list", from_struct)]
impl Node for SoundCardInputChannelList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_output_channel_list", label = "Output Channels")]
pub struct SoundCardOutputChannelList {}

#[node("sound_card_output_channel_list", from_struct)]
impl Node for SoundCardOutputChannelList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_input_values", label = "Input")]
#[children(
    master_level: f64 = 0.0 [0.0..1.0] (
        label = "Master Input Level",
        read_only = true
    );
    node channels: SoundCardChannelValueList = SoundCardChannelValueList::new() (
        label = "Channels"
    );
)]
pub struct SoundCardInputValues {}

#[node("sound_card_input_values", from_struct)]
impl Node for SoundCardInputValues {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_output_values", label = "Output")]
#[children(
    master_level: f64 = 0.0 [0.0..1.0] (
        label = "Master Output Level",
        read_only = true
    );
    node channels: SoundCardChannelValueList = SoundCardChannelValueList::new() (
        label = "Channels"
    );
)]
pub struct SoundCardOutputValues {}

#[node("sound_card_output_values", from_struct)]
impl Node for SoundCardOutputValues {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_channel_value_list", label = "Channels")]
pub struct SoundCardChannelValueList {}

#[node("sound_card_channel_value_list", from_struct)]
impl Node for SoundCardChannelValueList {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_pitch_values", label = "Pitch Detection")]
#[children(
    valid: bool = false (
        label = "Valid",
        read_only = true
    );
    frequency_hz: f64 = 0.0 (
        label = "Frequency",
        read_only = true
    );
    confidence: f64 = 0.0 (
        label = "Confidence",
        read_only = true
    );
    midi_note: f64 = 0.0 (
        label = "MIDI Note",
        read_only = true
    );
    note_name: String = String::new() (
        label = "Note Name",
        read_only = true
    );
    cents: f64 = 0.0 (
        label = "Cents",
        read_only = true
    );
)]
pub struct SoundCardPitchValues {}

#[node("sound_card_pitch_values", from_struct)]
impl Node for SoundCardPitchValues {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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

#[node("sound_card_spectral_values", label = "Spectral Analysis")]
pub struct SoundCardSpectralValues {}

#[node("sound_card_spectral_values", from_struct)]
impl Node for SoundCardSpectralValues {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        initialize_backend_owned(self);
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
