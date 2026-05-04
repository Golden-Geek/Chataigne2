use std::{
    collections::HashSet,
    time::Duration,
};

pub(crate) mod midi_message;
pub(crate) mod midi_runtime;

use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{DeclId, Folder, Node, NodeCreationContext, NodeId, NodeMetaPatch},
    parameter::{
        Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, ParameterEnumOption,
        ParameterEventBehaviour, RangeConstraint,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use self::{
    midi_message::{
        cc_decl_id, cc_label, channel_decl_id, channel_folder_label, clamp_i32_to_u14, clamp_i32_to_u7,
        decode_midi_message, decode_rotary_delta, encode_14_bit_control_change, encode_midi_message,
        encode_rotary_delta, message_description, note_decl_id, note_label, MidiMessage, MidiSystemMessage,
        MIDI_DATA_MAX, MIDI_PITCH_BEND_CENTER, MIDI_U14_MAX, ROTARY_ABSOLUTE, ROTARY_BINARY_OFFSET,
        ROTARY_SIGN_MAGNITUDE, ROTARY_TWOS_COMPLEMENT,
    },
    midi_runtime::{
        available_midi_port_options, format_midi_bytes, midi_input_port_options, midi_output_port_options,
        midi_port_selected, sync_midi_port_enum_options, MidiInputConfig, MidiInputEvent, MidiInputHandle,
        MidiOutputConfig, MidiOutputHandle, NO_MIDI_PORT_VARIANT,
    },
};

use crate::app::MidiSendRequest;

const MIDI_MODULE_UPDATE_RATE_HZ: u32 = 240;
const MIDI_PORT_REFRESH_INTERVAL_SECS: f64 = 1.0;
const MIDI_INPUT_WARNING_ID: &str = "midi_input_connection";
const MIDI_OUTPUT_WARNING_ID: &str = "midi_output_connection";
const MIDI_PORT_OPTIONS_WARNING_ID: &str = "midi_port_options";

const NOTES_FOLDER_DECL_ID: &str = "notes";
const CONTROL_CHANGE_FOLDER_DECL_ID: &str = "control_change";
const POLY_PRESSURE_FOLDER_DECL_ID: &str = "poly_pressure";
const SYSTEM_FOLDER_DECL_ID: &str = "system";
const SYSEX_FOLDER_DECL_ID: &str = "sysex";

const NOTE_ON_DECL_ID: &str = "on";
const NOTE_VELOCITY_DECL_ID: &str = "velocity";
const NOTE_RELEASE_VELOCITY_DECL_ID: &str = "release_velocity";
const NOTE_PITCH_DECL_ID: &str = "pitch";
const NOTE_NAME_DECL_ID: &str = "name";

const CC_VALUE_DECL_ID: &str = "value";
const CC_RAW_VALUE_DECL_ID: &str = "raw_value";
const CC_CONTROLLER_DECL_ID: &str = "controller";
const CC_IS_14_BIT_DECL_ID: &str = "is_14_bit";
const CC_ROTARY_MECHANISM_DECL_ID: &str = "rotary_mechanism";

const PROGRAM_DECL_ID: &str = "program";
const CHANNEL_PRESSURE_DECL_ID: &str = "channel_pressure";
const PITCH_BEND_DECL_ID: &str = "pitch_bend";
const TIME_CODE_QUARTER_FRAME_DECL_ID: &str = "time_code_quarter_frame";
const SONG_POSITION_DECL_ID: &str = "song_position";
const SONG_SELECT_DECL_ID: &str = "song_select";
const TUNE_REQUEST_DECL_ID: &str = "tune_request";
const TIMING_CLOCK_DECL_ID: &str = "timing_clock";
const START_DECL_ID: &str = "start";
const CONTINUE_DECL_ID: &str = "continue";
const STOP_DECL_ID: &str = "stop";
const ACTIVE_SENSING_DECL_ID: &str = "active_sensing";
const RESET_DECL_ID: &str = "reset";
const SYSEX_BYTES_DECL_ID: &str = "bytes";
const SYSEX_LENGTH_DECL_ID: &str = "length";

const MIDI_MODULE_COMMAND_TYPES: &[&str] = &[
    crate::app::MidiSendNoteOnCommand::NODE_TYPE,
    crate::app::MidiSendNoteOffCommand::NODE_TYPE,
    crate::app::MidiSendFullNoteCommand::NODE_TYPE,
    crate::app::MidiSendControlChangeCommand::NODE_TYPE,
    crate::app::MidiSendProgramChangeCommand::NODE_TYPE,
    crate::app::MidiSendPitchBendCommand::NODE_TYPE,
    crate::app::MidiSendChannelPressureCommand::NODE_TYPE,
    crate::app::MidiSendPolyPressureCommand::NODE_TYPE,
    crate::app::MidiSendSystemCommonCommand::NODE_TYPE,
    crate::app::MidiSendSystemRealtimeCommand::NODE_TYPE,
    crate::app::MidiSendSysexBytesCommand::NODE_TYPE,
    crate::app::MidiSendSysexStringCommand::NODE_TYPE,
    crate::app::MidiSendRawBytesCommand::NODE_TYPE,
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Cc14BitState {
    msb: Option<u8>,
    lsb: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingMidiPacket {
    due_at: Duration,
    bytes: Vec<u8>,
    description: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MidiApplyResult {
    Applied,
    Retry,
    Ignored,
}

#[node("midi_note_value", label = "Note")]
#[children(
    on: bool = false (
        label = "On",
        description = "Whether this note is currently held."
    );
    velocity: i32 = 0 [0..127] (
        label = "Velocity",
        description = "Last note-on velocity."
    );
    release_velocity: i32 = 0 [0..127] (
        label = "Release Velocity",
        description = "Last note-off release velocity."
    );
    pitch: i32 = 0 [0..127] (
        label = "Pitch",
        description = "MIDI note pitch.",
        read_only = true
    );
    name: String = String::new() (
        label = "Name",
        description = "Pitch formatted as note name and octave.",
        read_only = true
    );
)]
pub struct MidiNoteValue {}

#[node("midi_note_value", from_struct)]
impl Node for MidiNoteValue {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("midi_control_change_value", label = "Control Change")]
#[children(
    value: i32 = 0 (
        label = "Value",
        description = "Absolute, 14-bit, or accumulated relative control-change value."
    );
    raw_value: i32 = 0 [0..127] (
        label = "Raw Value",
        description = "Last raw 7-bit MIDI control-change value.",
        read_only = true
    );
    controller: i32 = 0 [0..127] (
        label = "Controller",
        description = "MIDI control-change number.",
        read_only = true
    );
    is_14_bit: bool = false (
        label = "14-bit",
        description = "Treat this controller as a 14-bit MSB/LSB MIDI control value."
    );
    rotary_mechanism: Enum = ROTARY_ABSOLUTE (
        label = "Rotary Mechanism",
        description = "Infinite rotary encoder interpretation for incoming and feedback values.",
        enum_options = midi_rotary_mechanism_options()
    );
)]
pub struct MidiControlChangeValue {}

#[node("midi_control_change_value", from_struct)]
impl Node for MidiControlChangeValue {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::new)
    }
}

#[node("midi_module", label = "MIDI")]
#[children(
    folder(connection) {
        input_port: Enum = NO_MIDI_PORT_VARIANT (
            label = "Input Port",
            description = "MIDI input port used to receive notes, controllers, system messages, and SysEx.",
            enum_options = ["none (No Input)"]
        );
        output_port: Enum = NO_MIDI_PORT_VARIANT (
            label = "Output Port",
            description = "MIDI output port used for commands and automatic feedback.",
            enum_options = ["none (No Output)"]
        );
    }
    folder(parameters) {
        folder(processing, label = "Processing") {
            auto_add: bool = true (
                label = "Auto Add",
                description = "Automatically create missing MIDI value nodes from incoming messages."
            );
            auto_feedback: bool = false (
                label = "Auto Feedback",
                description = "Send MIDI when values in the MIDI value tree are edited."
            );
        }
    }
)]
pub struct MidiModule {
    base: crate::app::ModuleBase,
    port_refresh_elapsed: f64,
    input: Option<MidiInputHandle>,
    output: Option<MidiOutputHandle>,
    last_input_config: Option<MidiInputConfig>,
    last_output_config: Option<MidiOutputConfig>,
    input_dirty: bool,
    output_dirty: bool,
    pending_incoming_messages: Vec<MidiMessage>,
    ignored_param_changes: HashSet<NodeId>,
    pending_packets: Vec<PendingMidiPacket>,
    cc_14_bit_state: [[Cc14BitState; 32]; 16],
}

impl MidiModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            MIDI_PORT_REFRESH_INTERVAL_SECS,
            None,
            None,
            None,
            None,
            true,
            true,
            Vec::new(),
            HashSet::new(),
            Vec::new(),
            [[Cc14BitState::default(); 32]; 16],
        )
    }

    fn module_command_types() -> &'static [&'static str] {
        MIDI_MODULE_COMMAND_TYPES
    }

    fn module_enabled(&self, snapshot: &ProcessTreeSnapshot) -> bool {
        snapshot.node(self.id()).map(|node| node.enabled).unwrap_or(false)
    }

    fn port_refresh_due(&self) -> bool {
        self.port_refresh_elapsed >= MIDI_PORT_REFRESH_INTERVAL_SECS
    }

    fn refresh_port_options(&self, ctx: &mut ProcessCtx) {
        match available_midi_port_options() {
            Ok(options) => {
                if self.input_port.is_bound() {
                    sync_midi_port_enum_options(ctx, self.input_port.id(), midi_input_port_options(&options.inputs));
                    self.input_port
                        .clear_warning(ctx, Some(MIDI_PORT_OPTIONS_WARNING_ID));
                }
                if self.output_port.is_bound() {
                    sync_midi_port_enum_options(
                        ctx,
                        self.output_port.id(),
                        midi_output_port_options(&options.outputs),
                    );
                    self.output_port
                        .clear_warning(ctx, Some(MIDI_PORT_OPTIONS_WARNING_ID));
                }
            }
            Err(error) => {
                if self.input_port.is_bound() {
                    self.input_port
                        .set_warning_with(ctx, Some(MIDI_PORT_OPTIONS_WARNING_ID), error.as_str(), None);
                }
                if self.output_port.is_bound() {
                    self.output_port
                        .set_warning_with(ctx, Some(MIDI_PORT_OPTIONS_WARNING_ID), error.as_str(), None);
                }
            }
        }
    }

    fn refresh_input(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.input_dirty = false;

        if !self.module_enabled(snapshot) {
            self.stop_input();
            self.last_input_config = None;
            self.clear_input_warning(ctx);
            self.refresh_connected(ctx);
            return;
        }

        let Some(config) = self.input_config() else {
            self.stop_input();
            self.last_input_config = None;
            self.clear_input_warning(ctx);
            self.refresh_connected(ctx);
            return;
        };

        if self.input.is_some() && self.last_input_config.as_ref() == Some(&config) {
            self.clear_input_warning(ctx);
            self.refresh_connected(ctx);
            return;
        }

        self.stop_input();
        match MidiInputHandle::spawn(config.clone()) {
            Ok(handle) => {
                golden_core::log!(origin = self.id(); "Connected MIDI input.");
                self.input = Some(handle);
                self.last_input_config = Some(config);
                self.clear_input_warning(ctx);
            }
            Err(error) => {
                logerror!("Failed to connect MIDI input: {}", error);
                self.last_input_config = None;
                self.set_input_warning(ctx, error.as_str());
            }
        }
        self.refresh_connected(ctx);
    }

    fn refresh_output(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        self.output_dirty = false;

        if !self.module_enabled(snapshot) {
            self.stop_output();
            self.last_output_config = None;
            self.clear_output_warning(ctx);
            self.refresh_connected(ctx);
            return;
        }

        let Some(config) = self.output_config() else {
            self.stop_output();
            self.last_output_config = None;
            self.clear_output_warning(ctx);
            self.refresh_connected(ctx);
            return;
        };

        if self.output.is_some() && self.last_output_config.as_ref() == Some(&config) {
            self.clear_output_warning(ctx);
            self.refresh_connected(ctx);
            return;
        }

        self.stop_output();
        match MidiOutputHandle::spawn(config.clone()) {
            Ok(handle) => {
                golden_core::log!(origin = self.id(); "Connected MIDI output.");
                self.output = Some(handle);
                self.last_output_config = Some(config);
                self.clear_output_warning(ctx);
            }
            Err(error) => {
                logerror!("Failed to connect MIDI output: {}", error);
                self.last_output_config = None;
                self.set_output_warning(ctx, error.as_str());
            }
        }
        self.refresh_connected(ctx);
    }

    fn input_config(&self) -> Option<MidiInputConfig> {
        midi_port_selected(self.input_port.get_ref().as_str()).then(|| MidiInputConfig {
            port_variant: self.input_port.get_ref().as_str().to_string(),
        })
    }

    fn output_config(&self) -> Option<MidiOutputConfig> {
        midi_port_selected(self.output_port.get_ref().as_str()).then(|| MidiOutputConfig {
            port_variant: self.output_port.get_ref().as_str().to_string(),
        })
    }

    fn refresh_connected(&mut self, ctx: &mut ProcessCtx) {
        self.base
            .set_connected(ctx, self.input.is_some() || self.output.is_some());
    }

    fn refresh_data_capabilities(&mut self, ctx: &mut ProcessCtx) {
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(
                midi_port_selected(self.input_port.get_ref().as_str()),
                midi_port_selected(self.output_port.get_ref().as_str()),
            ),
        );
    }

    fn stop_input(&mut self) {
        if let Some(mut input) = self.input.take() {
            input.stop();
        }
    }

    fn stop_output(&mut self) {
        self.output = None;
    }

    fn drain_input_events(&mut self, ctx: &mut ProcessCtx) {
        let mut events = Vec::new();
        if let Some(input) = &self.input {
            while let Ok(event) = input.try_recv() {
                events.push(event);
            }
        }

        let mut received = false;
        for event in events {
            match event {
                MidiInputEvent::Message(bytes) => match decode_midi_message(bytes.as_slice()) {
                    Some(message) => {
                        received = true;
                        self.log_incoming_message(&message, bytes.as_slice());
                        self.pending_incoming_messages.push(message);
                    }
                    None => {
                        logerror!("Ignored unsupported MIDI message {}", format_midi_bytes(bytes.as_slice()));
                    }
                },
            }
        }

        if received {
            self.base.emit_incoming_traffic(ctx);
        }
    }

    fn process_pending_incoming(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) -> bool {
        if self.pending_incoming_messages.is_empty() {
            return false;
        }

        let mut remaining = Vec::new();
        let mut messages = std::mem::take(&mut self.pending_incoming_messages).into_iter();
        while let Some(message) = messages.next() {
            match self.apply_incoming_message(ctx, snapshot, &message) {
                MidiApplyResult::Applied | MidiApplyResult::Ignored => {}
                MidiApplyResult::Retry => {
                    remaining.push(message);
                    remaining.extend(messages);
                    self.pending_incoming_messages = remaining;
                    return true;
                }
            }
        }

        self.pending_incoming_messages = remaining;
        false
    }

    fn apply_incoming_message(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        message: &MidiMessage,
    ) -> MidiApplyResult {
        match message {
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } if *velocity == 0 => self.apply_note_off(ctx, snapshot, *channel, *note, 0),
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } => self.apply_note_on(ctx, snapshot, *channel, *note, *velocity),
            MidiMessage::NoteOff {
                channel,
                note,
                velocity,
            } => self.apply_note_off(ctx, snapshot, *channel, *note, *velocity),
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => self.apply_control_change(ctx, snapshot, *channel, *controller, *value),
            MidiMessage::PolyPressure {
                channel,
                note,
                pressure,
            } => self.apply_poly_pressure(ctx, snapshot, *channel, *note, *pressure),
            MidiMessage::ProgramChange { channel, program } => {
                self.apply_channel_parameter(ctx, snapshot, *channel, PROGRAM_DECL_ID, "Program", i32::from(*program), Some((0, 127)))
            }
            MidiMessage::ChannelPressure { channel, pressure } => self.apply_channel_parameter(
                ctx,
                snapshot,
                *channel,
                CHANNEL_PRESSURE_DECL_ID,
                "Channel Pressure",
                i32::from(*pressure),
                Some((0, 127)),
            ),
            MidiMessage::PitchBend { channel, value } => self.apply_channel_parameter(
                ctx,
                snapshot,
                *channel,
                PITCH_BEND_DECL_ID,
                "Pitch Bend",
                i32::from(*value),
                Some((0, i32::from(MIDI_U14_MAX))),
            ),
            MidiMessage::System(system) => self.apply_system_message(ctx, snapshot, system),
        }
    }

    fn apply_note_on(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        note: u8,
        velocity: u8,
    ) -> MidiApplyResult {
        let Some(note_id) = self.ensure_note_value(ctx, snapshot, channel, note) else {
            return MidiApplyResult::Retry;
        };
        let Some(on_id) = child_by_decl_id(snapshot, note_id, NOTE_ON_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        let Some(velocity_id) = child_by_decl_id(snapshot, note_id, NOTE_VELOCITY_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        let Some(pitch_id) = child_by_decl_id(snapshot, note_id, NOTE_PITCH_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        let Some(name_id) = child_by_decl_id(snapshot, note_id, NOTE_NAME_DECL_ID) else {
            return MidiApplyResult::Retry;
        };

        self.set_internal_param(ctx, pitch_id, ParamValue::Int(i32::from(note)));
        self.set_internal_param(ctx, name_id, ParamValue::Str(note_label(note)));
        self.set_internal_param(ctx, velocity_id, ParamValue::Int(i32::from(velocity)));
        self.set_internal_param(ctx, on_id, ParamValue::Bool(true));
        MidiApplyResult::Applied
    }

    fn apply_note_off(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        note: u8,
        velocity: u8,
    ) -> MidiApplyResult {
        let Some(note_id) = self.ensure_note_value(ctx, snapshot, channel, note) else {
            return MidiApplyResult::Retry;
        };
        let Some(on_id) = child_by_decl_id(snapshot, note_id, NOTE_ON_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        let Some(release_velocity_id) = child_by_decl_id(snapshot, note_id, NOTE_RELEASE_VELOCITY_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        let Some(pitch_id) = child_by_decl_id(snapshot, note_id, NOTE_PITCH_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        let Some(name_id) = child_by_decl_id(snapshot, note_id, NOTE_NAME_DECL_ID) else {
            return MidiApplyResult::Retry;
        };

        self.set_internal_param(ctx, pitch_id, ParamValue::Int(i32::from(note)));
        self.set_internal_param(ctx, name_id, ParamValue::Str(note_label(note)));
        self.set_internal_param(ctx, release_velocity_id, ParamValue::Int(i32::from(velocity)));
        self.set_internal_param(ctx, on_id, ParamValue::Bool(false));
        MidiApplyResult::Applied
    }

    fn apply_control_change(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        controller: u8,
        raw_value: u8,
    ) -> MidiApplyResult {
        if (32..=63).contains(&controller) {
            let base_controller = controller - 32;
            if self.cc_value_is_14_bit(snapshot, channel, base_controller) {
                return self.apply_control_change_value(ctx, snapshot, channel, base_controller, raw_value, true);
            }
        }

        self.apply_control_change_value(ctx, snapshot, channel, controller, raw_value, false)
    }

    fn apply_control_change_value(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        controller: u8,
        raw_value: u8,
        is_lsb_message: bool,
    ) -> MidiApplyResult {
        let Some(cc_id) = self.ensure_cc_value(ctx, snapshot, channel, controller) else {
            return MidiApplyResult::Retry;
        };
        let Some(value_id) = child_by_decl_id(snapshot, cc_id, CC_VALUE_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        let Some(raw_value_id) = child_by_decl_id(snapshot, cc_id, CC_RAW_VALUE_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        let Some(controller_id) = child_by_decl_id(snapshot, cc_id, CC_CONTROLLER_DECL_ID) else {
            return MidiApplyResult::Retry;
        };

        let is_14_bit = child_bool(snapshot, cc_id, CC_IS_14_BIT_DECL_ID).unwrap_or(false);
        let mechanism = child_enum(snapshot, cc_id, CC_ROTARY_MECHANISM_DECL_ID).unwrap_or_else(|| ROTARY_ABSOLUTE.to_string());

        self.set_internal_param(ctx, controller_id, ParamValue::Int(i32::from(controller)));
        self.set_internal_param(ctx, raw_value_id, ParamValue::Int(i32::from(raw_value)));

        if mechanism != ROTARY_ABSOLUTE {
            let current = child_int(snapshot, cc_id, CC_VALUE_DECL_ID).unwrap_or(0);
            let delta = decode_rotary_delta(mechanism.as_str(), raw_value).unwrap_or(0);
            self.set_internal_param(ctx, value_id, ParamValue::Int(current.saturating_add(delta)));
            return MidiApplyResult::Applied;
        }

        if is_14_bit && controller <= 31 {
            let channel_index = usize::from(channel.saturating_sub(1).min(15));
            let controller_index = usize::from(controller);
            let state = &mut self.cc_14_bit_state[channel_index][controller_index];
            if is_lsb_message {
                state.lsb = Some(raw_value);
            } else {
                state.msb = Some(raw_value);
            }

            let msb = state.msb.unwrap_or(raw_value);
            let lsb = state.lsb.unwrap_or(0);
            let combined = (u16::from(msb) << 7) | u16::from(lsb);
            self.set_internal_param(ctx, value_id, ParamValue::Int(i32::from(combined)));
            return MidiApplyResult::Applied;
        }

        self.set_internal_param(ctx, value_id, ParamValue::Int(i32::from(raw_value)));
        MidiApplyResult::Applied
    }

    fn apply_poly_pressure(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        note: u8,
        pressure: u8,
    ) -> MidiApplyResult {
        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };
        let Some(pressure_folder_id) = self.ensure_folder(
            ctx,
            snapshot,
            channel_id,
            "Poly Pressure",
            POLY_PRESSURE_FOLDER_DECL_ID,
        ) else {
            return MidiApplyResult::Retry;
        };

        self.apply_direct_int_parameter(
            ctx,
            snapshot,
            pressure_folder_id,
            note_label(note).as_str(),
            note_decl_id(note).as_str(),
            i32::from(pressure),
            Some((0, 127)),
        )
    }

    fn apply_channel_parameter(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        decl_id: &str,
        label: &str,
        value: i32,
        range: Option<(i32, i32)>,
    ) -> MidiApplyResult {
        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };

        self.apply_direct_int_parameter(ctx, snapshot, channel_id, label, decl_id, value, range)
    }

    fn apply_system_message(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        message: &MidiSystemMessage,
    ) -> MidiApplyResult {
        let Some(values_id) = self.base.values_id() else {
            return MidiApplyResult::Ignored;
        };
        let Some(system_id) = self.ensure_folder(ctx, snapshot, values_id, "System", SYSTEM_FOLDER_DECL_ID) else {
            return MidiApplyResult::Retry;
        };

        match message {
            MidiSystemMessage::TimeCodeQuarterFrame { value } => self.apply_direct_int_parameter(
                ctx,
                snapshot,
                system_id,
                "Time Code Quarter Frame",
                TIME_CODE_QUARTER_FRAME_DECL_ID,
                i32::from(*value),
                Some((0, 127)),
            ),
            MidiSystemMessage::SongPosition { position } => self.apply_direct_int_parameter(
                ctx,
                snapshot,
                system_id,
                "Song Position",
                SONG_POSITION_DECL_ID,
                i32::from(*position),
                Some((0, i32::from(MIDI_U14_MAX))),
            ),
            MidiSystemMessage::SongSelect { song } => self.apply_direct_int_parameter(
                ctx,
                snapshot,
                system_id,
                "Song Select",
                SONG_SELECT_DECL_ID,
                i32::from(*song),
                Some((0, 127)),
            ),
            MidiSystemMessage::TuneRequest => self.apply_trigger(ctx, snapshot, system_id, "Tune Request", TUNE_REQUEST_DECL_ID),
            MidiSystemMessage::TimingClock => self.apply_trigger(ctx, snapshot, system_id, "Timing Clock", TIMING_CLOCK_DECL_ID),
            MidiSystemMessage::Start => self.apply_trigger(ctx, snapshot, system_id, "Start", START_DECL_ID),
            MidiSystemMessage::Continue => self.apply_trigger(ctx, snapshot, system_id, "Continue", CONTINUE_DECL_ID),
            MidiSystemMessage::Stop => self.apply_trigger(ctx, snapshot, system_id, "Stop", STOP_DECL_ID),
            MidiSystemMessage::ActiveSensing => {
                self.apply_trigger(ctx, snapshot, system_id, "Active Sensing", ACTIVE_SENSING_DECL_ID)
            }
            MidiSystemMessage::Reset => self.apply_trigger(ctx, snapshot, system_id, "Reset", RESET_DECL_ID),
            MidiSystemMessage::Sysex { bytes } => self.apply_sysex(ctx, snapshot, system_id, bytes.as_slice()),
        }
    }

    fn apply_sysex(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        system_id: NodeId,
        bytes: &[u8],
    ) -> MidiApplyResult {
        let Some(sysex_id) = self.ensure_folder(ctx, snapshot, system_id, "SysEx", SYSEX_FOLDER_DECL_ID) else {
            return MidiApplyResult::Retry;
        };

        let bytes_result = self.apply_direct_string_parameter(
            ctx,
            snapshot,
            sysex_id,
            "Bytes",
            SYSEX_BYTES_DECL_ID,
            format_midi_bytes(bytes),
            true,
        );
        if bytes_result == MidiApplyResult::Retry {
            return bytes_result;
        }

        self.apply_direct_int_parameter(
            ctx,
            snapshot,
            sysex_id,
            "Length",
            SYSEX_LENGTH_DECL_ID,
            i32::try_from(bytes.len()).unwrap_or(i32::MAX),
            Some((0, i32::MAX)),
        )
    }

    fn apply_trigger(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent_id: NodeId,
        label: &str,
        decl_id: &str,
    ) -> MidiApplyResult {
        match snapshot.find_child_by_decl_id(parent_id, decl_id) {
            Some(existing_id) if snapshot.node(existing_id).is_some_and(|node| node.node_type == "trigger") => {
                self.set_internal_param(ctx, existing_id, ParamValue::Trigger());
                MidiApplyResult::Applied
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                ctx.replace_node_boxed(existing_id, Box::new(create_trigger_parameter(label, decl_id)));
                MidiApplyResult::Retry
            }
            None => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                ctx.add_child_boxed(parent_id, Box::new(create_trigger_parameter(label, decl_id)), None);
                MidiApplyResult::Retry
            }
        }
    }

    fn apply_direct_int_parameter(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent_id: NodeId,
        label: &str,
        decl_id: &str,
        value: i32,
        range: Option<(i32, i32)>,
    ) -> MidiApplyResult {
        match snapshot.find_child_by_decl_id(parent_id, decl_id) {
            Some(existing_id) if snapshot.node(existing_id).is_some_and(|node| node.node_type == "int") => {
                self.set_internal_param(ctx, existing_id, ParamValue::Int(value));
                MidiApplyResult::Applied
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                ctx.replace_node_boxed(
                    existing_id,
                    Box::new(create_int_parameter(label, decl_id, value, range, false)),
                );
                MidiApplyResult::Retry
            }
            None => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                ctx.add_child_boxed(
                    parent_id,
                    Box::new(create_int_parameter(label, decl_id, value, range, false)),
                    None,
                );
                MidiApplyResult::Retry
            }
        }
    }

    fn apply_direct_string_parameter(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent_id: NodeId,
        label: &str,
        decl_id: &str,
        value: String,
        read_only: bool,
    ) -> MidiApplyResult {
        match snapshot.find_child_by_decl_id(parent_id, decl_id) {
            Some(existing_id) if snapshot.node(existing_id).is_some_and(|node| node.node_type == "str") => {
                self.set_internal_param(ctx, existing_id, ParamValue::Str(value));
                MidiApplyResult::Applied
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                ctx.replace_node_boxed(
                    existing_id,
                    Box::new(create_string_parameter(label, decl_id, value, read_only)),
                );
                MidiApplyResult::Retry
            }
            None => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                ctx.add_child_boxed(
                    parent_id,
                    Box::new(create_string_parameter(label, decl_id, value, read_only)),
                    None,
                );
                MidiApplyResult::Retry
            }
        }
    }

    fn ensure_channel_folder(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
    ) -> Option<NodeId> {
        let values_id = self.base.values_id()?;
        self.ensure_folder(
            ctx,
            snapshot,
            values_id,
            channel_folder_label(channel).as_str(),
            channel_decl_id(channel).as_str(),
        )
    }

    fn ensure_note_value(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        note: u8,
    ) -> Option<NodeId> {
        let channel_id = self.ensure_channel_folder(ctx, snapshot, channel)?;
        let notes_id = self.ensure_folder(ctx, snapshot, channel_id, "Notes", NOTES_FOLDER_DECL_ID)?;
        let decl_id = note_decl_id(note);
        match snapshot.find_child_by_decl_id(notes_id, decl_id.as_str()) {
            Some(existing_id)
                if snapshot
                    .node(existing_id)
                    .is_some_and(|node| node.node_type == MidiNoteValue::NODE_TYPE) =>
            {
                Some(existing_id)
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return None;
                }
                ctx.replace_node_boxed(existing_id, Box::new(create_note_value(note)));
                None
            }
            None => {
                if !self.auto_add.get() {
                    return None;
                }
                ctx.add_child_boxed(notes_id, Box::new(create_note_value(note)), None);
                None
            }
        }
    }

    fn ensure_cc_value(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        controller: u8,
    ) -> Option<NodeId> {
        let channel_id = self.ensure_channel_folder(ctx, snapshot, channel)?;
        let cc_folder_id = self.ensure_folder(
            ctx,
            snapshot,
            channel_id,
            "Control Change",
            CONTROL_CHANGE_FOLDER_DECL_ID,
        )?;
        let decl_id = cc_decl_id(controller);
        match snapshot.find_child_by_decl_id(cc_folder_id, decl_id.as_str()) {
            Some(existing_id)
                if snapshot
                    .node(existing_id)
                    .is_some_and(|node| node.node_type == MidiControlChangeValue::NODE_TYPE) =>
            {
                Some(existing_id)
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return None;
                }
                ctx.replace_node_boxed(existing_id, Box::new(create_cc_value(controller)));
                None
            }
            None => {
                if !self.auto_add.get() {
                    return None;
                }
                ctx.add_child_boxed(cc_folder_id, Box::new(create_cc_value(controller)), None);
                None
            }
        }
    }

    fn ensure_folder(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent_id: NodeId,
        label: &str,
        decl_id: &str,
    ) -> Option<NodeId> {
        match snapshot.find_child_by_decl_id(parent_id, decl_id) {
            Some(existing_id) if snapshot.node(existing_id).is_some_and(|node| node.node_type == "folder") => {
                Some(existing_id)
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return None;
                }
                ctx.replace_node_boxed(existing_id, Box::new(create_folder(label, decl_id)));
                None
            }
            None => {
                if !self.auto_add.get() {
                    return None;
                }
                ctx.add_child_boxed(parent_id, Box::new(create_folder(label, decl_id)), None);
                None
            }
        }
    }

    fn cc_value_is_14_bit(&self, snapshot: &ProcessTreeSnapshot, channel: u8, controller: u8) -> bool {
        self.find_cc_value(snapshot, channel, controller)
            .and_then(|cc_id| child_bool(snapshot, cc_id, CC_IS_14_BIT_DECL_ID))
            .unwrap_or(false)
    }

    fn find_cc_value(&self, snapshot: &ProcessTreeSnapshot, channel: u8, controller: u8) -> Option<NodeId> {
        let values_id = self.base.values_id()?;
        let channel_id = snapshot.find_child_by_decl_id(values_id, channel_decl_id(channel).as_str())?;
        let cc_folder_id = snapshot.find_child_by_decl_id(channel_id, CONTROL_CHANGE_FOLDER_DECL_ID)?;
        snapshot.find_child_by_decl_id(cc_folder_id, cc_decl_id(controller).as_str())
    }

    fn set_internal_param(&mut self, ctx: &mut ProcessCtx, param_id: NodeId, value: ParamValue) {
        self.ignored_param_changes.insert(param_id);
        ctx.set_param_with_behaviour(param_id, value, ParameterEventBehaviour::Append);
    }

    fn handle_feedback_param_change(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        param: NodeId,
        old_value: ParamValue,
    ) {
        if !self.auto_feedback.get() {
            return;
        }
        let Some(values_id) = self.base.values_id() else {
            return;
        };
        if !is_descendant_of(snapshot, param, values_id) {
            return;
        }

        let Some(messages) = feedback_messages_for_param(snapshot, param, old_value) else {
            return;
        };
        if messages.is_empty() {
            return;
        }

        for message in messages {
            let bytes = encode_midi_message(&message);
            if let Err(error) = self.send_bytes(ctx, bytes.as_slice(), message_description(&message).as_str()) {
                logerror!("Failed to send MIDI auto feedback: {}", error);
            }
        }
    }

    fn queue_send_request(&mut self, ctx: &mut ProcessCtx, request: MidiSendRequest) -> Result<String, String> {
        let mut queued = 0usize;
        for packet in request.packets {
            if packet.delay_ms == 0 {
                self.send_bytes(ctx, packet.bytes.as_slice(), packet.description.as_str())?;
                queued = queued.saturating_add(1);
                continue;
            }

            self.pending_packets.push(PendingMidiPacket {
                due_at: ctx.runtime_elapsed + Duration::from_millis(packet.delay_ms),
                bytes: packet.bytes,
                description: packet.description,
            });
            queued = queued.saturating_add(1);
        }

        Ok(format!("Queued {} MIDI packet(s)", queued))
    }

    fn flush_pending_packets(&mut self, ctx: &mut ProcessCtx) {
        if self.pending_packets.is_empty() {
            return;
        }

        let now = ctx.runtime_elapsed;
        let packets = std::mem::take(&mut self.pending_packets);
        for packet in packets {
            if packet.due_at > now {
                self.pending_packets.push(packet);
                continue;
            }

            if let Err(error) = self.send_bytes(ctx, packet.bytes.as_slice(), packet.description.as_str()) {
                logerror!("Failed to send delayed MIDI packet: {}", error);
            }
        }
    }

    fn send_bytes(&mut self, ctx: &mut ProcessCtx, bytes: &[u8], description: &str) -> Result<(), String> {
        let output = self
            .output
            .as_mut()
            .ok_or_else(|| "MIDI output is not connected".to_string())?;
        output.send(bytes)?;
        self.base.emit_outgoing_traffic(ctx);
        self.log_outgoing_bytes(bytes, description);
        Ok(())
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !Self::module_command_types().contains(&request.command_type.as_str()) {
            return;
        }

        if let Err(error) = serde_json::from_value::<MidiSendRequest>(request.payload)
            .map_err(|error| format!("invalid MIDI command payload: {error}"))
            .and_then(|payload| self.queue_send_request(ctx, payload))
        {
            logerror!(format!(
                "Failed to handle MIDI command {:?}: {error}",
                request.command_id
            ));
        }
    }

    fn on_param_change_inner(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        param: NodeId,
        old_value: ParamValue,
    ) {
        if self.ignored_param_changes.remove(&param) {
            return;
        }

        if self.param_affects_transport(param) {
            if self.input_port.is_bound() && self.input_port.id() == param {
                self.input_dirty = true;
            }
            if self.output_port.is_bound() && self.output_port.id() == param {
                self.output_dirty = true;
            }
            self.refresh_data_capabilities(ctx);
            return;
        }

        self.handle_feedback_param_change(ctx, snapshot, param, old_value);
    }

    fn param_affects_transport(&self, param: NodeId) -> bool {
        (self.input_port.is_bound() && self.input_port.id() == param)
            || (self.output_port.is_bound() && self.output_port.id() == param)
    }

    fn set_input_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if self.input_port.is_bound() {
            self.input_port
                .set_warning_with(ctx, Some(MIDI_INPUT_WARNING_ID), message, None);
        }
    }

    fn clear_input_warning(&self, ctx: &mut ProcessCtx) {
        if self.input_port.is_bound() {
            self.input_port.clear_warning(ctx, Some(MIDI_INPUT_WARNING_ID));
        }
    }

    fn set_output_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        if self.output_port.is_bound() {
            self.output_port
                .set_warning_with(ctx, Some(MIDI_OUTPUT_WARNING_ID), message, None);
        }
    }

    fn clear_output_warning(&self, ctx: &mut ProcessCtx) {
        if self.output_port.is_bound() {
            self.output_port.clear_warning(ctx, Some(MIDI_OUTPUT_WARNING_ID));
        }
    }

    fn log_incoming_message(&self, message: &MidiMessage, bytes: &[u8]) {
        if !self.base.log_incoming_enabled() {
            return;
        }
        golden_core::log!(
            origin = self.id();
            format!(
                "Received MIDI {} ({})",
                message_description(message),
                format_midi_bytes(bytes)
            )
        );
    }

    fn log_outgoing_bytes(&self, bytes: &[u8], description: &str) {
        if !self.base.log_outgoing_enabled() {
            return;
        }
        golden_core::log!(
            origin = self.id();
            format!("Sent MIDI {} ({})", description, format_midi_bytes(bytes))
        );
    }
}

#[golden_core::item(
    "module",
    node = "midi_module",
    via = base,
    from_struct,
    menu_path = ["Hardware"]
)]
impl Node for MidiModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.ensure_command_tester(ctx, Self::module_command_types());
        crate::app::module::enable_module_authoring(self.node_data_mut());
        self.input_dirty = true;
        self.output_dirty = true;
        self.refresh_port_options(ctx);
        self.port_refresh_elapsed = 0.0;
        self.refresh_data_capabilities(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.refresh_input(ctx, snapshot);
        self.refresh_output(ctx, snapshot);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.drain_input_events(ctx);
        self.port_refresh_elapsed += ctx.delta_time.as_secs_f64();

        let refresh_ports = self.port_refresh_due();
        let needs_work = refresh_ports
            || self.input_dirty
            || self.output_dirty
            || !self.pending_incoming_messages.is_empty()
            || !self.pending_packets.is_empty();
        if !needs_work {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        if refresh_ports {
            self.refresh_port_options(ctx);
            self.port_refresh_elapsed = 0.0;
        }

        self.refresh_data_capabilities(ctx);

        if self.input_dirty {
            self.refresh_input(ctx, snapshot);
        }
        if self.output_dirty {
            self.refresh_output(ctx, snapshot);
        }

        self.process_pending_incoming(ctx, snapshot);
        self.flush_pending_packets(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_input();
        self.stop_output();
        self.pending_packets.clear();
        self.pending_incoming_messages.clear();
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        true
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(MIDI_MODULE_UPDATE_RATE_HZ)
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        self.on_param_change_inner(ctx, snapshot_arc.as_ref(), param, old_value);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if let Some(enabled) = patch.enabled {
            if node == self.id() {
                if enabled {
                    self.input_dirty = true;
                    self.output_dirty = true;
                } else {
                    self.stop_input();
                    self.stop_output();
                    self.last_input_config = None;
                    self.last_output_config = None;
                    self.clear_input_warning(ctx);
                    self.clear_output_warning(ctx);
                    self.base.set_connected(ctx, false);
                    self.input_dirty = false;
                    self.output_dirty = false;
                }
                return;
            }

            self.input_dirty = true;
            self.output_dirty = true;
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

fn feedback_messages_for_param(
    snapshot: &ProcessTreeSnapshot,
    param: NodeId,
    old_value: ParamValue,
) -> Option<Vec<MidiMessage>> {
    let param_snapshot = snapshot.node(param)?;
    let parent_id = param_snapshot.parent?;
    let parent = snapshot.node(parent_id)?;

    if parent.node_type == MidiNoteValue::NODE_TYPE {
        return feedback_messages_for_note_param(snapshot, parent_id, param_snapshot, old_value);
    }

    if parent.node_type == MidiControlChangeValue::NODE_TYPE {
        return feedback_messages_for_cc_param(snapshot, parent_id, param_snapshot, old_value);
    }

    if parent.decl_id.as_str() == POLY_PRESSURE_FOLDER_DECL_ID {
        let channel = channel_for_descendant(snapshot, parent_id)?;
        let note = note_from_decl_id(param_snapshot.decl_id.as_str())?;
        let pressure = param_snapshot.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?;
        return Some(vec![MidiMessage::PolyPressure {
            channel,
            note,
            pressure,
        }]);
    }

    let channel = channel_for_descendant(snapshot, param)?;
    match param_snapshot.decl_id.as_str() {
        PROGRAM_DECL_ID => Some(vec![MidiMessage::ProgramChange {
            channel,
            program: param_snapshot.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?,
        }]),
        CHANNEL_PRESSURE_DECL_ID => Some(vec![MidiMessage::ChannelPressure {
            channel,
            pressure: param_snapshot.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?,
        }]),
        PITCH_BEND_DECL_ID => Some(vec![MidiMessage::PitchBend {
            channel,
            value: param_snapshot
                .param_value
                .as_ref()?
                .as_int()
                .map(clamp_i32_to_u14)
                .unwrap_or(MIDI_PITCH_BEND_CENTER),
        }]),
        _ => feedback_messages_for_system_param(param_snapshot),
    }
}

fn feedback_messages_for_note_param(
    snapshot: &ProcessTreeSnapshot,
    note_id: NodeId,
    param: &golden_core::process_ctx::ProcessTreeNodeSnapshot,
    _old_value: ParamValue,
) -> Option<Vec<MidiMessage>> {
    let channel = channel_for_descendant(snapshot, note_id)?;
    let note = child_int(snapshot, note_id, NOTE_PITCH_DECL_ID).map(clamp_i32_to_u7)?;
    let on = child_bool(snapshot, note_id, NOTE_ON_DECL_ID).unwrap_or(false);
    let velocity = child_int(snapshot, note_id, NOTE_VELOCITY_DECL_ID)
        .map(clamp_i32_to_u7)
        .unwrap_or(0);
    let release_velocity = child_int(snapshot, note_id, NOTE_RELEASE_VELOCITY_DECL_ID)
        .map(clamp_i32_to_u7)
        .unwrap_or(0);

    match param.decl_id.as_str() {
        NOTE_ON_DECL_ID => Some(vec![if on {
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            }
        } else {
            MidiMessage::NoteOff {
                channel,
                note,
                velocity: release_velocity,
            }
        }]),
        NOTE_VELOCITY_DECL_ID if on => Some(vec![MidiMessage::NoteOn {
            channel,
            note,
            velocity,
        }]),
        NOTE_RELEASE_VELOCITY_DECL_ID if !on => Some(vec![MidiMessage::NoteOff {
            channel,
            note,
            velocity: release_velocity,
        }]),
        _ => None,
    }
}

fn feedback_messages_for_cc_param(
    snapshot: &ProcessTreeSnapshot,
    cc_id: NodeId,
    param: &golden_core::process_ctx::ProcessTreeNodeSnapshot,
    old_value: ParamValue,
) -> Option<Vec<MidiMessage>> {
    if param.decl_id != CC_VALUE_DECL_ID {
        return None;
    }

    let channel = channel_for_descendant(snapshot, cc_id)?;
    let controller = child_int(snapshot, cc_id, CC_CONTROLLER_DECL_ID).map(clamp_i32_to_u7)?;
    let value = param.param_value.as_ref()?.as_int().unwrap_or(0);
    let mechanism = child_enum(snapshot, cc_id, CC_ROTARY_MECHANISM_DECL_ID).unwrap_or_else(|| ROTARY_ABSOLUTE.to_string());

    if mechanism != ROTARY_ABSOLUTE {
        let old = old_value.as_int().unwrap_or(value);
        let delta = value.saturating_sub(old);
        let raw_delta = encode_rotary_delta(mechanism.as_str(), delta)?;
        return Some(vec![MidiMessage::ControlChange {
            channel,
            controller,
            value: raw_delta,
        }]);
    }

    if child_bool(snapshot, cc_id, CC_IS_14_BIT_DECL_ID).unwrap_or(false) {
        return Some(encode_14_bit_control_change(
            channel,
            controller.min(31),
            clamp_i32_to_u14(value),
        ));
    }

    Some(vec![MidiMessage::ControlChange {
        channel,
        controller,
        value: clamp_i32_to_u7(value),
    }])
}

fn feedback_messages_for_system_param(
    param: &golden_core::process_ctx::ProcessTreeNodeSnapshot,
) -> Option<Vec<MidiMessage>> {
    let system = match param.decl_id.as_str() {
        TIME_CODE_QUARTER_FRAME_DECL_ID => MidiSystemMessage::TimeCodeQuarterFrame {
            value: param.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?,
        },
        SONG_POSITION_DECL_ID => MidiSystemMessage::SongPosition {
            position: param.param_value.as_ref()?.as_int().map(clamp_i32_to_u14)?,
        },
        SONG_SELECT_DECL_ID => MidiSystemMessage::SongSelect {
            song: param.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?,
        },
        TUNE_REQUEST_DECL_ID => MidiSystemMessage::TuneRequest,
        TIMING_CLOCK_DECL_ID => MidiSystemMessage::TimingClock,
        START_DECL_ID => MidiSystemMessage::Start,
        CONTINUE_DECL_ID => MidiSystemMessage::Continue,
        STOP_DECL_ID => MidiSystemMessage::Stop,
        ACTIVE_SENSING_DECL_ID => MidiSystemMessage::ActiveSensing,
        RESET_DECL_ID => MidiSystemMessage::Reset,
        _ => return None,
    };
    Some(vec![MidiMessage::System(system)])
}

fn create_note_value(note: u8) -> MidiNoteValue {
    let mut value = MidiNoteValue::new();
    let meta = &mut value.node_data_mut().meta;
    meta.label = note_label(note);
    meta.decl_id = DeclId(note_decl_id(note));
    meta.short_name = meta.decl_id.0.clone();
    meta.description = Some(format!("Auto-created MIDI note {}", meta.label));
    value
}

fn create_cc_value(controller: u8) -> MidiControlChangeValue {
    let mut value = MidiControlChangeValue::new();
    let meta = &mut value.node_data_mut().meta;
    meta.label = cc_label(controller);
    meta.decl_id = DeclId(cc_decl_id(controller));
    meta.short_name = meta.decl_id.0.clone();
    meta.description = Some(format!("Auto-created MIDI control change {}", controller));
    value
}

fn create_folder(label: &str, decl_id: &str) -> Folder {
    let mut folder = Folder::new(label);
    apply_node_identity(folder.node_data_mut(), decl_id);
    crate::app::module::enable_module_authoring(folder.node_data_mut());
    folder
}

fn create_int_parameter(
    label: &str,
    decl_id: &str,
    value: i32,
    range: Option<(i32, i32)>,
    read_only: bool,
) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Int(value), ParameterChangeCheck::ValueChange);
    apply_node_identity(parameter.node_data_mut(), decl_id);
    parameter.read_only = read_only;
    if let Some((min, max)) = range {
        parameter.constraints = ParameterConstraints {
            range: RangeConstraint::uniform(Some(f64::from(min)), Some(f64::from(max))),
            ..Default::default()
        };
    }
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter
}

fn create_string_parameter(label: &str, decl_id: &str, value: String, read_only: bool) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Str(value), ParameterChangeCheck::ValueChange);
    apply_node_identity(parameter.node_data_mut(), decl_id);
    parameter.read_only = read_only;
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter
}

fn create_trigger_parameter(label: &str, decl_id: &str) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Trigger(), ParameterChangeCheck::ValueChange);
    apply_node_identity(parameter.node_data_mut(), decl_id);
    crate::app::module::enable_module_authoring(parameter.node_data_mut());
    parameter
}

fn apply_node_identity(node_data: &mut golden_core::node::NodeData, decl_id: &str) {
    node_data.meta.decl_id = DeclId(decl_id.to_string());
    node_data.meta.short_name = decl_id.to_string();
}

fn child_by_decl_id(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<NodeId> {
    snapshot.find_child_by_decl_id(parent, decl_id)
}

fn child_int(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<i32> {
    child_by_decl_id(snapshot, parent, decl_id).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_int)
    })
}

fn child_bool(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<bool> {
    child_by_decl_id(snapshot, parent, decl_id).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_bool)
    })
}

fn child_enum(snapshot: &ProcessTreeSnapshot, parent: NodeId, decl_id: &str) -> Option<String> {
    child_by_decl_id(snapshot, parent, decl_id).and_then(|child_id| {
        snapshot
            .node(child_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_enum)
    })
}

fn channel_for_descendant(snapshot: &ProcessTreeSnapshot, node_id: NodeId) -> Option<u8> {
    let mut current = Some(node_id);
    while let Some(current_id) = current {
        let node = snapshot.node(current_id)?;
        if let Some(channel) = node.decl_id.strip_prefix("channel_") {
            return channel.parse::<u8>().ok().filter(|value| (1..=16).contains(value));
        }
        current = node.parent;
    }
    None
}

fn note_from_decl_id(decl_id: &str) -> Option<u8> {
    decl_id
        .strip_prefix("note_")?
        .parse::<u8>()
        .ok()
        .filter(|value| *value <= MIDI_DATA_MAX)
}

fn is_descendant_of(snapshot: &ProcessTreeSnapshot, start: NodeId, ancestor: NodeId) -> bool {
    let mut current = Some(start);
    while let Some(node_id) = current {
        if node_id == ancestor {
            return true;
        }
        current = snapshot.node(node_id).and_then(|node| node.parent);
    }
    false
}

fn midi_rotary_mechanism_options() -> Vec<ParameterEnumOption> {
    [
        (ROTARY_ABSOLUTE, "Absolute"),
        (ROTARY_TWOS_COMPLEMENT, "Two's Complement"),
        (ROTARY_BINARY_OFFSET, "Binary Offset"),
        (ROTARY_SIGN_MAGNITUDE, "Sign Magnitude"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (variant_id, label))| ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(index as i32),
    })
    .collect()
}
