use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue, ParameterEnumOption},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
mod bytes;
mod preparation;
pub(crate) use bytes::{append_midi_bytes, normalize_midi_sysex, parse_midi_bytes};
pub(crate) use preparation::{prepare_midi_command, MidiSendPacket, MidiSendRequest};

use crate::app::{
    cc_supports_14_bit, clamp_channel_i32, clamp_i32_to_u14, clamp_i32_to_u7, encode_14_bit_control_change,
    encode_midi_message, message_description, note_pitch_from_name_octave, MidiMessage, MidiSystemMessage,
};

const NOTE_MODE_PITCH: &str = "pitch";
const NOTE_MODE_OCTAVE_NOTE: &str = "octave_note";
const DEFAULT_CHANNEL: i32 = 1;
const DEFAULT_PITCH: i32 = 60;
const DEFAULT_VELOCITY: i32 = 127;
const DEFAULT_NOTE_NAME: &str = "C";
const DEFAULT_OCTAVE: i32 = 4;

const SYSTEM_COMMON_TIME_CODE_QUARTER_FRAME: &str = "time_code_quarter_frame";
const SYSTEM_COMMON_SONG_POSITION: &str = "song_position";
const SYSTEM_COMMON_SONG_SELECT: &str = "song_select";
const SYSTEM_COMMON_TUNE_REQUEST: &str = "tune_request";

const SYSTEM_REALTIME_TIMING_CLOCK: &str = "timing_clock";
const SYSTEM_REALTIME_START: &str = "start";
const SYSTEM_REALTIME_CONTINUE: &str = "continue";
const SYSTEM_REALTIME_STOP: &str = "stop";
const SYSTEM_REALTIME_ACTIVE_SENSING: &str = "active_sensing";
const SYSTEM_REALTIME_RESET: &str = "reset";

fn midi_command_child_event_interest_depth(event: &Event) -> u32 {
    match event.kind {
        EventKind::ParamChanged { .. } => u32::MAX,
        _ => 0,
    }
}

fn handle_midi_command_param_change<T: Node>(command: &T, ctx: &mut ProcessCtx, param: NodeId) {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return;
    };
    if crate::app::module_command::module_command_triggered(&snapshot, command.id(), param) {
        dispatch(command, ctx, None);
    }
}
fn handle_midi_command_execute_event<T: Node>(
    command: &T,
    ctx: &mut ProcessCtx,
    event: &golden_core::events::CustomEvent,
) {
    if event.topic == crate::app::module_command::MODULE_COMMAND_EXECUTE_TOPIC
        && event.payload.get("command_id").and_then(serde_json::Value::as_u64) == Some(command.id().0)
    {
        dispatch(command, ctx, Some(event.clone()));
    }
}
fn dispatch<T: Node>(command: &T, ctx: &mut ProcessCtx, event: Option<golden_core::events::CustomEvent>) {
    let Some(snapshot) = ctx.tree_snapshot_arc() else {
        return;
    };
    let result =
        crate::app::module_command::CapturedInvocation::capture(&snapshot, command.id(), event, &["midi_module"])
            .and_then(|invocation| crate::app::MidiModule::dispatch_command(ctx, invocation));
    if let Err(error) = result {
        golden_core::logerror!(format!("MIDI command was not admitted: {error}"));
    }
}

#[node("midi_note_command_base", label = "Note Command")]
#[children(
    [base_children];
    channel: i32 = DEFAULT_CHANNEL [1..16] (
        label = "Channel",
        description = "1-based MIDI channel."
    );
    note_mode: Enum = NOTE_MODE_PITCH (
        label = "Note Mode",
        description = "Whether the note is selected by MIDI pitch or by octave and note name.",
        enum_options = note_mode_options()
    );
    pitch: i32 = DEFAULT_PITCH [0..127] (
        label = "Pitch",
        description = "MIDI note pitch used when Note Mode is Pitch.",
        dependency = note_mode == NOTE_MODE_PITCH
    );
    octave: i32 = DEFAULT_OCTAVE [-1..9] (
        label = "Octave",
        description = "Octave used when Note Mode is Octave + Note.",
        dependency = note_mode == NOTE_MODE_OCTAVE_NOTE
    );
    note: Enum = DEFAULT_NOTE_NAME (
        label = "Note",
        description = "Note name used when Note Mode is Octave + Note.",
        enum_options = note_name_options(),
        dependency = note_mode == NOTE_MODE_OCTAVE_NOTE
    );
)]
struct MidiNoteCommandBase {
    base: crate::app::ModuleCommandBase,
}

impl MidiNoteCommandBase {
    fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[node("midi_note_command_base", via = base, from_struct)]
impl Node for MidiNoteCommandBase {}

#[node("midi_send_note_on_command", label = "Send Note On")]
#[children(
    [base_children];
    velocity: i32 = DEFAULT_VELOCITY [0..127] (
        label = "Velocity",
        description = "Note-on velocity."
    );
)]
pub struct MidiSendNoteOnCommand {
    base: MidiNoteCommandBase,
}

impl MidiSendNoteOnCommand {
    pub fn create() -> Self {
        Self::new(MidiNoteCommandBase::create())
    }
}

#[golden_core::item("module_command", node = "midi_send_note_on_command", via = base, from_struct)]
impl Node for MidiSendNoteOnCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_note_off_command", label = "Send Note Off")]
#[children(
    [base_children];
    velocity: i32 = 0 [0..127] (
        label = "Velocity",
        description = "Note-off release velocity."
    );
)]
pub struct MidiSendNoteOffCommand {
    base: MidiNoteCommandBase,
}

impl MidiSendNoteOffCommand {
    pub fn create() -> Self {
        Self::new(MidiNoteCommandBase::create())
    }
}

#[golden_core::item("module_command", node = "midi_send_note_off_command", via = base, from_struct)]
impl Node for MidiSendNoteOffCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_full_note_command", label = "Send Full Note")]
#[children(
    [base_children];
    velocity: i32 = DEFAULT_VELOCITY [0..127] (
        label = "Velocity",
        description = "Note-on velocity."
    );
    off_velocity: i32 = 0 [0..127] (
        label = "Off Velocity",
        description = "Note-off release velocity."
    );
    duration_ms: i32 = 100 [0..2147483647] (
        label = "Duration",
        description = "Delay before sending the note-off, in milliseconds.",
        widget = "text"
    );
)]
pub struct MidiSendFullNoteCommand {
    base: MidiNoteCommandBase,
}

impl MidiSendFullNoteCommand {
    pub fn create() -> Self {
        Self::new(MidiNoteCommandBase::create())
    }
}

#[golden_core::item("module_command", node = "midi_send_full_note_command", via = base, from_struct)]
impl Node for MidiSendFullNoteCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_control_change_command", label = "Send Control Change")]
#[children(
    channel: i32 = DEFAULT_CHANNEL [1..16] (
        label = "Channel",
        description = "1-based MIDI channel."
    );
    controller: i32 = 1 [0..127] (
        label = "Controller",
        description = "Control-change number. For 14-bit MIDI, use an MSB controller in the 0-31 range; the matching LSB is sent automatically on controller +32."
    );
    value: i32 = 0 [0..127] (
        label = "Value",
        description = "7-bit control-change value."
    );
    is_14_bit: bool = false (
        label = "14-bit",
        description = "Send this control change as an MSB/LSB 14-bit pair using this controller and its paired LSB controller at +32."
    );
    value_14_bit: i32 = 0 [0..16383] (
        label = "14-bit Value",
        description = "14-bit value used when 14-bit is enabled; it is split across the selected MSB controller and its paired LSB controller at +32."
    );
)]
pub struct MidiSendControlChangeCommand {
    base: crate::app::ModuleCommandBase,
}

impl MidiSendControlChangeCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "midi_send_control_change_command", via = base, from_struct)]
impl Node for MidiSendControlChangeCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_program_change_command", label = "Send Program Change")]
#[children(
    channel: i32 = DEFAULT_CHANNEL [1..16] (
        label = "Channel",
        description = "1-based MIDI channel."
    );
    program: i32 = 0 [0..127] (
        label = "Program",
        description = "Program number."
    );
)]
pub struct MidiSendProgramChangeCommand {
    base: crate::app::ModuleCommandBase,
}

impl MidiSendProgramChangeCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "midi_send_program_change_command", via = base, from_struct)]
impl Node for MidiSendProgramChangeCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_pitch_bend_command", label = "Send Pitch Bend")]
#[children(
    channel: i32 = DEFAULT_CHANNEL [1..16] (
        label = "Channel",
        description = "1-based MIDI channel."
    );
    value: i32 = 8192 [0..16383] (
        label = "Value",
        description = "14-bit pitch-bend value. Center is 8192."
    );
)]
pub struct MidiSendPitchBendCommand {
    base: crate::app::ModuleCommandBase,
}

impl MidiSendPitchBendCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "midi_send_pitch_bend_command", via = base, from_struct)]
impl Node for MidiSendPitchBendCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_channel_pressure_command", label = "Send Channel Pressure")]
#[children(
    channel: i32 = DEFAULT_CHANNEL [1..16] (
        label = "Channel",
        description = "1-based MIDI channel."
    );
    pressure: i32 = 0 [0..127] (
        label = "Pressure",
        description = "Channel pressure value."
    );
)]
pub struct MidiSendChannelPressureCommand {
    base: crate::app::ModuleCommandBase,
}

impl MidiSendChannelPressureCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "midi_send_channel_pressure_command", via = base, from_struct)]
impl Node for MidiSendChannelPressureCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_poly_pressure_command", label = "Send Poly Pressure")]
#[children(
    [base_children];
    pressure: i32 = 0 [0..127] (
        label = "Pressure",
        description = "Polyphonic pressure value."
    );
)]
pub struct MidiSendPolyPressureCommand {
    base: MidiNoteCommandBase,
}

impl MidiSendPolyPressureCommand {
    pub fn create() -> Self {
        Self::new(MidiNoteCommandBase::create())
    }
}

#[golden_core::item("module_command", node = "midi_send_poly_pressure_command", via = base, from_struct)]
impl Node for MidiSendPolyPressureCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_system_common_command", label = "Send System Common")]
#[children(
    message: Enum = SYSTEM_COMMON_TUNE_REQUEST (
        label = "Message",
        description = "System common message to send.",
        enum_options = system_common_message_options()
    );
    quarter_frame: i32 = 0 [0..127] (
        label = "Quarter Frame",
        description = "Time-code quarter-frame value."
    );
    song_position: i32 = 0 [0..16383] (
        label = "Song Position",
        description = "14-bit song-position pointer."
    );
    song: i32 = 0 [0..127] (
        label = "Song",
        description = "Song-select number."
    );
)]
pub struct MidiSendSystemCommonCommand {
    base: crate::app::ModuleCommandBase,
}

impl MidiSendSystemCommonCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "midi_send_system_common_command", via = base, from_struct)]
impl Node for MidiSendSystemCommonCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_system_realtime_command", label = "Send System Realtime")]
#[children(
    message: Enum = SYSTEM_REALTIME_START (
        label = "Message",
        description = "System realtime message to send.",
        enum_options = system_realtime_message_options()
    );
)]
pub struct MidiSendSystemRealtimeCommand {
    base: crate::app::ModuleCommandBase,
}

impl MidiSendSystemRealtimeCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "midi_send_system_realtime_command", via = base, from_struct)]
impl Node for MidiSendSystemRealtimeCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_sysex_bytes_command", label = "Send Sysex Bytes")]
#[children(
    bytes: String = "F0 7D F7".to_string() (
        label = "Bytes",
        description = "SysEx bytes as hex or decimal tokens. F0 and F7 are added when omitted.",
        widget = "textarea"
    );
)]
pub struct MidiSendSysexBytesCommand {
    base: crate::app::ModuleCommandBase,
}

impl MidiSendSysexBytesCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "midi_send_sysex_bytes_command", via = base, from_struct)]
impl Node for MidiSendSysexBytesCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_sysex_string_command", label = "Send Sysex String")]
#[children(
    text: String = String::new() (
        label = "Text",
        description = "UTF-8 text sent as the SysEx payload. F0 and F7 are added around the text bytes.",
        widget = "textarea"
    );
)]
pub struct MidiSendSysexStringCommand {
    base: crate::app::ModuleCommandBase,
}

impl MidiSendSysexStringCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "midi_send_sysex_string_command", via = base, from_struct)]
impl Node for MidiSendSysexStringCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

#[node("midi_send_raw_bytes_command", label = "Send Raw MIDI Bytes")]
#[children(
    bytes: String = String::new() (
        label = "Bytes",
        description = "Raw MIDI bytes as hex or decimal tokens.",
        widget = "textarea"
    );
)]
pub struct MidiSendRawBytesCommand {
    base: crate::app::ModuleCommandBase,
}

impl MidiSendRawBytesCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("module_command", node = "midi_send_raw_bytes_command", via = base, from_struct)]
impl Node for MidiSendRawBytesCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        midi_command_child_event_interest_depth(event)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        handle_midi_command_param_change(self, ctx, param);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event);
    }
}

fn validate_14_bit_controller(controller: u8) -> Result<u8, String> {
    if cc_supports_14_bit(controller) {
        return Ok(controller);
    }

    Err(format!(
        "14-bit CC requires an MSB controller in the 0-31 range; controller {controller} uses a paired LSB role instead"
    ))
}

fn enum_option(variant_id: &str, label: &str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}

fn note_mode_options() -> Vec<ParameterEnumOption> {
    vec![
        enum_option(NOTE_MODE_PITCH, "Pitch", 0),
        enum_option(NOTE_MODE_OCTAVE_NOTE, "Octave + Note", 1),
    ]
}

fn note_name_options() -> Vec<ParameterEnumOption> {
    ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
        .into_iter()
        .enumerate()
        .map(|(index, note)| enum_option(note, note, index as i32))
        .collect()
}

fn system_common_message_options() -> Vec<ParameterEnumOption> {
    [
        (SYSTEM_COMMON_TUNE_REQUEST, "Tune Request"),
        (SYSTEM_COMMON_TIME_CODE_QUARTER_FRAME, "Time Code Quarter Frame"),
        (SYSTEM_COMMON_SONG_POSITION, "Song Position"),
        (SYSTEM_COMMON_SONG_SELECT, "Song Select"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (variant, label))| enum_option(variant, label, index as i32))
    .collect()
}

fn system_realtime_message_options() -> Vec<ParameterEnumOption> {
    [
        (SYSTEM_REALTIME_START, "Start"),
        (SYSTEM_REALTIME_CONTINUE, "Continue"),
        (SYSTEM_REALTIME_STOP, "Stop"),
        (SYSTEM_REALTIME_TIMING_CLOCK, "Timing Clock"),
        (SYSTEM_REALTIME_ACTIVE_SENSING, "Active Sensing"),
        (SYSTEM_REALTIME_RESET, "Reset"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (variant, label))| enum_option(variant, label, index as i32))
    .collect()
}

#[cfg(test)]
mod tests;
