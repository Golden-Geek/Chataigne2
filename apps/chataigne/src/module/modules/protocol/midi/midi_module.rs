use std::{collections::{HashSet, VecDeque}, time::Duration};

pub(crate) mod midi_message;
pub(crate) mod midi_runtime;

use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    logerror, node,
    node::{
        DeclId, Folder, Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor,
        NodeUserPermissions,
    },
    parameter::{
        Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, ParameterEnumOption,
        ParameterEventBehaviour, RangeConstraint,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use self::{
    midi_message::{
        MIDI_DATA_MAX, MIDI_PITCH_BEND_CENTER, MIDI_U14_MAX, MidiMessage, MidiSystemMessage, ROTARY_ABSOLUTE,
        ROTARY_BINARY_OFFSET, ROTARY_SIGN_MAGNITUDE, ROTARY_TWOS_COMPLEMENT, cc_decl_id, cc_label, cc_supports_14_bit,
        channel_decl_id, channel_folder_label, clamp_channel_i32, clamp_i32_to_u7, clamp_i32_to_u14,
        decode_rotary_delta, encode_14_bit_control_change, encode_midi_message, encode_rotary_delta,
        message_description, normalize_sysex_bytes, note_decl_id, note_label,
    },
    midi_runtime::{
        MidiClockTiming, MidiInputConfig, MidiInputEvent, MidiInputHandle, MidiOutputConfig, MidiOutputHandle,
        NO_MIDI_PORT_VARIANT, available_midi_port_options, format_midi_bytes, midi_input_port_available,
        midi_input_port_options, midi_output_port_available, midi_output_port_options, midi_port_selected,
        sync_midi_port_enum_options,
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

const MIDI_CC_ROTARY_MODE_DECL_ID: &str = "rotary_mode";

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
const MIDI_CLOCK_PLAYING_TIMEOUT_SECS: f64 = 0.5;
const MTC_PLAYING_TIMEOUT_SECS: f64 = 0.25;
const MIDI_CLOCK_PULSES_PER_BEAT: u8 = 24;
const MIDI_CLOCK_BPM_WINDOW_BEATS: usize = 4;
const DEFAULT_MIDI_CLOCK_BPM_PRECISION: i32 = 2;
const DEFAULT_SCRIPT_MIDI_CHANNEL: i32 = 1;
const DEFAULT_SCRIPT_MIDI_VELOCITY: i32 = 127;

const MIDI_MESSAGE_RECEIVED_CALLBACK: &str = "midiMessageReceived";
const MIDI_NOTE_ON_RECEIVED_CALLBACK: &str = "noteOnReceived";
const MIDI_NOTE_OFF_RECEIVED_CALLBACK: &str = "noteOffReceived";
const MIDI_CC_RECEIVED_CALLBACK: &str = "ccReceived";
const MIDI_SYSEX_RECEIVED_CALLBACK: &str = "sysExReceived";

const MIDI_SCRIPT_METHODS: &[&str] = &[
    "sendNoteOn",
    "sendNoteOff",
    "sendFullNote",
    "sendCC",
    "sendControlChange",
    "sendProgramChange",
    "sendPitchBend",
    "sendChannelPressure",
    "sendPolyPressure",
    "sendSysEx",
    "sendSysex",
    "sendRawBytes",
];

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LastNoteInfo {
    channel: u8,
    note: u8,
    velocity: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct MidiClockState {
    running: bool,
    bpm: f64,
    tick_in_beat: u8,
    last_tick_at: Option<Duration>,
    last_beat_at: Option<Duration>,
    recent_beat_durations: VecDeque<Duration>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MtcQuarterFrameState {
    nibbles: [Option<u8>; 8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MidiCcConfig {
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

#[derive(Clone, Debug, PartialEq)]
struct PendingIncomingMessage {
    message: MidiMessage,
    received_at: Option<Duration>,
    clock_timing: Option<MidiClockTiming>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MidiApplyResult {
    Applied,
    Retry,
    Ignored,
}

struct MidiIntValue<'a> {
    label: &'a str,
    decl_id: &'a str,
    value: i32,
    range: Option<(i32, i32)>,
}

struct MidiStringValue<'a> {
    label: &'a str,
    decl_id: &'a str,
    value: String,
    read_only: bool,
}

struct MidiCcUpdate {
    channel: u8,
    controller: u8,
    existing_id: Option<NodeId>,
    value: i32,
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
        [base_children];
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
            clock_bpm_precision: i32 = 0 [0..6] (
                label = "Clock BPM Precision",
                description = "Number of decimal places used for the exposed MIDI clock BPM. 0 rounds to a whole BPM."
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
        [base_children];
    }
    folder(values) {
        folder(mtc, label = "MTC", collapsed = true) {
            rate: String = "24 fps".to_string() (
                label = "Rate",
                description = "Decoded MTC frame rate.",
                read_only = true
            );
            mtc_playing: bool = false (
                label = "Playing",
                description = "Whether MTC quarter-frame messages are actively arriving.",
                read_only = true,
                short_name = "playing"
            );
            time: f64 = 0.0 [0.0..] (
                label = "Time",
                description = "Decoded MTC time in seconds.",
                read_only = true,
                widget = "time"
            );
            timecode: String = "00:00:00:00".to_string() (
                label = "Timecode",
                description = "Decoded MTC timecode.",
                read_only = true
            );
        }
        folder(midi_clock, label = "MIDI Clock", collapsed = true) {
            beat: ParamValue = ParamValue::Trigger() (
                label = "Beat",
                description = "Fires on each received MIDI clock beat (24 pulses).",
                read_only = true,
                short_name = "beat"
            );
            start: ParamValue = ParamValue::Trigger() (
                label = "Start",
                description = "Fires when MIDI clock start is received.",
                read_only = true
            );
            r#continue: ParamValue = ParamValue::Trigger() (
                label = "Continue",
                description = "Fires when MIDI clock continue is received.",
                read_only = true,
                short_name = "continue"
            );
            stop: ParamValue = ParamValue::Trigger() (
                label = "Stop",
                description = "Fires when MIDI clock stop is received.",
                read_only = true
            );
            reset: ParamValue = ParamValue::Trigger() (
                label = "Reset",
                description = "Fires when MIDI clock reset is received.",
                read_only = true
            );
            playing: bool = false (
                label = "Playing",
                description = "Whether the incoming MIDI clock transport is running.",
                read_only = true
            );
            bpm: f64 = 0.0 [0.0..] (
                label = "BPM",
                description = "Tempo estimated from recent incoming MIDI clock ticks.",
                read_only = true
            );
        }
        folder(note_info, label = "Note Info", collapsed = true) {
            note_played: ParamValue = ParamValue::Trigger() (
                label = "Note Played",
                description = "Fires when a note-on is received."
                // read_only = true
            );
            current_note_on: i32 = 0 [0..2147483647] (
                label = "Current Note On",
                description = "Number of notes currently held on.",
                read_only = true
            );
            last_channel: i32 = 0 [0..16] (
                label = "Last Channel",
                description = "Channel of the last received note-on.",
                read_only = true
            );
            last_pitch: i32 = 0 [0..127] (
                label = "Last Pitch",
                description = "Pitch of the last received note-on.",
                read_only = true
            );
            last_velocity: i32 = 0 [0..127] (
                label = "Last Velocity",
                description = "Velocity of the last received note-on.",
                read_only = true
            );
        }
        [base_children];
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
    pending_incoming_messages: Vec<PendingIncomingMessage>,
    ignored_param_changes: HashSet<NodeId>,
    pending_packets: Vec<PendingMidiPacket>,
    cc_14_bit_state: [[Cc14BitState; 32]; 16],
    active_notes: HashSet<(u8, u8)>,
    last_note_on: Option<LastNoteInfo>,
    midi_clock_state: MidiClockState,
    mtc_state: MtcQuarterFrameState,
    mtc_stream_active: bool,
    last_mtc_quarter_frame_at: Option<Duration>,
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
            None,
            MidiClockState::default(),
            MtcQuarterFrameState::default(),
            false,
            None,
            HashSet::new(),
        )
    }

    #[cfg(test)]
    pub(crate) fn enqueue_incoming_message_for_test(&mut self, message: MidiMessage) {
        self.pending_incoming_messages.push(PendingIncomingMessage {
            message,
            received_at: None,
            clock_timing: None,
        });
    }

    #[cfg(test)]
    pub(crate) fn auto_add_enabled_for_test(&self) -> bool {
        self.auto_add.get()
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

        let input_port_available = midi_input_port_available(config.port_variant.as_str()).unwrap_or(true);
        if self.input.is_some() && self.last_input_config.as_ref() == Some(&config) && input_port_available {
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

        let output_port_available = midi_output_port_available(config.port_variant.as_str()).unwrap_or(true);
        if self.output.is_some() && self.last_output_config.as_ref() == Some(&config) && output_port_available {
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
        self.base.set_connected(
            ctx,
            midi_transport_connected(
                self.input_config().is_some(),
                self.input.is_some(),
                self.output_config().is_some(),
                self.output.is_some(),
            ),
        );
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
                MidiInputEvent::Message {
                    bytes,
                    message,
                    received_at,
                    clock_timing,
                } => {
                    received = true;
                    self.log_incoming_message(&message, bytes.as_slice());
                    self.pending_incoming_messages.push(PendingIncomingMessage {
                        message,
                        received_at: Some(received_at),
                        clock_timing,
                    });
                }
                MidiInputEvent::UnsupportedMessage { bytes, .. } => {
                    logerror!(
                        "Ignored unsupported MIDI message {}",
                        format_midi_bytes(bytes.as_slice())
                    );
                }
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
        while let Some(pending) = messages.next() {
            let received_at = pending.received_at.unwrap_or(ctx.runtime_elapsed);
            match self.apply_incoming_message(
                ctx,
                snapshot,
                &pending.message,
                received_at,
                pending.clock_timing.as_ref(),
            ) {
                MidiApplyResult::Applied | MidiApplyResult::Ignored => {
                    self.emit_midi_received_callbacks(ctx, &pending.message);
                }
                MidiApplyResult::Retry => {
                    remaining.push(pending);
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
        received_at: Duration,
        clock_timing: Option<&MidiClockTiming>,
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
                MidiIntValue {
                    label: "Program",
                    decl_id: PROGRAM_DECL_ID,
                    value: i32::from(*program),
                    range: Some((0, 127)),
                },
            ),
            MidiMessage::ChannelPressure { channel, pressure } => self.apply_channel_parameter(
                ctx,
                snapshot,
                *channel,
                MidiIntValue {
                    label: "Channel Pressure",
                    decl_id: CHANNEL_PRESSURE_DECL_ID,
                    value: i32::from(*pressure),
                    range: Some((0, 127)),
                },
            ),
            MidiMessage::PitchBend { channel, value } => self.apply_channel_parameter(
                ctx,
                snapshot,
                *channel,
                MidiIntValue {
                    label: "Pitch Bend",
                    decl_id: PITCH_BEND_DECL_ID,
                    value: i32::from(*value),
                    range: Some((0, i32::from(MIDI_U14_MAX))),
                },
            ),
            MidiMessage::System(system) => {
                self.apply_system_message(ctx, snapshot, system, received_at, clock_timing)
            }
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
        self.active_notes.insert((channel, note));
        self.last_note_on = Some(LastNoteInfo {
            channel,
            note,
            velocity,
        });

        let note_info_result = self.apply_note_info(ctx, snapshot, true);
        if note_info_result == MidiApplyResult::Retry {
            return note_info_result;
        }

        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };
        let Some(notes_id) = self.ensure_folder(ctx, snapshot, channel_id, "Notes", NOTES_FOLDER_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        merge_apply_results(
            note_info_result,
            self.apply_direct_int_parameter(
                ctx,
                snapshot,
                notes_id,
                MidiIntValue {
                    label: note_label(note).as_str(),
                    decl_id: note_decl_id(note).as_str(),
                    value: i32::from(velocity),
                    range: Some(MIDI_7_BIT_VALUE_RANGE),
                },
            ),
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
        self.active_notes.remove(&(channel, note));

        let note_info_result = self.apply_note_info(ctx, snapshot, false);
        if note_info_result == MidiApplyResult::Retry {
            return note_info_result;
        }

        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };
        let Some(notes_id) = self.ensure_folder(ctx, snapshot, channel_id, "Notes", NOTES_FOLDER_DECL_ID) else {
            return MidiApplyResult::Retry;
        };
        merge_apply_results(
            note_info_result,
            self.apply_direct_int_parameter(
                ctx,
                snapshot,
                notes_id,
                MidiIntValue {
                    label: note_label(note).as_str(),
                    decl_id: note_decl_id(note).as_str(),
                    value: 0,
                    range: Some(MIDI_7_BIT_VALUE_RANGE),
                },
            ),
        )
    }

    fn apply_note_info(
        &mut self,
        ctx: &mut ProcessCtx,
        _snapshot: &ProcessTreeSnapshot,
        note_played: bool,
    ) -> MidiApplyResult {
        if !self.current_note_on.is_bound()
            || !self.last_channel.is_bound()
            || !self.last_pitch.is_bound()
            || !self.last_velocity.is_bound()
            || (note_played && !self.note_played.is_bound())
        {
            return MidiApplyResult::Retry;
        }

        if note_played {
            self.set_internal_param(ctx, self.note_played.id(), ParamValue::Trigger());
        }

        self.set_internal_param(
            ctx,
            self.current_note_on.id(),
            ParamValue::Int(clamp_usize_to_i32(self.active_notes.len())),
        );

        let last_channel = self.last_note_on.map(|info| i32::from(info.channel)).unwrap_or(0);
        self.set_internal_param(ctx, self.last_channel.id(), ParamValue::Int(last_channel));

        let last_pitch = self.last_note_on.map(|info| i32::from(info.note)).unwrap_or(0);
        self.set_internal_param(ctx, self.last_pitch.id(), ParamValue::Int(last_pitch));

        let last_velocity = self.last_note_on.map(|info| i32::from(info.velocity)).unwrap_or(0);
        self.set_internal_param(ctx, self.last_velocity.id(), ParamValue::Int(last_velocity));
        MidiApplyResult::Applied
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
        let config = midi_cc_config(snapshot, cc_id);

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
            MidiIntValue {
                label: note_label(note).as_str(),
                decl_id: note_decl_id(note).as_str(),
                value: i32::from(pressure),
                range: Some(MIDI_7_BIT_VALUE_RANGE),
            },
        )
    }

    fn apply_channel_parameter(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        channel: u8,
        value: MidiIntValue<'_>,
    ) -> MidiApplyResult {
        let Some(channel_id) = self.ensure_channel_folder(ctx, snapshot, channel) else {
            return MidiApplyResult::Retry;
        };

        self.apply_direct_int_parameter(ctx, snapshot, channel_id, value)
    }

    fn apply_system_message(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        message: &MidiSystemMessage,
        received_at: Duration,
        clock_timing: Option<&MidiClockTiming>,
    ) -> MidiApplyResult {
        match message {
            MidiSystemMessage::TimeCodeQuarterFrame { value } => {
                self.apply_time_code_quarter_frame(ctx, snapshot, *value, received_at)
            }
            MidiSystemMessage::SongPosition { position } => self.apply_system_int_parameter(
                ctx,
                snapshot,
                MidiIntValue {
                    label: "Song Position",
                    decl_id: SONG_POSITION_DECL_ID,
                    value: i32::from(*position),
                    range: Some(MIDI_14_BIT_VALUE_RANGE),
                },
            ),
            MidiSystemMessage::SongSelect { song } => self.apply_system_int_parameter(
                ctx,
                snapshot,
                MidiIntValue {
                    label: "Song Select",
                    decl_id: SONG_SELECT_DECL_ID,
                    value: i32::from(*song),
                    range: Some(MIDI_7_BIT_VALUE_RANGE),
                },
            ),
            MidiSystemMessage::TuneRequest => self.apply_system_trigger(ctx, snapshot, "Tune Request", TUNE_REQUEST_DECL_ID),
            MidiSystemMessage::TimingClock => {
                self.apply_midi_clock_tick(ctx, snapshot, received_at, clock_timing.copied())
            }
            MidiSystemMessage::Start => {
                self.apply_midi_clock_transport_event(ctx, snapshot, START_DECL_ID, true, true)
            }
            MidiSystemMessage::Continue => {
                self.apply_midi_clock_transport_event(ctx, snapshot, CONTINUE_DECL_ID, true, false)
            }
            MidiSystemMessage::Stop => {
                self.apply_midi_clock_transport_event(ctx, snapshot, STOP_DECL_ID, false, false)
            }
            MidiSystemMessage::ActiveSensing => {
                self.apply_system_trigger(ctx, snapshot, "Active Sensing", ACTIVE_SENSING_DECL_ID)
            }
            MidiSystemMessage::Reset => {
                self.apply_midi_clock_transport_event(ctx, snapshot, RESET_DECL_ID, false, true)
            }
            MidiSystemMessage::Sysex { bytes } => self.apply_sysex_to_system(ctx, snapshot, bytes.as_slice()),
        }
    }

    fn apply_system_int_parameter(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        value: MidiIntValue<'_>,
    ) -> MidiApplyResult {
        let Some(system_id) = self.ensure_system_folder(ctx, snapshot) else {
            return MidiApplyResult::Retry;
        };

        self.apply_direct_int_parameter(ctx, snapshot, system_id, value)
    }

    fn apply_system_trigger(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        label: &str,
        decl_id: &str,
    ) -> MidiApplyResult {
        let Some(system_id) = self.ensure_system_folder(ctx, snapshot) else {
            return MidiApplyResult::Retry;
        };

        self.apply_trigger(ctx, snapshot, system_id, label, decl_id)
    }

    fn ensure_system_folder(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        let values_id = self.base.values_id()?;
        self.ensure_folder(ctx, snapshot, values_id, "System", SYSTEM_FOLDER_DECL_ID)
    }

    fn apply_time_code_quarter_frame(
        &mut self,
        ctx: &mut ProcessCtx,
        _snapshot: &ProcessTreeSnapshot,
        raw_value: u8,
        received_at: Duration,
    ) -> MidiApplyResult {
        self.mtc_state.apply(raw_value);
        let was_active = self.mtc_stream_active;
        self.last_mtc_quarter_frame_at = Some(received_at);
        self.mtc_stream_active = true;

        if !self.rate.is_bound() || !self.mtc_playing.is_bound() || !self.time.is_bound() || !self.timecode.is_bound() {
            return MidiApplyResult::Retry;
        }

        if !was_active {
            self.set_internal_param(ctx, self.mtc_playing.id(), ParamValue::Bool(true));
        }
        self.set_internal_param(
            ctx,
            self.rate.id(),
            ParamValue::Str(mtc_rate_label(self.mtc_state.rate_code()).to_string()),
        );
        self.set_internal_param(ctx, self.time.id(), ParamValue::Float(self.mtc_state.time_seconds()));
        self.set_internal_param(ctx, self.timecode.id(), ParamValue::Str(self.mtc_state.timecode_string()));
        MidiApplyResult::Applied
    }

    fn apply_midi_clock_tick(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        received_at: Duration,
        clock_timing: Option<MidiClockTiming>,
    ) -> MidiApplyResult {
        let (beat_triggered, values_changed) = if let Some(clock_timing) = clock_timing {
            let mut values_changed = false;
            if !self.midi_clock_state.running {
                self.midi_clock_state.running = true;
                values_changed = true;
            }

            self.midi_clock_state.last_tick_at = Some(received_at);

            if let Some(next_bpm) = clock_timing.bpm {
                let previous_units = self.quantized_midi_clock_bpm_units(self.midi_clock_state.bpm);
                let next_units = self.quantized_midi_clock_bpm_units(next_bpm);
                self.midi_clock_state.bpm = next_bpm;
                if next_units != previous_units {
                    values_changed = true;
                }
            }

            (clock_timing.beat_triggered, values_changed)
        } else {
            self.record_midi_clock_tick_fallback(received_at)
        };

        let mut result = MidiApplyResult::Applied;

        if beat_triggered {
            if !self.beat.is_bound() {
                return MidiApplyResult::Retry;
            }
            self.set_internal_param(ctx, self.beat.id(), ParamValue::Trigger());
        }

        if values_changed {
            result = merge_apply_results(result, self.apply_midi_clock_values(ctx, snapshot));
        }

        result
    }

    fn apply_midi_clock_transport_event(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        decl_id: &str,
        running: bool,
        reset_ticks: bool,
    ) -> MidiApplyResult {
        self.midi_clock_state.tick_in_beat = 0;
        self.midi_clock_state.last_tick_at = None;
        self.midi_clock_state.last_beat_at = None;
        self.midi_clock_state.recent_beat_durations.clear();
        self.midi_clock_state.running = running;

        if reset_ticks || matches!(decl_id, START_DECL_ID) {
            self.midi_clock_state.bpm = 0.0;
        }

        let trigger_id = match decl_id {
            START_DECL_ID if self.start.is_bound() => Some(self.start.id()),
            CONTINUE_DECL_ID if self.r#continue.is_bound() => Some(self.r#continue.id()),
            STOP_DECL_ID if self.stop.is_bound() => Some(self.stop.id()),
            RESET_DECL_ID if self.reset.is_bound() => Some(self.reset.id()),
            _ => None,
        };
        let Some(trigger_id) = trigger_id else {
            return MidiApplyResult::Retry;
        };

        merge_apply_results(
            {
                self.set_internal_param(ctx, trigger_id, ParamValue::Trigger());
                MidiApplyResult::Applied
            },
            self.apply_midi_clock_values(ctx, snapshot),
        )
    }

    fn apply_midi_clock_values(
        &mut self,
        ctx: &mut ProcessCtx,
        _snapshot: &ProcessTreeSnapshot,
    ) -> MidiApplyResult {
        if !self.playing.is_bound() || !self.bpm.is_bound() {
            return MidiApplyResult::Retry;
        }

        self.set_internal_param(ctx, self.playing.id(), ParamValue::Bool(self.midi_clock_state.running));
        self.set_internal_param(ctx, self.bpm.id(), ParamValue::Float(self.quantized_midi_clock_bpm(self.midi_clock_state.bpm)));
        MidiApplyResult::Applied
    }

    fn midi_clock_bpm_precision_digits(&self) -> u32 {
        if !self.clock_bpm_precision.is_bound() {
            return DEFAULT_MIDI_CLOCK_BPM_PRECISION as u32;
        }

        self.clock_bpm_precision.get().clamp(0, 6) as u32
    }

    fn quantized_midi_clock_bpm(&self, bpm: f64) -> f64 {
        quantize_decimal(bpm, self.midi_clock_bpm_precision_digits())
    }

    fn quantized_midi_clock_bpm_units(&self, bpm: f64) -> i64 {
        quantize_decimal_units(bpm, self.midi_clock_bpm_precision_digits())
    }

    fn record_midi_clock_tick_fallback(&mut self, now: Duration) -> (bool, bool) {
        let mut values_changed = false;
        if !self.midi_clock_state.running {
            self.midi_clock_state.running = true;
            values_changed = true;
        }

        self.midi_clock_state.last_tick_at = Some(now);
        self.midi_clock_state.tick_in_beat = self.midi_clock_state.tick_in_beat.saturating_add(1);
        if self.midi_clock_state.tick_in_beat < MIDI_CLOCK_PULSES_PER_BEAT {
            return (false, values_changed);
        }

        self.midi_clock_state.tick_in_beat = 0;

        if let Some(last_beat_at) = self.midi_clock_state.last_beat_at {
            let beat_duration = now.saturating_sub(last_beat_at);
            if !beat_duration.is_zero() {
                self.midi_clock_state.recent_beat_durations.push_back(beat_duration);
                while self.midi_clock_state.recent_beat_durations.len() > MIDI_CLOCK_BPM_WINDOW_BEATS {
                    self.midi_clock_state.recent_beat_durations.pop_front();
                }

                if let Some(next_bpm) = stable_midi_clock_bpm(&self.midi_clock_state.recent_beat_durations) {
                    let previous_units = self.quantized_midi_clock_bpm_units(self.midi_clock_state.bpm);
                    let next_units = self.quantized_midi_clock_bpm_units(next_bpm);
                    self.midi_clock_state.bpm = next_bpm;
                    if next_units != previous_units {
                        values_changed = true;
                    }
                }
            }
        }

        self.midi_clock_state.last_beat_at = Some(now);

        (true, values_changed)
    }

    fn refresh_transport_activity(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) {
        let now = self.midi_receive_now(ctx);

        if self.midi_clock_state.running
            && self
                .midi_clock_state
                .last_tick_at
                .is_some_and(|last_tick_at| now.saturating_sub(last_tick_at).as_secs_f64() > MIDI_CLOCK_PLAYING_TIMEOUT_SECS)
        {
            self.midi_clock_state.running = false;
            let _ = self.apply_midi_clock_values(ctx, snapshot);
        }

        if self.mtc_stream_active
            && self
                .last_mtc_quarter_frame_at
                .is_some_and(|last_mtc_at| now.saturating_sub(last_mtc_at).as_secs_f64() > MTC_PLAYING_TIMEOUT_SECS)
        {
            self.mtc_stream_active = false;
            if self.mtc_playing.is_bound() {
                self.set_internal_param(ctx, self.mtc_playing.id(), ParamValue::Bool(false));
            }
        }
    }

    fn transport_activity_timeout_due(&self, now: Duration) -> bool {
        (self.midi_clock_state.running
            && self
                .midi_clock_state
                .last_tick_at
                .is_some_and(|last_tick_at| now.saturating_sub(last_tick_at).as_secs_f64() > MIDI_CLOCK_PLAYING_TIMEOUT_SECS))
            || (self.mtc_stream_active
                && self
                    .last_mtc_quarter_frame_at
                    .is_some_and(|last_mtc_at| now.saturating_sub(last_mtc_at).as_secs_f64() > MTC_PLAYING_TIMEOUT_SECS))
    }

    fn midi_receive_now(&self, ctx: &ProcessCtx) -> Duration {
        self.input
            .as_ref()
            .map(MidiInputHandle::elapsed)
            .unwrap_or(ctx.runtime_elapsed)
    }

    fn apply_sysex_to_system(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        bytes: &[u8],
    ) -> MidiApplyResult {
        let Some(system_id) = self.ensure_system_folder(ctx, snapshot) else {
            return MidiApplyResult::Retry;
        };

        self.apply_sysex(ctx, snapshot, system_id, bytes)
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
            MidiStringValue {
                label: "Bytes",
                decl_id: SYSEX_BYTES_DECL_ID,
                value: format_midi_bytes(bytes),
                read_only: true,
            },
        );
        if bytes_result == MidiApplyResult::Retry {
            return bytes_result;
        }

        self.apply_direct_int_parameter(
            ctx,
            snapshot,
            sysex_id,
            MidiIntValue {
                label: "Length",
                decl_id: SYSEX_LENGTH_DECL_ID,
                value: i32::try_from(bytes.len()).unwrap_or(i32::MAX),
                range: Some(MIDI_NONNEGATIVE_INT_RANGE),
            },
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
        value: MidiIntValue<'_>,
    ) -> MidiApplyResult {
        match snapshot.find_child_by_decl_id(parent_id, value.decl_id) {
            Some(existing_id) if snapshot.node(existing_id).is_some_and(|node| node.node_type == "int") => {
                self.clear_pending_auto_child(parent_id, value.decl_id);
                sync_int_parameter_constraints(ctx, snapshot, existing_id, value.range);
                self.set_internal_param(ctx, existing_id, ParamValue::Int(value.value));
                MidiApplyResult::Applied
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                if self.mark_pending_auto_child(parent_id, value.decl_id) {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_int_parameter(
                            value.label,
                            value.decl_id,
                            value.value,
                            value.range,
                            false,
                        )),
                    );
                }
                MidiApplyResult::Retry
            }
            None => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                if self.mark_pending_auto_child(parent_id, value.decl_id) {
                    ctx.add_child_boxed(
                        parent_id,
                        Box::new(create_int_parameter(
                            value.label,
                            value.decl_id,
                            value.value,
                            value.range,
                            false,
                        )),
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
        value: MidiStringValue<'_>,
    ) -> MidiApplyResult {
        match snapshot.find_child_by_decl_id(parent_id, value.decl_id) {
            Some(existing_id) if snapshot.node(existing_id).is_some_and(|node| node.node_type == "str") => {
                self.clear_pending_auto_child(parent_id, value.decl_id);
                self.set_internal_param(ctx, existing_id, ParamValue::Str(value.value));
                MidiApplyResult::Applied
            }
            Some(existing_id) => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                if self.mark_pending_auto_child(parent_id, value.decl_id) {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_string_parameter(
                            value.label,
                            value.decl_id,
                            value.value,
                            value.read_only,
                        )),
                    );
                }
                MidiApplyResult::Retry
            }
            None => {
                if !self.auto_add.get() {
                    return MidiApplyResult::Ignored;
                }
                if self.mark_pending_auto_child(parent_id, value.decl_id) {
                    ctx.add_child_boxed(
                        parent_id,
                        Box::new(create_string_parameter(
                            value.label,
                            value.decl_id,
                            value.value,
                            value.read_only,
                        )),
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
                self.ensure_cc_rotary_mode_parameter(ctx, snapshot, existing_id);
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

    fn ensure_cc_rotary_mode_parameter(
        &mut self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        cc_id: NodeId,
    ) {
        match snapshot.find_child_by_decl_id(cc_id, MIDI_CC_ROTARY_MODE_DECL_ID) {
            Some(existing_id) if snapshot.node(existing_id).is_some_and(|node| node.node_type == "enum") => {
                self.clear_pending_auto_child(cc_id, MIDI_CC_ROTARY_MODE_DECL_ID);
            }
            Some(existing_id) => {
                if self.mark_pending_auto_child(cc_id, MIDI_CC_ROTARY_MODE_DECL_ID) {
                    ctx.replace_node_boxed(
                        existing_id,
                        Box::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE)),
                    );
                }
            }
            None => {
                if self.mark_pending_auto_child(cc_id, MIDI_CC_ROTARY_MODE_DECL_ID) {
                    ctx.add_child_boxed(
                        cc_id,
                        Box::new(create_cc_rotary_mode_parameter(ROTARY_ABSOLUTE)),
                        None,
                    );
                }
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
        cc_folder_id: NodeId,
        update: MidiCcUpdate,
    ) {
        let decl_id = cc_decl_id(update.controller);
        let label = cc_label(update.controller);
        let range = self.cc_parameter_range(snapshot, update.channel, update.controller);
        match update.existing_id {
            Some(node_id) if snapshot.node(node_id).is_some_and(|node| node.node_type == "int") => {
                sync_int_parameter_constraints(ctx, snapshot, node_id, range);
                self.ensure_cc_rotary_mode_parameter(ctx, snapshot, node_id);
                self.set_internal_param(ctx, node_id, ParamValue::Int(update.value));
            }
            Some(node_id) => {
                self.clear_pending_auto_child(cc_folder_id, decl_id.as_str());
                ctx.replace_node_boxed(
                    node_id,
                    Box::new(create_int_parameter(
                        label.as_str(),
                        decl_id.as_str(),
                        update.value,
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
                        update.value,
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

                self.set_or_create_cc_param(
                    ctx,
                    snapshot,
                    cc_folder_id,
                    MidiCcUpdate {
                        channel,
                        controller: base_controller,
                        existing_id: base_id,
                        value: combined,
                    },
                );
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

            self.set_or_create_cc_param(
                ctx,
                snapshot,
                cc_folder_id,
                MidiCcUpdate {
                    channel,
                    controller: base_controller,
                    existing_id: base_id,
                    value: msb,
                },
            );
            self.set_or_create_cc_param(
                ctx,
                snapshot,
                cc_folder_id,
                MidiCcUpdate {
                    channel,
                    controller: lsb_controller,
                    existing_id: lsb_id,
                    value: lsb,
                },
            );
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

    fn emit_midi_received_callbacks(&self, ctx: &mut ProcessCtx, message: &MidiMessage) {
        use crate::app::module::script_api;

        script_api::emit_script_callback(
            ctx,
            self.id(),
            MIDI_MESSAGE_RECEIVED_CALLBACK,
            vec![midi_message_script_payload(message)],
        );

        match message {
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } if *velocity == 0 => script_api::emit_script_callback(
                ctx,
                self.id(),
                MIDI_NOTE_OFF_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(note),
                    serde_json::json!(0),
                ],
            ),
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } => script_api::emit_script_callback(
                ctx,
                self.id(),
                MIDI_NOTE_ON_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(note),
                    serde_json::json!(velocity),
                ],
            ),
            MidiMessage::NoteOff {
                channel,
                note,
                velocity,
            } => script_api::emit_script_callback(
                ctx,
                self.id(),
                MIDI_NOTE_OFF_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(note),
                    serde_json::json!(velocity),
                ],
            ),
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => script_api::emit_script_callback(
                ctx,
                self.id(),
                MIDI_CC_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(controller),
                    serde_json::json!(value),
                ],
            ),
            MidiMessage::System(MidiSystemMessage::Sysex { bytes }) => {
                script_api::emit_script_callback(
                    ctx,
                    self.id(),
                    MIDI_SYSEX_RECEIVED_CALLBACK,
                    vec![script_api::bytes_arg(bytes.as_slice())],
                );
            }
            _ => {}
        }
    }

    fn handle_script_send_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let result = match method {
            "sendNoteOn" => {
                let message = MidiMessage::NoteOn {
                    channel: script_channel_arg(args, 0, "channel"),
                    note: script_u7_arg(args, 1, "note"),
                    velocity: script_u7_arg_or(args, 2, DEFAULT_SCRIPT_MIDI_VELOCITY),
                };
                self.send_bytes(ctx, encode_midi_message(&message).as_slice(), message_description(&message).as_str())
            }
            "sendNoteOff" => {
                let message = MidiMessage::NoteOff {
                    channel: script_channel_arg(args, 0, "channel"),
                    note: script_u7_arg(args, 1, "note"),
                    velocity: script_u7_arg_or(args, 2, 0),
                };
                self.send_bytes(ctx, encode_midi_message(&message).as_slice(), message_description(&message).as_str())
            }
            "sendFullNote" => (|| -> Result<(), String> {
                let channel = script_channel_arg(args, 0, "channel");
                let note = script_u7_arg(args, 1, "note");
                let velocity = script_u7_arg_or(args, 2, DEFAULT_SCRIPT_MIDI_VELOCITY);
                let duration_ms = script_nonnegative_u64_arg_or(args, 3, 100);
                let off_velocity = script_u7_arg_or(args, 4, 0);
                let note_on = MidiMessage::NoteOn {
                    channel,
                    note,
                    velocity,
                };
                let note_off = MidiMessage::NoteOff {
                    channel,
                    note,
                    velocity: off_velocity,
                };
                self.send_bytes(
                    ctx,
                    encode_midi_message(&note_on).as_slice(),
                    message_description(&note_on).as_str(),
                )?;
                self.pending_packets.push(PendingMidiPacket {
                    due_at: ctx.runtime_elapsed + Duration::from_millis(duration_ms),
                    bytes: encode_midi_message(&note_off),
                    description: message_description(&note_off),
                });
                Ok(())
            })(),
            "sendCC" | "sendControlChange" => {
                let message = MidiMessage::ControlChange {
                    channel: script_channel_arg(args, 0, "channel"),
                    controller: script_u7_arg(args, 1, "controller"),
                    value: script_u7_arg(args, 2, "value"),
                };
                self.send_bytes(ctx, encode_midi_message(&message).as_slice(), message_description(&message).as_str())
            }
            "sendProgramChange" => {
                let message = MidiMessage::ProgramChange {
                    channel: script_channel_arg(args, 0, "channel"),
                    program: script_u7_arg(args, 1, "program"),
                };
                self.send_bytes(ctx, encode_midi_message(&message).as_slice(), message_description(&message).as_str())
            }
            "sendPitchBend" => {
                let message = MidiMessage::PitchBend {
                    channel: script_channel_arg(args, 0, "channel"),
                    value: script_u14_arg(args, 1, "value"),
                };
                self.send_bytes(ctx, encode_midi_message(&message).as_slice(), message_description(&message).as_str())
            }
            "sendChannelPressure" => {
                let message = MidiMessage::ChannelPressure {
                    channel: script_channel_arg(args, 0, "channel"),
                    pressure: script_u7_arg(args, 1, "pressure"),
                };
                self.send_bytes(ctx, encode_midi_message(&message).as_slice(), message_description(&message).as_str())
            }
            "sendPolyPressure" => {
                let message = MidiMessage::PolyPressure {
                    channel: script_channel_arg(args, 0, "channel"),
                    note: script_u7_arg(args, 1, "note"),
                    pressure: script_u7_arg(args, 2, "pressure"),
                };
                self.send_bytes(ctx, encode_midi_message(&message).as_slice(), message_description(&message).as_str())
            }
            "sendSysEx" | "sendSysex" => script_bytes_from_args(args)
                .map(|bytes| normalize_sysex_bytes(bytes.as_slice()))
                .and_then(|bytes| self.send_bytes(ctx, bytes.as_slice(), "sysex bytes")),
            "sendRawBytes" => script_bytes_from_args(args)
                .and_then(|bytes| self.send_bytes(ctx, bytes.as_slice(), "raw MIDI bytes")),
            _ => return None,
        };

        Some(result)
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id() || !MIDI_MODULE_COMMAND_TYPES.contains(&request.command_type.as_str()) {
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

        if self.clock_bpm_precision.is_bound() && self.clock_bpm_precision.id() == param {
            let _ = self.apply_midi_clock_values(ctx, snapshot);
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

fn midi_message_script_payload(message: &MidiMessage) -> serde_json::Value {
    match message {
        MidiMessage::NoteOff {
            channel,
            note,
            velocity,
        } => serde_json::json!({
            "type": "noteOff",
            "channel": channel,
            "note": note,
            "velocity": velocity,
        }),
        MidiMessage::NoteOn {
            channel,
            note,
            velocity,
        } => serde_json::json!({
            "type": "noteOn",
            "channel": channel,
            "note": note,
            "velocity": velocity,
        }),
        MidiMessage::PolyPressure {
            channel,
            note,
            pressure,
        } => serde_json::json!({
            "type": "polyPressure",
            "channel": channel,
            "note": note,
            "pressure": pressure,
        }),
        MidiMessage::ControlChange {
            channel,
            controller,
            value,
        } => serde_json::json!({
            "type": "controlChange",
            "channel": channel,
            "controller": controller,
            "value": value,
        }),
        MidiMessage::ProgramChange { channel, program } => serde_json::json!({
            "type": "programChange",
            "channel": channel,
            "program": program,
        }),
        MidiMessage::ChannelPressure { channel, pressure } => serde_json::json!({
            "type": "channelPressure",
            "channel": channel,
            "pressure": pressure,
        }),
        MidiMessage::PitchBend { channel, value } => serde_json::json!({
            "type": "pitchBend",
            "channel": channel,
            "value": value,
        }),
        MidiMessage::System(system) => midi_system_message_script_payload(system),
    }
}

fn midi_system_message_script_payload(message: &MidiSystemMessage) -> serde_json::Value {
    match message {
        MidiSystemMessage::TimeCodeQuarterFrame { value } => serde_json::json!({
            "type": "timeCodeQuarterFrame",
            "value": value,
        }),
        MidiSystemMessage::SongPosition { position } => serde_json::json!({
            "type": "songPosition",
            "position": position,
        }),
        MidiSystemMessage::SongSelect { song } => serde_json::json!({
            "type": "songSelect",
            "song": song,
        }),
        MidiSystemMessage::TuneRequest => serde_json::json!({ "type": "tuneRequest" }),
        MidiSystemMessage::TimingClock => serde_json::json!({ "type": "timingClock" }),
        MidiSystemMessage::Start => serde_json::json!({ "type": "start" }),
        MidiSystemMessage::Continue => serde_json::json!({ "type": "continue" }),
        MidiSystemMessage::Stop => serde_json::json!({ "type": "stop" }),
        MidiSystemMessage::ActiveSensing => serde_json::json!({ "type": "activeSensing" }),
        MidiSystemMessage::Reset => serde_json::json!({ "type": "reset" }),
        MidiSystemMessage::Sysex { bytes } => serde_json::json!({
            "type": "sysEx",
            "bytes": crate::app::module::script_api::bytes_arg(bytes.as_slice()),
        }),
    }
}

fn script_channel_arg(args: &[ParamValue], index: usize, _name: &str) -> u8 {
    clamp_channel_i32(script_i32_arg_or(args, index, DEFAULT_SCRIPT_MIDI_CHANNEL))
}

fn script_u7_arg(args: &[ParamValue], index: usize, _name: &str) -> u8 {
    clamp_i32_to_u7(script_i32_arg_or(args, index, 0))
}

fn script_u7_arg_or(args: &[ParamValue], index: usize, fallback: i32) -> u8 {
    clamp_i32_to_u7(script_i32_arg_or(args, index, fallback))
}

fn script_u14_arg(args: &[ParamValue], index: usize, _name: &str) -> u16 {
    clamp_i32_to_u14(script_i32_arg_or(args, index, i32::from(MIDI_PITCH_BEND_CENTER)))
}

fn script_nonnegative_u64_arg_or(args: &[ParamValue], index: usize, fallback: u64) -> u64 {
    u64::try_from(script_i32_arg_or(args, index, i32::try_from(fallback).unwrap_or(i32::MAX)).max(0))
        .unwrap_or(fallback)
}

fn script_i32_arg_or(args: &[ParamValue], index: usize, fallback: i32) -> i32 {
    let Some(value) = args.get(index) else {
        return fallback;
    };
    value
        .as_int()
        .or_else(|| value.as_float().map(|value| value.round() as i32))
        .unwrap_or(fallback)
}

fn script_bytes_from_args(args: &[ParamValue]) -> Result<Vec<u8>, String> {
    if args.len() == 1 {
        if let Some(text) = args[0].as_str() {
            return parse_script_byte_list(text.as_str());
        }
    }

    let mut bytes = Vec::new();
    for value in args {
        if let Some(text) = value.as_str() {
            bytes.extend(parse_script_byte_list(text.as_str())?);
            continue;
        }
        if let Some((x, y)) = value.as_vec2() {
            bytes.push(script_f64_byte(x)?);
            bytes.push(script_f64_byte(y)?);
            continue;
        }
        if let Some((x, y, z)) = value.as_vec3() {
            bytes.push(script_f64_byte(x)?);
            bytes.push(script_f64_byte(y)?);
            bytes.push(script_f64_byte(z)?);
            continue;
        }
        bytes.push(script_i32_byte(script_i32_arg_or(std::slice::from_ref(value), 0, -1))?);
    }
    Ok(bytes)
}

fn parse_script_byte_list(text: &str) -> Result<Vec<u8>, String> {
    text.split(|character: char| character == ',' || character == ';' || character.is_whitespace())
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(parse_script_byte_token)
        .collect()
}

fn parse_script_byte_token(token: &str) -> Result<u8, String> {
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

fn script_f64_byte(value: f64) -> Result<u8, String> {
    script_i32_byte(value.round() as i32)
}

fn script_i32_byte(value: i32) -> Result<u8, String> {
    u8::try_from(value).map_err(|_| format!("MIDI byte {value} is outside the 0-255 range"))
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
        self.base.configure_command_tester(ctx, MIDI_MODULE_COMMAND_TYPES);
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
        let receive_now = self.midi_receive_now(ctx);
        let needs_work = refresh_ports
            || self.input_dirty
            || self.output_dirty
            || self.transport_activity_timeout_due(receive_now)
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
            self.input_dirty = true;
            self.output_dirty = true;
        }

        self.refresh_data_capabilities(ctx);

        if self.input_dirty {
            self.refresh_input(ctx, snapshot);
        }
        if self.output_dirty {
            self.refresh_output(ctx, snapshot);
        }

        self.process_pending_incoming(ctx, snapshot);
        self.refresh_transport_activity(ctx, snapshot);
        self.flush_pending_packets(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_input();
        self.stop_output();
        self.pending_packets.clear();
        self.pending_incoming_messages.clear();
        self.active_notes.clear();
        self.midi_clock_state = MidiClockState::default();
        self.mtc_stream_active = false;
        self.last_mtc_quarter_frame_at = None;
    }

    fn needs_update(&self) -> bool {
        self.input_dirty
            || self.output_dirty
            || !self.pending_incoming_messages.is_empty()
            || !self.pending_packets.is_empty()
            || self.input.is_some()
            || self.output.is_some()
            || self.port_refresh_elapsed >= MIDI_PORT_REFRESH_INTERVAL_SECS
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.port_refresh_due()
            || self.input_dirty
            || self.output_dirty
            || !self.pending_incoming_messages.is_empty()
            || !self.pending_packets.is_empty()
            || self.midi_clock_state.running
            || self.mtc_stream_active
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::periodic(MIDI_MODULE_UPDATE_RATE_HZ)
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            MIDI_SCRIPT_METHODS,
        )
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(result) = self.handle_script_send_method(ctx, method, args) {
            result?;
            return Ok(true);
        }

        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.base
            .emit_script_param_callback(ctx, snapshot, param, &old_value);
        self.on_param_change_inner(ctx, snapshot, param, old_value);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if let Some(enabled) = patch.enabled {
            let _ = ctx;
            if node != self.id() {
                let _ = enabled;
                self.input_dirty = true;
                self.output_dirty = true;
            }
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
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
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

fn midi_transport_connected(
    input_selected: bool,
    input_ready: bool,
    output_selected: bool,
    output_ready: bool,
) -> bool {
    match (input_selected, output_selected) {
        (false, false) => false,
        (true, false) => input_ready,
        (false, true) => output_ready,
        (true, true) => input_ready && output_ready,
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
    let config = midi_cc_config(snapshot, param.id);
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

fn rotary_mode_option(variant_id: &'static str, label: &'static str, ordering: i32) -> ParameterEnumOption {
    ParameterEnumOption {
        variant_id: variant_id.to_string(),
        value: ParamValue::Enum(variant_id.to_string()),
        label: label.to_string(),
        tags: Vec::new(),
        ordering: Some(ordering),
    }
}

fn create_cc_rotary_mode_parameter(value: &'static str) -> Parameter {
    let mut parameter = Parameter::new(
        "Rotary Mode",
        ParamValue::Enum(value.to_string()),
        ParameterChangeCheck::ValueChange,
    );
    apply_midi_value_identity(parameter.node_data_mut(), MIDI_CC_ROTARY_MODE_DECL_ID);
    parameter.constraints.enum_options = vec![
        rotary_mode_option(ROTARY_ABSOLUTE, "Absolute", 0),
        rotary_mode_option(ROTARY_TWOS_COMPLEMENT, "Two's Complement", 1),
        rotary_mode_option(ROTARY_BINARY_OFFSET, "Binary Offset", 2),
        rotary_mode_option(ROTARY_SIGN_MAGNITUDE, "Sign Magnitude", 3),
    ];
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

fn midi_cc_config(snapshot: &ProcessTreeSnapshot, cc_id: NodeId) -> MidiCcConfig {
    let rotary_mechanism = snapshot
        .find_child_by_decl_id(cc_id, MIDI_CC_ROTARY_MODE_DECL_ID)
        .and_then(|mode_id| snapshot.node(mode_id))
        .and_then(|mode| mode.param_value.as_ref())
        .and_then(|value| value.as_enum().or_else(|| value.as_str()))
        .and_then(|mechanism| normalize_rotary_mechanism(mechanism.as_str()))
        .unwrap_or(ROTARY_ABSOLUTE);

    MidiCcConfig { rotary_mechanism }
}

fn normalize_rotary_mechanism(mechanism: &str) -> Option<&'static str> {
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

fn merge_apply_results(current: MidiApplyResult, next: MidiApplyResult) -> MidiApplyResult {
    match (current, next) {
        (MidiApplyResult::Retry, _) | (_, MidiApplyResult::Retry) => MidiApplyResult::Retry,
        (MidiApplyResult::Applied, _) | (_, MidiApplyResult::Applied) => MidiApplyResult::Applied,
        _ => MidiApplyResult::Ignored,
    }
}

fn clamp_usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn quantize_decimal_units(value: f64, decimals: u32) -> i64 {
    let scale = 10_i64.pow(decimals.min(9));
    (value * scale as f64).round() as i64
}

fn quantize_decimal(value: f64, decimals: u32) -> f64 {
    let scale = 10_i64.pow(decimals.min(9)) as f64;
    (value * scale).round() / scale
}

fn stable_midi_clock_bpm(beat_durations: &VecDeque<Duration>) -> Option<f64> {
    if beat_durations.is_empty() {
        return None;
    }

    let mut samples = beat_durations.iter().map(Duration::as_secs_f64).collect::<Vec<_>>();
    samples.sort_by(f64::total_cmp);

    let trimmed = if samples.len() >= 4 {
        &samples[1..samples.len() - 1]
    } else {
        samples.as_slice()
    };
    let average_beat_seconds = trimmed.iter().sum::<f64>() / trimmed.len() as f64;
    if average_beat_seconds <= 0.0 {
        return None;
    }

    Some(60.0 / average_beat_seconds)
}

impl MtcQuarterFrameState {
    fn apply(&mut self, raw_value: u8) {
        let part_index = usize::from((raw_value >> 4) & 0x07);
        self.nibbles[part_index] = Some(raw_value & 0x0F);
    }

    fn frames(&self) -> i32 {
        i32::from(self.nibble(0) | ((self.nibble(1) & 0x01) << 4))
    }

    fn seconds(&self) -> i32 {
        i32::from(self.nibble(2) | ((self.nibble(3) & 0x03) << 4))
    }

    fn minutes(&self) -> i32 {
        i32::from(self.nibble(4) | ((self.nibble(5) & 0x03) << 4))
    }

    fn hours(&self) -> i32 {
        i32::from(self.nibble(6) | ((self.nibble(7) & 0x01) << 4))
    }

    fn rate_code(&self) -> i32 {
        i32::from((self.nibble(7) >> 1) & 0x03)
    }

    fn rate_fps(&self) -> f64 {
        match self.rate_code() {
            0 => 24.0,
            1 => 25.0,
            2 => 29.97,
            3 => 30.0,
            _ => 24.0,
        }
    }

    fn time_seconds(&self) -> f64 {
        f64::from(self.hours()) * 3600.0
            + f64::from(self.minutes()) * 60.0
            + f64::from(self.seconds())
            + (f64::from(self.frames()) / self.rate_fps())
    }

    fn timecode_string(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}:{:02}",
            self.hours(),
            self.minutes(),
            self.seconds(),
            self.frames()
        )
    }

    fn nibble(&self, index: usize) -> u8 {
        self.nibbles.get(index).copied().flatten().unwrap_or(0) & 0x0F
    }
}

fn mtc_rate_label(rate_code: i32) -> &'static str {
    match rate_code {
        0 => "24 fps",
        1 => "25 fps",
        2 => "29.97 drop",
        3 => "30 fps",
        _ => "24 fps",
    }
}

#[cfg(test)]
mod midi_module_tests;
