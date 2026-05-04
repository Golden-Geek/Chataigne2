use std::{collections::HashSet, time::Duration};

pub(crate) mod midi_message;
pub(crate) mod midi_runtime;

use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{DeclId, Folder, Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeUserPermissions},
    parameter::{
        Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, ParameterEventBehaviour,
        RangeConstraint,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use self::{
    midi_message::{
        MIDI_DATA_MAX, MIDI_PITCH_BEND_CENTER, MIDI_U14_MAX, MidiMessage, MidiSystemMessage, ROTARY_ABSOLUTE,
        ROTARY_BINARY_OFFSET, ROTARY_SIGN_MAGNITUDE, ROTARY_TWOS_COMPLEMENT, cc_decl_id, cc_label, cc_supports_14_bit,
        channel_decl_id, channel_folder_label, clamp_i32_to_u7, clamp_i32_to_u14, decode_midi_message,
        decode_rotary_delta, encode_14_bit_control_change, encode_midi_message, encode_rotary_delta,
        message_description, note_decl_id, note_label,
    },
    midi_runtime::{
        MidiInputConfig, MidiInputEvent, MidiInputHandle, MidiOutputConfig, MidiOutputHandle, NO_MIDI_PORT_VARIANT,
        available_midi_port_options, format_midi_bytes, midi_input_port_options, midi_output_port_options,
        midi_port_selected, sync_midi_port_enum_options,
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
const FOURTEEN_BIT_CHANNELS_FOLDER_DECL_ID: &str = "fourteen_bit_channels";

const MIDI_CC_ROTARY_TAG_PREFIX: &str = "midi:cc:rotary:";

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

const MIDI_7_BIT_VALUE_RANGE: (i32, i32) = (0, 127);
const MIDI_14_BIT_VALUE_RANGE: (i32, i32) = (0, MIDI_U14_MAX as i32);
const MIDI_NONNEGATIVE_INT_RANGE: (i32, i32) = (0, i32::MAX);

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MidiCcTagConfig {
    rotary_mechanism: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MidiOrderedValueKind {
    Note,
    ControlChange,
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
        folder(fourteen_bit_channels, label = "14-bit Channels", collapsed = true) {
            channel_1: bool = false (label = "Channel 1", description = "Treat CC 0-31 on MIDI channel 1 as 14-bit controls paired with CC 32-63.");
            channel_2: bool = false (label = "Channel 2", description = "Treat CC 0-31 on MIDI channel 2 as 14-bit controls paired with CC 32-63.");
            channel_3: bool = false (label = "Channel 3", description = "Treat CC 0-31 on MIDI channel 3 as 14-bit controls paired with CC 32-63.");
            channel_4: bool = false (label = "Channel 4", description = "Treat CC 0-31 on MIDI channel 4 as 14-bit controls paired with CC 32-63.");
            channel_5: bool = false (label = "Channel 5", description = "Treat CC 0-31 on MIDI channel 5 as 14-bit controls paired with CC 32-63.");
            channel_6: bool = false (label = "Channel 6", description = "Treat CC 0-31 on MIDI channel 6 as 14-bit controls paired with CC 32-63.");
            channel_7: bool = false (label = "Channel 7", description = "Treat CC 0-31 on MIDI channel 7 as 14-bit controls paired with CC 32-63.");
            channel_8: bool = false (label = "Channel 8", description = "Treat CC 0-31 on MIDI channel 8 as 14-bit controls paired with CC 32-63.");
            channel_9: bool = false (label = "Channel 9", description = "Treat CC 0-31 on MIDI channel 9 as 14-bit controls paired with CC 32-63.");
            channel_10: bool = false (label = "Channel 10", description = "Treat CC 0-31 on MIDI channel 10 as 14-bit controls paired with CC 32-63.");
            channel_11: bool = false (label = "Channel 11", description = "Treat CC 0-31 on MIDI channel 11 as 14-bit controls paired with CC 32-63.");
            channel_12: bool = false (label = "Channel 12", description = "Treat CC 0-31 on MIDI channel 12 as 14-bit controls paired with CC 32-63.");
            channel_13: bool = false (label = "Channel 13", description = "Treat CC 0-31 on MIDI channel 13 as 14-bit controls paired with CC 32-63.");
            channel_14: bool = false (label = "Channel 14", description = "Treat CC 0-31 on MIDI channel 14 as 14-bit controls paired with CC 32-63.");
            channel_15: bool = false (label = "Channel 15", description = "Treat CC 0-31 on MIDI channel 15 as 14-bit controls paired with CC 32-63.");
            channel_16: bool = false (label = "Channel 16", description = "Treat CC 0-31 on MIDI channel 16 as 14-bit controls paired with CC 32-63.");
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
    pending_auto_children: HashSet<(NodeId, String)>,
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
            HashSet::new(),
        )
    }

    #[cfg(test)]
    pub(crate) fn enqueue_incoming_message_for_test(&mut self, message: MidiMessage) {
        self.pending_incoming_messages.push(message);
    }

    #[cfg(test)]
    pub(crate) fn auto_add_enabled_for_test(&self) -> bool {
        self.auto_add.get()
    }

    fn module_command_types() -> &'static [&'static str] {
        MIDI_MODULE_COMMAND_TYPES
    }

    fn clear_pending_auto_child(&mut self, parent_id: NodeId, decl_id: &str) {
        self.pending_auto_children.remove(&(parent_id, decl_id.to_string()));
    }

    fn mark_pending_auto_child(&mut self, parent_id: NodeId, decl_id: &str) -> bool {
        self.pending_auto_children.insert((parent_id, decl_id.to_string()))
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
                    self.input_port.clear_warning(ctx, Some(MIDI_PORT_OPTIONS_WARNING_ID));
                }
                if self.output_port.is_bound() {
                    sync_midi_port_enum_options(ctx, self.output_port.id(), midi_output_port_options(&options.outputs));
                    self.output_port.clear_warning(ctx, Some(MIDI_PORT_OPTIONS_WARNING_ID));
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
                        logerror!(
                            "Ignored unsupported MIDI message {}",
                            format_midi_bytes(bytes.as_slice())
                        );
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
            MidiMessage::ProgramChange { channel, program } => self.apply_channel_parameter(
                ctx,
                snapshot,
                *channel,
                PROGRAM_DECL_ID,
                "Program",
                i32::from(*program),
                Some((0, 127)),
            ),
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
        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };
        let Some(notes_id) = self.ensure_folder(ctx, snapshot, channel_id, "Notes", NOTES_FOLDER_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        self.apply_direct_int_parameter(
            ctx,
            snapshot,
            notes_id,
            note_label(note).as_str(),
            note_decl_id(note).as_str(),
            i32::from(velocity),
            Some(MIDI_7_BIT_VALUE_RANGE),
        )
    }

    fn apply_note_off(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        note: u8,
        _velocity: u8,
    ) -> MidiApplyResult {
        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };
        let Some(notes_id) = self.ensure_folder(ctx, snapshot, channel_id, "Notes", NOTES_FOLDER_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        self.apply_direct_int_parameter(
            ctx,
            snapshot,
            notes_id,
            note_label(note).as_str(),
            note_decl_id(note).as_str(),
            0,
            Some(MIDI_7_BIT_VALUE_RANGE),
        )
    }

    fn cc_parameter_range(
        &self,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        controller: u8,
    ) -> Option<(i32, i32)> {
        Some(if self.cc_parameter_is_14_bit(snapshot, channel, controller) {
            MIDI_14_BIT_VALUE_RANGE
        } else {
            MIDI_7_BIT_VALUE_RANGE
        })
    }

    fn apply_control_change(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        controller: u8,
        raw_value: u8,
    ) -> MidiApplyResult {
        if self.cc_channel_is_14_bit(snapshot, channel) && (32..=63).contains(&controller) {
            let base_controller = controller - 32;
            if self.cc_parameter_is_14_bit(snapshot, channel, base_controller) {
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
        let Some(cc_id) = self.ensure_cc_parameter(ctx, snapshot, channel, controller) else {
            return MidiApplyResult::Retry;
        };
        let config = midi_cc_tag_config(snapshot.node(cc_id).map(|node| node.tags.as_slice()).unwrap_or(&[]));

        if config.rotary_mechanism != ROTARY_ABSOLUTE {
            let current = snapshot
                .node(cc_id)
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_int)
                .unwrap_or(0);
            let delta = decode_rotary_delta(config.rotary_mechanism, raw_value).unwrap_or(0);
            self.set_internal_param(ctx, cc_id, ParamValue::Int(current.saturating_add(delta)));
            return MidiApplyResult::Applied;
        }

        if self.cc_parameter_is_14_bit(snapshot, channel, controller) {
            let current_combined = snapshot
                .node(cc_id)
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_int)
                .map(clamp_i32_to_u14)
                .unwrap_or(0);
            let default_msb = ((current_combined >> 7) & 0x7F) as u8;
            let default_lsb = (current_combined & 0x7F) as u8;
            let channel_index = usize::from(channel.saturating_sub(1).min(15));
            let controller_index = usize::from(controller);
            let state = &mut self.cc_14_bit_state[channel_index][controller_index];
            if is_lsb_message {
                state.lsb = Some(raw_value);
            } else {
                state.msb = Some(raw_value);
            }

            let msb = state.msb.unwrap_or(default_msb);
            let lsb = state.lsb.unwrap_or(default_lsb);
            let combined = (u16::from(msb) << 7) | u16::from(lsb);
            self.set_internal_param(ctx, cc_id, ParamValue::Int(i32::from(combined)));
            return MidiApplyResult::Applied;
        }

        self.set_internal_param(ctx, cc_id, ParamValue::Int(i32::from(raw_value)));
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
        let Some(pressure_folder_id) =
            self.ensure_folder(ctx, snapshot, channel_id, "Poly Pressure", POLY_PRESSURE_FOLDER_DECL_ID)
        else {
            return MidiApplyResult::Retry;
        };

        self.apply_direct_int_parameter(
            ctx,
            snapshot,
            pressure_folder_id,
            note_label(note).as_str(),
            note_decl_id(note).as_str(),
            i32::from(pressure),
            Some(MIDI_7_BIT_VALUE_RANGE),
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
                Some(MIDI_7_BIT_VALUE_RANGE),
            ),
            MidiSystemMessage::SongPosition { position } => self.apply_direct_int_parameter(
                ctx,
                snapshot,
                system_id,
                "Song Position",
                SONG_POSITION_DECL_ID,
                i32::from(*position),
                Some(MIDI_14_BIT_VALUE_RANGE),
            ),
            MidiSystemMessage::SongSelect { song } => self.apply_direct_int_parameter(
                ctx,
                snapshot,
                system_id,
                "Song Select",
                SONG_SELECT_DECL_ID,
                i32::from(*song),
                Some(MIDI_7_BIT_VALUE_RANGE),
            ),
            MidiSystemMessage::TuneRequest => {
                self.apply_trigger(ctx, snapshot, system_id, "Tune Request", TUNE_REQUEST_DECL_ID)
            }
            MidiSystemMessage::TimingClock => {
                self.apply_trigger(ctx, snapshot, system_id, "Timing Clock", TIMING_CLOCK_DECL_ID)
            }
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
            Some(MIDI_NONNEGATIVE_INT_RANGE),
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
            Some(existing_id)
                if snapshot
                    .node(existing_id)
                    .is_some_and(|node| node.node_type == "trigger") =>
            {
                self.clear_pending_auto_child(parent_id, decl_id);
                self.set_internal_param(ctx, existing_id, ParamValue::Trigger());
                MidiApplyResult::Applied
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                if self.mark_pending_auto_child(parent_id, decl_id) {
                    ctx.replace_node_boxed(existing_id, Box::new(create_trigger_parameter(label, decl_id)));
                }
                MidiApplyResult::Retry
            }
            None => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                if self.mark_pending_auto_child(parent_id, decl_id) {
                    ctx.add_child_boxed(parent_id, Box::new(create_trigger_parameter(label, decl_id)), None);
                }
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
                self.clear_pending_auto_child(parent_id, decl_id);
                sync_int_parameter_constraints(ctx, snapshot, existing_id, range);
                self.set_internal_param(ctx, existing_id, ParamValue::Int(value));
                MidiApplyResult::Applied
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                if self.mark_pending_auto_child(parent_id, decl_id) {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_int_parameter(label, decl_id, value, range, false)),
                    );
                }
                MidiApplyResult::Retry
            }
            None => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                if self.mark_pending_auto_child(parent_id, decl_id) {
                    ctx.add_child_boxed(
                        parent_id,
                        Box::new(create_int_parameter(label, decl_id, value, range, false)),
                        None,
                    );
                    if ordered_midi_value_kind(snapshot, parent_id).is_some() {
                        self.schedule_ordered_value_rebuild(ctx, parent_id);
                    }
                }
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
                self.clear_pending_auto_child(parent_id, decl_id);
                self.set_internal_param(ctx, existing_id, ParamValue::Str(value));
                MidiApplyResult::Applied
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                if self.mark_pending_auto_child(parent_id, decl_id) {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_string_parameter(label, decl_id, value, read_only)),
                    );
                }
                MidiApplyResult::Retry
            }
            None => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                if self.mark_pending_auto_child(parent_id, decl_id) {
                    ctx.add_child_boxed(
                        parent_id,
                        Box::new(create_string_parameter(label, decl_id, value, read_only)),
                        None,
                    );
                }
                MidiApplyResult::Retry
            }
        }
    }

    fn ensure_channel_folder(
        &mut self,
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

    fn ensure_cc_parameter(
        &mut self,
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
        let range = self.cc_parameter_range(snapshot, channel, controller);
        match snapshot.find_child_by_decl_id(cc_folder_id, decl_id.as_str()) {
            Some(existing_id) if snapshot.node(existing_id).is_some_and(|node| node.node_type == "int") => {
                self.clear_pending_auto_child(cc_folder_id, decl_id.as_str());
                sync_int_parameter_constraints(ctx, snapshot, existing_id, range);
                Some(existing_id)
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return None;
                }
                if self.mark_pending_auto_child(cc_folder_id, decl_id.as_str()) {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_int_parameter(
                            cc_label(controller).as_str(),
                            decl_id.as_str(),
                            0,
                            range,
                            false,
                        )),
                    );
                }
                None
            }
            None => {
                if !self.auto_add.get() {
                    return None;
                }
                if self.mark_pending_auto_child(cc_folder_id, decl_id.as_str()) {
                    ctx.add_child_boxed(
                        cc_folder_id,
                        Box::new(create_int_parameter(
                            cc_label(controller).as_str(),
                            decl_id.as_str(),
                            0,
                            range,
                            false,
                        )),
                        None,
                    );
                    self.schedule_ordered_value_rebuild(ctx, cc_folder_id);
                }
                None
            }
        }
    }

    fn ensure_folder(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        parent_id: NodeId,
        label: &str,
        decl_id: &str,
    ) -> Option<NodeId> {
        match snapshot.find_child_by_decl_id(parent_id, decl_id) {
            Some(existing_id)
                if snapshot
                    .node(existing_id)
                    .is_some_and(|node| node.node_type == "folder") =>
            {
                self.clear_pending_auto_child(parent_id, decl_id);
                Some(existing_id)
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return None;
                }
                if self.mark_pending_auto_child(parent_id, decl_id) {
                    ctx.replace_node_boxed(existing_id, Box::new(create_folder(label, decl_id)));
                }
                None
            }
            None => {
                if !self.auto_add.get() {
                    return None;
                }
                if self.mark_pending_auto_child(parent_id, decl_id) {
                    ctx.add_child_boxed(parent_id, Box::new(create_folder(label, decl_id)), None);
                }
                None
            }
        }
    }

    fn find_cc_folder(&self, snapshot: &ProcessTreeSnapshot, channel: u8) -> Option<NodeId> {
        let values_id = self.base.values_id()?;
        let channel_id = snapshot.find_child_by_decl_id(values_id, channel_decl_id(channel).as_str())?;
        snapshot.find_child_by_decl_id(channel_id, CONTROL_CHANGE_FOLDER_DECL_ID)
    }

    fn cc_channel_is_14_bit(&self, _snapshot: &ProcessTreeSnapshot, channel: u8) -> bool {
        match channel {
            1 => self.channel_1.get(),
            2 => self.channel_2.get(),
            3 => self.channel_3.get(),
            4 => self.channel_4.get(),
            5 => self.channel_5.get(),
            6 => self.channel_6.get(),
            7 => self.channel_7.get(),
            8 => self.channel_8.get(),
            9 => self.channel_9.get(),
            10 => self.channel_10.get(),
            11 => self.channel_11.get(),
            12 => self.channel_12.get(),
            13 => self.channel_13.get(),
            14 => self.channel_14.get(),
            15 => self.channel_15.get(),
            16 => self.channel_16.get(),
            _ => false,
        }
    }

    fn cc_parameter_is_14_bit(&self, snapshot: &ProcessTreeSnapshot, channel: u8, controller: u8) -> bool {
        cc_supports_14_bit(controller) && self.cc_channel_is_14_bit(snapshot, channel)
    }

    fn channel_14_bit_toggle_channel(&self, param: NodeId) -> Option<u8> {
        if self.channel_1.is_bound() && self.channel_1.id() == param {
            return Some(1);
        }
        if self.channel_2.is_bound() && self.channel_2.id() == param {
            return Some(2);
        }
        if self.channel_3.is_bound() && self.channel_3.id() == param {
            return Some(3);
        }
        if self.channel_4.is_bound() && self.channel_4.id() == param {
            return Some(4);
        }
        if self.channel_5.is_bound() && self.channel_5.id() == param {
            return Some(5);
        }
        if self.channel_6.is_bound() && self.channel_6.id() == param {
            return Some(6);
        }
        if self.channel_7.is_bound() && self.channel_7.id() == param {
            return Some(7);
        }
        if self.channel_8.is_bound() && self.channel_8.id() == param {
            return Some(8);
        }
        if self.channel_9.is_bound() && self.channel_9.id() == param {
            return Some(9);
        }
        if self.channel_10.is_bound() && self.channel_10.id() == param {
            return Some(10);
        }
        if self.channel_11.is_bound() && self.channel_11.id() == param {
            return Some(11);
        }
        if self.channel_12.is_bound() && self.channel_12.id() == param {
            return Some(12);
        }
        if self.channel_13.is_bound() && self.channel_13.id() == param {
            return Some(13);
        }
        if self.channel_14.is_bound() && self.channel_14.id() == param {
            return Some(14);
        }
        if self.channel_15.is_bound() && self.channel_15.id() == param {
            return Some(15);
        }
        if self.channel_16.is_bound() && self.channel_16.id() == param {
            return Some(16);
        }
        None
    }

    fn set_or_create_cc_param(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        cc_folder_id: NodeId,
        controller: u8,
        existing_id: Option<NodeId>,
        value: i32,
    ) {
        let decl_id = cc_decl_id(controller);
        let label = cc_label(controller);
        let range = self.cc_parameter_range(snapshot, channel, controller);
        match existing_id {
            Some(node_id) if snapshot.node(node_id).is_some_and(|node| node.node_type == "int") => {
                sync_int_parameter_constraints(ctx, snapshot, node_id, range);
                self.set_internal_param(ctx, node_id, ParamValue::Int(value));
            }
            Some(node_id) => {
                self.clear_pending_auto_child(cc_folder_id, decl_id.as_str());
                ctx.replace_node_boxed(
                    node_id,
                    Box::new(create_int_parameter(
                        label.as_str(),
                        decl_id.as_str(),
                        value,
                        range,
                        false,
                    )),
                );
            }
            None => {
                self.clear_pending_auto_child(cc_folder_id, decl_id.as_str());
                ctx.add_child_boxed(
                    cc_folder_id,
                    Box::new(create_int_parameter(
                        label.as_str(),
                        decl_id.as_str(),
                        value,
                        range,
                        false,
                    )),
                    None,
                );
                self.schedule_ordered_value_rebuild(ctx, cc_folder_id);
            }
        }
    }

    fn schedule_ordered_value_rebuild(&self, ctx: &mut ProcessCtx, parent_id: NodeId) {
        ctx.call_node_mutation(self.id(), move |_node, inner_ctx| {
            let Some(snapshot_arc) = inner_ctx.tree_snapshot_arc() else {
                return Ok(());
            };
            reorder_ordered_midi_value_children(inner_ctx, snapshot_arc.as_ref(), parent_id);
            Ok(())
        });
    }

    fn set_channel_14_bit_mode(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        enable_14_bit: bool,
    ) {
        let channel_index = usize::from(channel.saturating_sub(1).min(15));
        self.cc_14_bit_state[channel_index] = [Cc14BitState::default(); 32];

        let Some(cc_folder_id) = self.find_cc_folder(snapshot, channel) else {
            return;
        };

        for base_controller in 0..32u8 {
            let lsb_controller = base_controller + 32;
            let base_decl_id = cc_decl_id(base_controller);
            let lsb_decl_id = cc_decl_id(lsb_controller);
            let base_id = snapshot.find_child_by_decl_id(cc_folder_id, base_decl_id.as_str());
            let lsb_id = snapshot.find_child_by_decl_id(cc_folder_id, lsb_decl_id.as_str());

            if enable_14_bit {
                if base_id.is_none() && lsb_id.is_none() {
                    continue;
                }

                let msb = base_id
                    .and_then(|node_id| snapshot.node(node_id))
                    .and_then(|node| node.param_value.as_ref())
                    .and_then(ParamValue::as_int)
                    .map(clamp_i32_to_u7)
                    .unwrap_or(0);
                let lsb = lsb_id
                    .and_then(|node_id| snapshot.node(node_id))
                    .and_then(|node| node.param_value.as_ref())
                    .and_then(ParamValue::as_int)
                    .map(clamp_i32_to_u7)
                    .unwrap_or(0);
                let combined = i32::from((u16::from(msb) << 7) | u16::from(lsb));

                self.set_or_create_cc_param(ctx, snapshot, channel, cc_folder_id, base_controller, base_id, combined);
                if let Some(node_id) = lsb_id {
                    self.clear_pending_auto_child(cc_folder_id, lsb_decl_id.as_str());
                    ctx.edits.push(Edit::RemoveNode { node: node_id });
                }

                self.cc_14_bit_state[channel_index][usize::from(base_controller)] = Cc14BitState {
                    msb: Some(msb),
                    lsb: Some(lsb),
                };
                continue;
            }

            if base_id.is_none() && lsb_id.is_none() {
                continue;
            }

            let combined = base_id
                .and_then(|node_id| snapshot.node(node_id))
                .and_then(|node| node.param_value.as_ref())
                .and_then(ParamValue::as_int)
                .map(clamp_i32_to_u14)
                .unwrap_or_else(|| {
                    lsb_id
                        .and_then(|node_id| snapshot.node(node_id))
                        .and_then(|node| node.param_value.as_ref())
                        .and_then(ParamValue::as_int)
                        .map(clamp_i32_to_u7)
                        .map(u16::from)
                        .unwrap_or(0)
                });
            let msb = i32::from((combined >> 7) & 0x7F);
            let lsb = i32::from(combined & 0x7F);

            self.set_or_create_cc_param(ctx, snapshot, channel, cc_folder_id, base_controller, base_id, msb);
            self.set_or_create_cc_param(ctx, snapshot, channel, cc_folder_id, lsb_controller, lsb_id, lsb);
        }
    }

    fn lock_existing_value_tree(&self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let Some(values_id) = self.base.values_id() else {
            return;
        };

        let mut pending = vec![values_id];
        while let Some(node_id) = pending.pop() {
            lock_midi_value_node(ctx, node_id);
            pending.extend(snapshot.child_ids(node_id));
        }
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

        if let Some(channel) = self.channel_14_bit_toggle_channel(param) {
            ctx.call_node_mutation(self.id(), move |node, inner_ctx| {
                let Some(module) = node.as_any_mut().downcast_mut::<MidiModule>() else {
                    return Err("expected MidiModule during deferred 14-bit channel conversion".to_string());
                };
                let Some(snapshot_arc) = inner_ctx.tree_snapshot_arc() else {
                    return Ok(());
                };
                let enabled = module.cc_channel_is_14_bit(snapshot_arc.as_ref(), channel);
                module.set_channel_14_bit_mode(inner_ctx, snapshot_arc.as_ref(), channel, enabled);
                Ok(())
            });
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
        self.lock_existing_value_tree(ctx, snapshot);
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

    if parent.decl_id.as_str() == NOTES_FOLDER_DECL_ID {
        return feedback_messages_for_note_param(snapshot, param_snapshot);
    }

    if parent.decl_id.as_str() == CONTROL_CHANGE_FOLDER_DECL_ID {
        return feedback_messages_for_cc_param(snapshot, param_snapshot, old_value);
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
    param: &golden_core::process_ctx::ProcessTreeNodeSnapshot,
) -> Option<Vec<MidiMessage>> {
    let channel = channel_for_descendant(snapshot, param.id)?;
    let note = note_from_decl_id(param.decl_id.as_str())?;
    let velocity = param.param_value.as_ref()?.as_int().map(clamp_i32_to_u7)?;

    Some(vec![if velocity == 0 {
        MidiMessage::NoteOff {
            channel,
            note,
            velocity: 0,
        }
    } else {
        MidiMessage::NoteOn {
            channel,
            note,
            velocity,
        }
    }])
}

fn feedback_messages_for_cc_param(
    snapshot: &ProcessTreeSnapshot,
    param: &golden_core::process_ctx::ProcessTreeNodeSnapshot,
    old_value: ParamValue,
) -> Option<Vec<MidiMessage>> {
    let channel = channel_for_descendant(snapshot, param.id)?;
    let controller = cc_from_decl_id(param.decl_id.as_str())?;
    let value = param.param_value.as_ref()?.as_int().unwrap_or(0);
    let config = midi_cc_tag_config(param.tags.as_slice());
    let channel_14_bit = midi_cc_channel_is_14_bit(snapshot, param.id, channel);

    if config.rotary_mechanism != ROTARY_ABSOLUTE {
        let old = old_value.as_int().unwrap_or(value);
        let delta = value.saturating_sub(old);
        let raw_delta = encode_rotary_delta(config.rotary_mechanism, delta)?;
        return Some(vec![MidiMessage::ControlChange {
            channel,
            controller,
            value: raw_delta,
        }]);
    }

    if channel_14_bit && cc_supports_14_bit(controller) {
        return Some(encode_14_bit_control_change(
            channel,
            controller,
            clamp_i32_to_u14(value),
        ));
    }

    if channel_14_bit && (32..=63).contains(&controller) {
        return None;
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

fn create_folder(label: &str, decl_id: &str) -> Folder {
    let mut folder = Folder::new(label);
    apply_midi_value_identity(folder.node_data_mut(), decl_id);
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
    apply_midi_value_identity(parameter.node_data_mut(), decl_id);
    parameter.read_only = read_only;
    parameter.constraints = int_parameter_constraints(range);
    parameter
}

fn create_string_parameter(label: &str, decl_id: &str, value: String, read_only: bool) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Str(value), ParameterChangeCheck::ValueChange);
    apply_midi_value_identity(parameter.node_data_mut(), decl_id);
    parameter.read_only = read_only;
    parameter
}

fn create_trigger_parameter(label: &str, decl_id: &str) -> Parameter {
    let mut parameter = Parameter::new(label, ParamValue::Trigger(), ParameterChangeCheck::ValueChange);
    apply_midi_value_identity(parameter.node_data_mut(), decl_id);
    parameter
}

fn apply_midi_value_identity(node_data: &mut golden_core::node::NodeData, decl_id: &str) {
    apply_node_identity(node_data, decl_id);
    crate::app::module::enable_module_authoring(node_data);
    node_data.meta.user_permissions = NodeUserPermissions::none();
}

fn int_parameter_constraints(range: Option<(i32, i32)>) -> ParameterConstraints {
    match range {
        Some((min, max)) => ParameterConstraints {
            range: RangeConstraint::uniform(Some(f64::from(min)), Some(f64::from(max))),
            ..Default::default()
        },
        None => ParameterConstraints::default(),
    }
}

fn sync_int_parameter_constraints(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    node_id: NodeId,
    range: Option<(i32, i32)>,
) {
    let expected = int_parameter_constraints(range);
    if snapshot.node(node_id).and_then(|node| node.param_constraints.as_ref()) == Some(&expected) {
        return;
    }

    ctx.edits.push(Edit::SetParamConstraints {
        node: node_id,
        constraints: expected,
    });
}

fn lock_midi_value_node(ctx: &mut ProcessCtx, node_id: NodeId) {
    ctx.edits.push(Edit::PatchMeta {
        node: node_id,
        patch: NodeMetaPatch {
            user_permissions: Some(NodeUserPermissions::none()),
            ..Default::default()
        },
    });
}

fn apply_node_identity(node_data: &mut golden_core::node::NodeData, decl_id: &str) {
    node_data.meta.decl_id = DeclId(decl_id.to_string());
    node_data.meta.short_name = decl_id.to_string();
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

fn cc_from_decl_id(decl_id: &str) -> Option<u8> {
    decl_id
        .strip_prefix("cc_")?
        .parse::<u8>()
        .ok()
        .filter(|value| *value <= MIDI_DATA_MAX)
}

fn ordered_midi_value_kind(snapshot: &ProcessTreeSnapshot, parent_id: NodeId) -> Option<MidiOrderedValueKind> {
    match snapshot.node(parent_id)?.decl_id.as_str() {
        NOTES_FOLDER_DECL_ID => Some(MidiOrderedValueKind::Note),
        CONTROL_CHANGE_FOLDER_DECL_ID => Some(MidiOrderedValueKind::ControlChange),
        _ => None,
    }
}

fn ordered_midi_value_key(kind: MidiOrderedValueKind, decl_id: &str) -> Option<u8> {
    match kind {
        MidiOrderedValueKind::Note => note_from_decl_id(decl_id),
        MidiOrderedValueKind::ControlChange => cc_from_decl_id(decl_id),
    }
}

fn reorder_ordered_midi_value_children(ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot, parent_id: NodeId) {
    let Some(kind) = ordered_midi_value_kind(snapshot, parent_id) else {
        return;
    };

    let mut current_children = snapshot
        .child_ids(parent_id)
        .into_iter()
        .filter_map(|child_id| {
            let key = snapshot
                .node(child_id)
                .and_then(|node| ordered_midi_value_key(kind, node.decl_id.as_str()))?;
            Some((key, child_id))
        })
        .collect::<Vec<_>>();

    let mut ordered_children = current_children.clone();
    ordered_children.sort_by_key(|(key, _child_id)| *key);

    if current_children == ordered_children {
        return;
    }

    for (_key, child_id) in ordered_children.into_iter().rev() {
        let Some(current_index) = current_children.iter().position(|(_key, current_id)| *current_id == child_id) else {
            continue;
        };
        if current_index == 0 {
            continue;
        }

        let entry = current_children.remove(current_index);
        current_children.insert(0, entry);
        ctx.edits.push(Edit::MoveNode {
            node: child_id,
            new_parent: parent_id,
            new_prev_sibling: None,
        });
    }
}

fn midi_cc_tag_config(tags: &[String]) -> MidiCcTagConfig {
    let rotary_mechanism = tags
        .iter()
        .find_map(|tag| tag.strip_prefix(MIDI_CC_ROTARY_TAG_PREFIX))
        .and_then(normalize_rotary_mechanism_tag)
        .unwrap_or(ROTARY_ABSOLUTE);

    MidiCcTagConfig { rotary_mechanism }
}

fn normalize_rotary_mechanism_tag(mechanism: &str) -> Option<&'static str> {
    match mechanism {
        ROTARY_ABSOLUTE => Some(ROTARY_ABSOLUTE),
        ROTARY_TWOS_COMPLEMENT => Some(ROTARY_TWOS_COMPLEMENT),
        ROTARY_BINARY_OFFSET => Some(ROTARY_BINARY_OFFSET),
        ROTARY_SIGN_MAGNITUDE => Some(ROTARY_SIGN_MAGNITUDE),
        _ => None,
    }
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

fn enclosing_midi_module(snapshot: &ProcessTreeSnapshot, start: NodeId) -> Option<NodeId> {
    let mut current = Some(start);
    while let Some(node_id) = current {
        let node = snapshot.node(node_id)?;
        if node.node_type == MidiModule::NODE_TYPE {
            return Some(node_id);
        }
        current = node.parent;
    }
    None
}

fn midi_cc_channel_is_14_bit(snapshot: &ProcessTreeSnapshot, start: NodeId, channel: u8) -> bool {
    let Some(module_id) = enclosing_midi_module(snapshot, start) else {
        return false;
    };
    let Some(parameters_id) = snapshot.find_child_by_decl_id(module_id, "parameters") else {
        return false;
    };
    let Some(folder_id) = snapshot.find_child_by_decl_id(parameters_id, FOURTEEN_BIT_CHANNELS_FOLDER_DECL_ID) else {
        return false;
    };
    snapshot
        .find_child_by_decl_id(folder_id, channel_decl_id(channel).as_str())
        .and_then(|param_id| snapshot.node(param_id))
        .and_then(|node| node.param_value.as_ref())
        .and_then(ParamValue::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod midi_module_tests;
