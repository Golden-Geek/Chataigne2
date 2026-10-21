use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

mod catalog_delivery;
mod events;
mod feedback;
mod helpers;
mod incoming;
mod input_delivery;
mod midi_input;
pub(crate) mod midi_message;
mod midi_output;
pub(crate) mod midi_runtime;
mod outgoing;
mod script_values;
mod service;
mod structure;
mod timing;
mod transport;
use helpers::*;
use script_values::*;

use golden_core::{
    edit::Edit,
    engine::NodeExecutionRule,
    events::Event,
    logerror, node,
    node::{
        DeclId, Folder, Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor, NodeUserPermissions,
    },
    parameter::{
        Enum, ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, ParameterEnumOption,
        ParameterEventBehaviour, RangeConstraint,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use self::{
    midi_message::{
        cc_decl_id, cc_label, cc_supports_14_bit, channel_decl_id, channel_folder_label, clamp_channel_i32,
        clamp_i32_to_u14, clamp_i32_to_u7, decode_rotary_delta, encode_14_bit_control_change, encode_rotary_delta,
        message_description, note_decl_id, note_label, MidiMessage, MidiSystemMessage, MIDI_DATA_MAX,
        MIDI_PITCH_BEND_CENTER, MIDI_U14_MAX, ROTARY_ABSOLUTE, ROTARY_BINARY_OFFSET, ROTARY_SIGN_MAGNITUDE,
        ROTARY_TWOS_COMPLEMENT,
    },
    midi_runtime::{
        format_midi_bytes, midi_port_selected, MidiClockTiming, MidiDiscovery, MidiInputConfig, MidiInputEvent,
        MidiInputHandle, MidiOutputConfig, MidiOutputEvent, MidiOutputHandle, NO_MIDI_PORT_VARIANT,
    },
};

use crate::app::MidiSendRequest;

const MIDI_COMPILED_KERNEL: &str = "chataigne.runtime.midi";
const MIDI_INPUT_WARNING_ID: &str = "midi_input_connection";
const MIDI_OUTPUT_WARNING_ID: &str = "midi_output_connection";
const MIDI_PORT_OPTIONS_WARNING_ID: &str = "midi_port_options";

const NOTES_FOLDER_DECL_ID: &str = "notes";
const CONTROL_CHANGE_FOLDER_DECL_ID: &str = "control_change";
const POLY_PRESSURE_FOLDER_DECL_ID: &str = "poly_pressure";
const SYSTEM_FOLDER_DECL_ID: &str = "system";
const SYSEX_FOLDER_DECL_ID: &str = "sysex";

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
const MIDI_CLOCK_PLAYING_TIMEOUT: Duration = Duration::from_millis(500);
const MTC_PLAYING_TIMEOUT: Duration = Duration::from_millis(250);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MidiApplyResult {
    Applied,
    Retry,
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
    service: service::ServiceState,
    input: Option<MidiInputHandle>,
    output: Option<MidiOutputHandle>,
    retiring_output: Option<MidiOutputHandle>,
    discovery: Option<MidiDiscovery>,
    pending_catalog: Option<catalog_delivery::CatalogDelivery>,
    last_input_config: Option<MidiInputConfig>,
    last_output_config: Option<MidiOutputConfig>,
    input_dirty: bool,
    output_dirty: bool,
    pending_incoming_messages: VecDeque<golden_io::RetainedPayload<midi_input::PreparedInput>>,
    incoming: Option<input_delivery::InputDelivery>,
    incoming_callbacks: VecDeque<input_delivery::InputDelivery>,
    cc_14_bit_state: [[Cc14BitState; 32]; 16],
    active_notes: HashSet<(u8, u8)>,
    last_note_on: Option<LastNoteInfo>,
    midi_clock_state: MidiClockState,
    mtc_state: MtcQuarterFrameState,
    mtc_stream_active: bool,
    last_mtc_quarter_frame_at: Option<Duration>,
    structure_work: structure::StructureWork,
}

impl MidiModule {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            service::ServiceState::default(),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            true,
            true,
            VecDeque::new(),
            None,
            VecDeque::new(),
            [[Cc14BitState::default(); 32]; 16],
            HashSet::new(),
            None,
            MidiClockState::default(),
            MtcQuarterFrameState::default(),
            false,
            None,
            structure::StructureWork::default(),
        )
    }

    #[cfg(test)]
    pub(crate) fn enqueue_incoming_message_for_test(&mut self, message: MidiMessage) {
        self.pending_incoming_messages
            .push_back(midi_input::InputDraft::prepared(message));
        if let Some(wake) = &self.service.wake {
            wake.notifier.notify();
        }
    }

    #[cfg(test)]
    pub(crate) fn auto_add_enabled_for_test(&self) -> bool {
        self.auto_add.get()
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
        self.base.configure_command_tester(ctx, MIDI_MODULE_COMMAND_TYPES);
        crate::app::module::enable_module_authoring(self.node_data_mut());
        self.input_dirty = true;
        self.output_dirty = true;
        self.refresh_data_capabilities(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        self.start_value_locks(ctx, snapshot);
        if let Err(error) = self.register_service(ctx) {
            ctx.set_node_warning_with(self.id(), Some("midi_service"), error, None);
        }
        self.request_service(ctx);
    }

    fn destroy(&mut self, _ctx: &mut ProcessCtx) {
        self.stop_input();
        self.stop_output();
        self.discovery = None;
        self.pending_catalog = None;
        self.structure_work.cancel();
        self.service = service::ServiceState::default();
        self.pending_incoming_messages.clear();
        self.incoming = None;
        self.incoming_callbacks.clear();
        self.active_notes.clear();
        self.midi_clock_state = MidiClockState::default();
        self.mtc_stream_active = false;
        self.last_mtc_quarter_frame_at = None;
    }

    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> NodeExecutionRule {
        NodeExecutionRule::passive().with_compiled_kernel(MIDI_COMPILED_KERNEL)
    }
    fn on_worker_wake(&mut self, ctx: &mut ProcessCtx) {
        self.service(ctx);
    }
    fn worker_wake_requires_tree_snapshot(&self) -> bool {
        self.service_requires_snapshot()
    }
    fn worker_wake_when_disabled(&self) -> bool {
        self.structure_work.busy()
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(self.node_data(), self.get_type(), MIDI_SCRIPT_METHODS)
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
        self.base.emit_script_param_callback(ctx, snapshot, param, &old_value);
        self.on_param_change_inner(ctx, snapshot, param, old_value);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        if patch.enabled.is_some() && node != self.id() {
            self.input_dirty = true;
            self.output_dirty = true;
            self.service.input_retry = None;
            self.service.output_retry = None;
            self.request_service(ctx);
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        if enabled {
            self.input_dirty = true;
            self.output_dirty = true;
            self.service.input_retry = None;
            self.service.output_retry = None;
            self.service.discovery_retry = None;
            self.request_service(ctx);
        } else {
            if let Some(wake) = &self.service.wake {
                wake.cancel_deadline();
            }
            self.service.input_retry = None;
            self.service.output_retry = None;
            self.service.discovery_retry = None;
            self.stop_input();
            self.stop_output();
            self.discovery = None;
            self.pending_catalog = None;
            self.pending_incoming_messages.clear();
            self.incoming = None;
            self.incoming_callbacks.clear();
            self.active_notes.clear();
            self.midi_clock_state = MidiClockState::default();
            self.mtc_stream_active = false;
            self.last_mtc_quarter_frame_at = None;
            self.last_input_config = None;
            self.last_output_config = None;
            self.clear_input_warning(ctx);
            self.clear_output_warning(ctx);
            self.base.set_connected(ctx, false);
            self.input_dirty = false;
            self.output_dirty = false;
            self.request_structure_wake(ctx);
        }
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[cfg(test)]
mod tests;
