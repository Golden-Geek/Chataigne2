use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::{Enum, ParamValue, ParameterEnumOption},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};
use serde::{Deserialize, Serialize};

use crate::app::{
    cc_supports_14_bit, clamp_channel_i32, clamp_i32_to_u14, clamp_i32_to_u7, encode_14_bit_control_change,
    encode_midi_message, message_description, normalize_sysex_bytes, note_pitch_from_name_octave, MidiMessage,
    MidiSystemMessage,
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct MidiSendRequest {
    pub packets: Vec<MidiSendPacket>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct MidiSendPacket {
    pub bytes: Vec<u8>,
    pub description: String,
    pub delay_ms: u64,
}

impl MidiSendRequest {
    fn immediate(message: MidiMessage) -> Self {
        Self {
            packets: vec![MidiSendPacket::immediate(message)],
        }
    }

    fn immediate_many(messages: Vec<MidiMessage>, description: String) -> Self {
        Self {
            packets: messages
                .into_iter()
                .map(|message| MidiSendPacket {
                    bytes: encode_midi_message(&message),
                    description: description.clone(),
                    delay_ms: 0,
                })
                .collect(),
        }
    }
}

impl MidiSendPacket {
    fn immediate(message: MidiMessage) -> Self {
        Self {
            bytes: encode_midi_message(&message),
            description: message_description(&message),
            delay_ms: 0,
        }
    }

    fn delayed(message: MidiMessage, delay_ms: u64) -> Self {
        Self {
            bytes: encode_midi_message(&message),
            description: message_description(&message),
            delay_ms,
        }
    }

    fn immediate_bytes(bytes: Vec<u8>, description: impl Into<String>) -> Self {
        Self {
            bytes,
            description: description.into(),
            delay_ms: 0,
        }
    }
}

fn midi_command_child_event_interest_depth(event: &Event) -> u32 {
    match event.kind {
        EventKind::ParamChanged { .. } => u32::MAX,
        _ => 0,
    }
}

fn handle_midi_command_param_change<TCommand, TPayload, TRequest>(
    command: &TCommand,
    ctx: &mut ProcessCtx,
    param: NodeId,
    context: &str,
    request_payload: TRequest,
) where
    TCommand: Node,
    TPayload: Serialize,
    TRequest: FnOnce(&TCommand, &ProcessTreeSnapshot) -> Result<TPayload, String>,
{
    let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
        return;
    };
    let snapshot = snapshot_arc.as_ref();
    if !crate::app::module_command::module_command_triggered(snapshot, command.id(), param) {
        return;
    }

    if let Err(error) = request_payload(command, snapshot).and_then(|payload| {
        crate::app::module_command::emit_module_command_request(
            ctx,
            snapshot,
            command.id(),
            command.get_type(),
            &payload,
        )
    }) {
        golden_core::logerror!(format!("Failed to trigger {context}: {error}"));
    }
}

fn handle_midi_command_execute_event<TCommand, TPayload, TRequest>(
    command: &TCommand,
    ctx: &mut ProcessCtx,
    event: &golden_core::events::CustomEvent,
    context: &str,
    request_payload: TRequest,
) where
    TCommand: Node,
    TPayload: Serialize,
    TRequest: FnOnce(&TCommand, &ProcessTreeSnapshot) -> Result<TPayload, String>,
{
    if !crate::app::module_command::is_command_execute_request(event, command.id()) {
        return;
    }
    let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
        return;
    };
    let snapshot = crate::app::module_command::command_execute_snapshot(
        event,
        snapshot_arc.as_ref(),
        command.id(),
    );
    let snapshot = snapshot.as_ref();
    if let Err(error) = request_payload(command, snapshot).and_then(|payload| {
        crate::app::module_command::emit_module_command_request(
            ctx,
            snapshot,
            command.id(),
            command.get_type(),
            &payload,
        )
    }) {
        golden_core::logerror!(format!("Failed to execute {context}: {error}"));
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

#[derive(Clone, Copy)]
struct MidiNoteTarget {
    channel: u8,
    note: u8,
}

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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        let target = command_note_target(snapshot, self.id())?;
        Ok(MidiSendRequest::immediate(MidiMessage::NoteOn {
            channel: target.channel,
            note: target.note,
            velocity: command_u7(snapshot, self.id(), "velocity", DEFAULT_VELOCITY),
        }))
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
        handle_midi_command_param_change(self, ctx, param, "MIDI note-on command", |command, snapshot| {
            command.request_payload(snapshot)
        });
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event, "MIDI note-on command", |command, snapshot| {
            command.request_payload(snapshot)
        });
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        let target = command_note_target(snapshot, self.id())?;
        Ok(MidiSendRequest::immediate(MidiMessage::NoteOff {
            channel: target.channel,
            note: target.note,
            velocity: command_u7(snapshot, self.id(), "velocity", 0),
        }))
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
        handle_midi_command_param_change(self, ctx, param, "MIDI note-off command", |command, snapshot| {
            command.request_payload(snapshot)
        });
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event, "MIDI note-off command", |command, snapshot| {
            command.request_payload(snapshot)
        });
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        let target = command_note_target(snapshot, self.id())?;
        let duration_ms = command_int(snapshot, self.id(), "duration_ms", 100).max(0) as u64;
        Ok(MidiSendRequest {
            packets: vec![
                MidiSendPacket::immediate(MidiMessage::NoteOn {
                    channel: target.channel,
                    note: target.note,
                    velocity: command_u7(snapshot, self.id(), "velocity", DEFAULT_VELOCITY),
                }),
                MidiSendPacket::delayed(
                    MidiMessage::NoteOff {
                        channel: target.channel,
                        note: target.note,
                        velocity: command_u7(snapshot, self.id(), "off_velocity", 0),
                    },
                    duration_ms,
                ),
            ],
        })
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
        handle_midi_command_param_change(self, ctx, param, "MIDI full-note command", |command, snapshot| {
            command.request_payload(snapshot)
        });
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event, "MIDI full-note command", |command, snapshot| {
            command.request_payload(snapshot)
        });
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        let channel = command_channel(snapshot, self.id());
        let controller = command_u7(snapshot, self.id(), "controller", 1);
        if command_bool(snapshot, self.id(), "is_14_bit", false) {
            let controller = validate_14_bit_controller(controller)?;
            let value = clamp_i32_to_u14(command_int(snapshot, self.id(), "value_14_bit", 0));
            return Ok(MidiSendRequest::immediate_many(
                encode_14_bit_control_change(channel, controller, value),
                format!("14-bit cc ch{} {} value {}", channel, controller, value),
            ));
        }

        Ok(MidiSendRequest::immediate(MidiMessage::ControlChange {
            channel,
            controller,
            value: command_u7(snapshot, self.id(), "value", 0),
        }))
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
        handle_midi_command_param_change(
            self,
            ctx,
            param,
            "MIDI control-change command",
            |command, snapshot| command.request_payload(snapshot),
        );
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(
            self,
            ctx,
            &event,
            "MIDI control-change command",
            |command, snapshot| command.request_payload(snapshot),
        );
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        Ok(MidiSendRequest::immediate(MidiMessage::ProgramChange {
            channel: command_channel(snapshot, self.id()),
            program: command_u7(snapshot, self.id(), "program", 0),
        }))
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
        handle_midi_command_param_change(
            self,
            ctx,
            param,
            "MIDI program-change command",
            |command, snapshot| command.request_payload(snapshot),
        );
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(
            self,
            ctx,
            &event,
            "MIDI program-change command",
            |command, snapshot| command.request_payload(snapshot),
        );
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        Ok(MidiSendRequest::immediate(MidiMessage::PitchBend {
            channel: command_channel(snapshot, self.id()),
            value: clamp_i32_to_u14(command_int(snapshot, self.id(), "value", 8192)),
        }))
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
        handle_midi_command_param_change(self, ctx, param, "MIDI pitch-bend command", |command, snapshot| {
            command.request_payload(snapshot)
        });
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event, "MIDI pitch-bend command", |command, snapshot| {
            command.request_payload(snapshot)
        });
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        Ok(MidiSendRequest::immediate(MidiMessage::ChannelPressure {
            channel: command_channel(snapshot, self.id()),
            pressure: command_u7(snapshot, self.id(), "pressure", 0),
        }))
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
        handle_midi_command_param_change(
            self,
            ctx,
            param,
            "MIDI channel-pressure command",
            |command, snapshot| command.request_payload(snapshot),
        );
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(
            self,
            ctx,
            &event,
            "MIDI channel-pressure command",
            |command, snapshot| command.request_payload(snapshot),
        );
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        let target = command_note_target(snapshot, self.id())?;
        Ok(MidiSendRequest::immediate(MidiMessage::PolyPressure {
            channel: target.channel,
            note: target.note,
            pressure: command_u7(snapshot, self.id(), "pressure", 0),
        }))
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
        handle_midi_command_param_change(self, ctx, param, "MIDI poly-pressure command", |command, snapshot| {
            command.request_payload(snapshot)
        });
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event, "MIDI poly-pressure command", |command, snapshot| {
            command.request_payload(snapshot)
        });
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        let message = command_enum(snapshot, self.id(), "message", SYSTEM_COMMON_TUNE_REQUEST);
        let system = match message.as_str() {
            SYSTEM_COMMON_TIME_CODE_QUARTER_FRAME => MidiSystemMessage::TimeCodeQuarterFrame {
                value: command_u7(snapshot, self.id(), "quarter_frame", 0),
            },
            SYSTEM_COMMON_SONG_POSITION => MidiSystemMessage::SongPosition {
                position: clamp_i32_to_u14(command_int(snapshot, self.id(), "song_position", 0)),
            },
            SYSTEM_COMMON_SONG_SELECT => MidiSystemMessage::SongSelect {
                song: command_u7(snapshot, self.id(), "song", 0),
            },
            SYSTEM_COMMON_TUNE_REQUEST => MidiSystemMessage::TuneRequest,
            other => return Err(format!("unsupported system common message '{other}'")),
        };

        Ok(MidiSendRequest::immediate(MidiMessage::System(system)))
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
        handle_midi_command_param_change(self, ctx, param, "MIDI system-common command", |command, snapshot| {
            command.request_payload(snapshot)
        });
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event, "MIDI system-common command", |command, snapshot| {
            command.request_payload(snapshot)
        });
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        let message = command_enum(snapshot, self.id(), "message", SYSTEM_REALTIME_START);
        let system = match message.as_str() {
            SYSTEM_REALTIME_TIMING_CLOCK => MidiSystemMessage::TimingClock,
            SYSTEM_REALTIME_START => MidiSystemMessage::Start,
            SYSTEM_REALTIME_CONTINUE => MidiSystemMessage::Continue,
            SYSTEM_REALTIME_STOP => MidiSystemMessage::Stop,
            SYSTEM_REALTIME_ACTIVE_SENSING => MidiSystemMessage::ActiveSensing,
            SYSTEM_REALTIME_RESET => MidiSystemMessage::Reset,
            other => return Err(format!("unsupported system realtime message '{other}'")),
        };

        Ok(MidiSendRequest::immediate(MidiMessage::System(system)))
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
        handle_midi_command_param_change(
            self,
            ctx,
            param,
            "MIDI system-realtime command",
            |command, snapshot| command.request_payload(snapshot),
        );
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(
            self,
            ctx,
            &event,
            "MIDI system-realtime command",
            |command, snapshot| command.request_payload(snapshot),
        );
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        let bytes = command_string(snapshot, self.id(), "bytes").unwrap_or_default();
        Ok(MidiSendRequest {
            packets: vec![MidiSendPacket::immediate_bytes(
                normalize_sysex_bytes(parse_midi_byte_list(bytes.as_str())?.as_slice()),
                "sysex bytes",
            )],
        })
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
        handle_midi_command_param_change(self, ctx, param, "MIDI sysex-bytes command", |command, snapshot| {
            command.request_payload(snapshot)
        });
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event, "MIDI sysex-bytes command", |command, snapshot| {
            command.request_payload(snapshot)
        });
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        let text = command_string(snapshot, self.id(), "text").unwrap_or_default();
        Ok(MidiSendRequest {
            packets: vec![MidiSendPacket::immediate_bytes(
                normalize_sysex_bytes(text.as_bytes()),
                "sysex string",
            )],
        })
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
        handle_midi_command_param_change(self, ctx, param, "MIDI sysex-string command", |command, snapshot| {
            command.request_payload(snapshot)
        });
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event, "MIDI sysex-string command", |command, snapshot| {
            command.request_payload(snapshot)
        });
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

    fn request_payload(&self, snapshot: &ProcessTreeSnapshot) -> Result<MidiSendRequest, String> {
        let bytes = command_string(snapshot, self.id(), "bytes").unwrap_or_default();
        Ok(MidiSendRequest {
            packets: vec![MidiSendPacket::immediate_bytes(
                parse_midi_byte_list(bytes.as_str())?,
                "raw MIDI bytes",
            )],
        })
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
        handle_midi_command_param_change(self, ctx, param, "MIDI raw-bytes command", |command, snapshot| {
            command.request_payload(snapshot)
        });
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        handle_midi_command_execute_event(self, ctx, &event, "MIDI raw-bytes command", |command, snapshot| {
            command.request_payload(snapshot)
        });
    }
}

fn command_channel(snapshot: &ProcessTreeSnapshot, command_id: NodeId) -> u8 {
    clamp_channel_i32(command_int(snapshot, command_id, "channel", DEFAULT_CHANNEL))
}

fn command_note_target(snapshot: &ProcessTreeSnapshot, command_id: NodeId) -> Result<MidiNoteTarget, String> {
    Ok(MidiNoteTarget {
        channel: command_channel(snapshot, command_id),
        note: command_note_pitch(snapshot, command_id)?,
    })
}

fn command_note_pitch(snapshot: &ProcessTreeSnapshot, command_id: NodeId) -> Result<u8, String> {
    let mode = command_enum(snapshot, command_id, "note_mode", NOTE_MODE_PITCH);
    if mode == NOTE_MODE_OCTAVE_NOTE {
        let note = command_enum(snapshot, command_id, "note", DEFAULT_NOTE_NAME);
        let octave = command_int(snapshot, command_id, "octave", DEFAULT_OCTAVE);
        return note_pitch_from_name_octave(note.as_str(), octave)
            .ok_or_else(|| format!("note {note}{octave} is outside the MIDI 0-127 pitch range"));
    }

    Ok(command_u7(snapshot, command_id, "pitch", DEFAULT_PITCH))
}

fn command_u7(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str, fallback: i32) -> u8 {
    clamp_i32_to_u7(command_int(snapshot, command_id, path, fallback))
}

fn command_int(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str, fallback: i32) -> i32 {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)
        .and_then(|param_id| {
            snapshot
                .node(param_id)
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_int)
        })
        .unwrap_or(fallback)
}

fn command_bool(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str, fallback: bool) -> bool {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)
        .and_then(|param_id| {
            snapshot
                .node(param_id)
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_bool)
        })
        .unwrap_or(fallback)
}

fn command_enum(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str, fallback: &str) -> String {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path)
        .and_then(|param_id| {
            snapshot
                .node(param_id)
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_enum)
        })
        .unwrap_or_else(|| fallback.to_string())
}

fn command_string(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    crate::app::module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
}

fn parse_midi_byte_list(text: &str) -> Result<Vec<u8>, String> {
    let tokens = text
        .split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();

    if tokens.len() == 1 && looks_like_compact_hex(tokens[0]) {
        return parse_compact_hex(tokens[0]);
    }

    tokens.into_iter().map(parse_midi_byte_token).collect()
}

fn parse_midi_byte_token(token: &str) -> Result<u8, String> {
    if let Some(hex) = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")) {
        return u8::from_str_radix(hex, 16).map_err(|error| format!("invalid MIDI byte '{token}': {error}"));
    }

    if token.chars().any(|character| matches!(character, 'a'..='f' | 'A'..='F')) {
        return u8::from_str_radix(token, 16).map_err(|error| format!("invalid MIDI byte '{token}': {error}"));
    }

    token
        .parse::<u8>()
        .map_err(|error| format!("invalid MIDI byte '{token}': {error}"))
}

fn looks_like_compact_hex(token: &str) -> bool {
    token.len() > 2
        && token.len() % 2 == 0
        && token.chars().all(|character| character.is_ascii_hexdigit())
        && token
            .chars()
            .any(|character| matches!(character, 'a'..='f' | 'A'..='F'))
}

fn parse_compact_hex(text: &str) -> Result<Vec<u8>, String> {
    text.as_bytes()
        .chunks(2)
        .map(|chunk| {
            let pair = std::str::from_utf8(chunk).map_err(|error| format!("invalid hex string: {error}"))?;
            u8::from_str_radix(pair, 16).map_err(|error| format!("invalid MIDI byte '{pair}': {error}"))
        })
        .collect()
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
    [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, note)| enum_option(note, note, index as i32))
    .collect()
}

fn system_common_message_options() -> Vec<ParameterEnumOption> {
    [
        (SYSTEM_COMMON_TUNE_REQUEST, "Tune Request"),
        (
            SYSTEM_COMMON_TIME_CODE_QUARTER_FRAME,
            "Time Code Quarter Frame",
        ),
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
