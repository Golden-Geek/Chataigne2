use std::{
    collections::VecDeque,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

use golden_core::{
    node::{Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterEnumOption},
    process_ctx::ProcessCtx,
};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use super::midi_message::{MidiMessage, decode_midi_message};

pub(crate) const NO_MIDI_PORT_VARIANT: &str = "none";
const MIDI_CLOCK_STATUS: u8 = 0xF8;
const MIDI_CLOCK_PULSES_PER_BEAT: u8 = 24;
const MIDI_CLOCK_BPM_WINDOW_BEATS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveredMidiPort {
    pub variant_id: String,
    pub label: String,
    pub details: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MidiPortOptions {
    pub inputs: Vec<DiscoveredMidiPort>,
    pub outputs: Vec<DiscoveredMidiPort>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MidiInputConfig {
    pub port_variant: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MidiOutputConfig {
    pub port_variant: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MidiInputEvent {
    Message {
        bytes: Vec<u8>,
        message: MidiMessage,
        received_at: Duration,
        clock_timing: Option<MidiClockTiming>,
    },
    UnsupportedMessage {
        bytes: Vec<u8>,
        received_at: Duration,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MidiClockTiming {
    pub beat_triggered: bool,
    pub bpm: Option<f64>,
}

#[derive(Clone, Debug, Default)]
struct MidiClockAnalyzer {
    tick_in_beat: u8,
    last_beat_at: Option<Duration>,
    recent_beat_durations: VecDeque<Duration>,
}

pub(crate) struct MidiInputHandle {
    connection: Option<MidiInputConnection<()>>,
    event_rx: Receiver<MidiInputEvent>,
    time_origin: Instant,
}

pub(crate) struct MidiOutputHandle {
    connection: MidiOutputConnection,
}

impl MidiInputHandle {
    pub(crate) fn spawn(config: MidiInputConfig) -> Result<Self, String> {
        let mut input = MidiInput::new("chataigne2-midi-input")
            .map_err(|error| format!("failed to initialize MIDI input: {error}"))?;
        input.ignore(Ignore::None);

        let (port, label) = resolve_input_port(input.ports(), &input, config.port_variant.as_str())?
            .ok_or_else(|| "MIDI input port is not selected".to_string())?;
        let (event_tx, event_rx) = mpsc::channel();
        let callback_origin = Instant::now();
        let mut clock_analyzer = MidiClockAnalyzer::default();
        let connection = input
            .connect(
                &port,
                "chataigne2-midi-input-connection",
                move |_timestamp, message, _| {
                    let bytes = message.to_vec();
                    let received_at = callback_origin.elapsed();
                    let clock_timing = clock_analyzer.observe(bytes.as_slice(), received_at);
                    let event = match decode_midi_message(bytes.as_slice()) {
                        Some(message) => MidiInputEvent::Message {
                            bytes,
                            message,
                            received_at,
                            clock_timing,
                        },
                        None => MidiInputEvent::UnsupportedMessage { bytes, received_at },
                    };
                    let _ = event_tx.send(event);
                },
                (),
            )
            .map_err(|error| format!("failed to connect MIDI input {label}: {error}"))?;

        Ok(Self {
            connection: Some(connection),
            event_rx,
            time_origin: callback_origin,
        })
    }

    pub(crate) fn try_recv(&self) -> Result<MidiInputEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
    }

    pub(crate) fn elapsed(&self) -> std::time::Duration {
        self.time_origin.elapsed()
    }

    pub(crate) fn stop(&mut self) {
        self.connection = None;
    }
}

impl Drop for MidiInputHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

impl MidiClockAnalyzer {
    fn observe(&mut self, bytes: &[u8], received_at: Duration) -> Option<MidiClockTiming> {
        if bytes != [MIDI_CLOCK_STATUS] {
            return None;
        }

        self.tick_in_beat = self.tick_in_beat.saturating_add(1);
        if self.tick_in_beat < MIDI_CLOCK_PULSES_PER_BEAT {
            return Some(MidiClockTiming {
                beat_triggered: false,
                bpm: None,
            });
        }

        self.tick_in_beat = 0;
        let mut bpm = None;
        if let Some(last_beat_at) = self.last_beat_at {
            let beat_duration = received_at.saturating_sub(last_beat_at);
            if !beat_duration.is_zero() {
                self.recent_beat_durations.push_back(beat_duration);
                while self.recent_beat_durations.len() > MIDI_CLOCK_BPM_WINDOW_BEATS {
                    self.recent_beat_durations.pop_front();
                }
                bpm = stable_midi_clock_bpm(&self.recent_beat_durations);
            }
        }
        self.last_beat_at = Some(received_at);

        Some(MidiClockTiming {
            beat_triggered: true,
            bpm,
        })
    }
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

impl MidiOutputHandle {
    pub(crate) fn spawn(config: MidiOutputConfig) -> Result<Self, String> {
        let output = MidiOutput::new("chataigne2-midi-output")
            .map_err(|error| format!("failed to initialize MIDI output: {error}"))?;
        let (port, label) = resolve_output_port(output.ports(), &output, config.port_variant.as_str())?
            .ok_or_else(|| "MIDI output port is not selected".to_string())?;
        let connection = output
            .connect(&port, "chataigne2-midi-output-connection")
            .map_err(|error| format!("failed to connect MIDI output {label}: {error}"))?;

        Ok(Self { connection })
    }

    pub(crate) fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.connection
            .send(bytes)
            .map_err(|error| format!("failed to send MIDI bytes: {error}"))
    }
}

pub(crate) fn available_midi_port_options() -> Result<MidiPortOptions, String> {
    Ok(MidiPortOptions {
        inputs: available_input_ports()?,
        outputs: available_output_ports()?,
    })
}

pub(crate) fn midi_input_port_options(ports: &[DiscoveredMidiPort]) -> Vec<ParameterEnumOption> {
    midi_port_options("No Input", ports)
}

pub(crate) fn midi_output_port_options(ports: &[DiscoveredMidiPort]) -> Vec<ParameterEnumOption> {
    midi_port_options("No Output", ports)
}

pub(crate) fn midi_input_port_available(selection: &str) -> Result<bool, String> {
    midi_port_available(selection, available_input_ports)
}

pub(crate) fn midi_output_port_available(selection: &str) -> Result<bool, String> {
    midi_port_available(selection, available_output_ports)
}

pub(crate) fn sync_midi_port_enum_options(ctx: &mut ProcessCtx, param_id: NodeId, options: Vec<ParameterEnumOption>) {
    ctx.call_node_mutation(param_id, move |node, inner_ctx| {
        let Some(parameter) = node.as_any_mut().downcast_mut::<Parameter>() else {
            return Err("MIDI port target is not a parameter".to_string());
        };

        let mut next_options = options.clone();
        let current_variant = parameter
            .value
            .as_enum()
            .filter(|variant| !variant.trim().is_empty())
            .unwrap_or_else(|| NO_MIDI_PORT_VARIANT.to_string());

        if current_variant != NO_MIDI_PORT_VARIANT
            && !next_options.iter().any(|option| option.variant_id == current_variant)
        {
            next_options.insert(1, missing_midi_port_option(current_variant.as_str()));
        }

        let next_value = if next_options.iter().any(|option| option.variant_id == current_variant) {
            ParamValue::Enum(current_variant.clone())
        } else {
            ParamValue::Enum(NO_MIDI_PORT_VARIANT.to_string())
        };

        if parameter.constraints.enum_options == next_options && parameter.value == next_value {
            return Ok(());
        }

        let label = parameter.node_data().meta.label.clone();
        let change_check = parameter.change_check.clone();
        let mut replacement = Parameter::new(label.as_str(), next_value, change_check);
        *replacement.node_data_mut() = parameter.node_data().clone();
        replacement.default_value = parameter.default_value.clone();
        replacement.event_behaviour = parameter.event_behaviour;
        replacement.read_only = parameter.read_only;
        replacement.constraints = parameter.constraints.clone();
        replacement.constraints.enum_options = next_options;
        replacement.ui_hints = parameter.ui_hints.clone();
        replacement.control = parameter.control.clone();
        replacement.control_modes_enabled = parameter.control_modes_enabled;

        inner_ctx.replace_node(param_id, replacement);

        Ok(())
    });
}

pub(crate) fn midi_port_selected(variant_id: &str) -> bool {
    let trimmed = variant_id.trim();
    !trimmed.is_empty() && trimmed != NO_MIDI_PORT_VARIANT
}

fn midi_port_available<F>(selection: &str, discover_ports: F) -> Result<bool, String>
where
    F: FnOnce() -> Result<Vec<DiscoveredMidiPort>, String>,
{
    if !midi_port_selected(selection) {
        return Ok(false);
    }

    let selection_label = human_midi_port_variant(selection);
    Ok(discover_ports()?
        .into_iter()
        .any(|port| port.variant_id == selection || port.label == selection_label))
}

pub(crate) fn format_midi_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "<empty>".to_string();
    }

    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn available_input_ports() -> Result<Vec<DiscoveredMidiPort>, String> {
    let input = MidiInput::new("chataigne2-midi-input-discovery")
        .map_err(|error| format!("failed to enumerate MIDI input ports: {error}"))?;
    describe_ports(input.ports(), |port| input.port_name(port))
}

fn available_output_ports() -> Result<Vec<DiscoveredMidiPort>, String> {
    let output = MidiOutput::new("chataigne2-midi-output-discovery")
        .map_err(|error| format!("failed to enumerate MIDI output ports: {error}"))?;
    describe_ports(output.ports(), |port| output.port_name(port))
}

fn describe_ports<P, F, E>(ports: Vec<P>, mut port_name: F) -> Result<Vec<DiscoveredMidiPort>, String>
where
    F: FnMut(&P) -> Result<String, E>,
    E: std::fmt::Display,
{
    let mut discovered = Vec::with_capacity(ports.len());
    for (index, port) in ports.iter().enumerate() {
        let label = port_name(port).map_err(|error| format!("failed to read MIDI port name: {error}"))?;
        discovered.push(DiscoveredMidiPort {
            variant_id: midi_port_variant_id(index, label.as_str()),
            label,
            details: format!("index={index}"),
        });
    }

    discovered.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(discovered)
}

fn resolve_input_port(
    ports: Vec<midir::MidiInputPort>,
    input: &MidiInput,
    selection: &str,
) -> Result<Option<(midir::MidiInputPort, String)>, String> {
    resolve_port(ports, selection, |index, port| {
        input
            .port_name(port)
            .map_err(|error| format!("failed to read MIDI input port {index}: {error}"))
    })
}

fn resolve_output_port(
    ports: Vec<midir::MidiOutputPort>,
    output: &MidiOutput,
    selection: &str,
) -> Result<Option<(midir::MidiOutputPort, String)>, String> {
    resolve_port(ports, selection, |index, port| {
        output
            .port_name(port)
            .map_err(|error| format!("failed to read MIDI output port {index}: {error}"))
    })
}

fn resolve_port<P, F>(ports: Vec<P>, selection: &str, mut port_name: F) -> Result<Option<(P, String)>, String>
where
    F: FnMut(usize, &P) -> Result<String, String>,
{
    if !midi_port_selected(selection) {
        return Ok(None);
    }

    let mut fallback_by_name = None;
    for (index, port) in ports.into_iter().enumerate() {
        let label = port_name(index, &port)?;
        if midi_port_variant_id(index, label.as_str()) == selection {
            return Ok(Some((port, label)));
        }

        if fallback_by_name.is_none() && label == selection {
            fallback_by_name = Some((port, label));
        }
    }

    if let Some(found) = fallback_by_name {
        return Ok(Some(found));
    }

    Err(format!(
        "selected MIDI port '{}' is not available",
        human_midi_port_variant(selection)
    ))
}

fn midi_port_options(no_port_label: &str, ports: &[DiscoveredMidiPort]) -> Vec<ParameterEnumOption> {
    let mut options = vec![ParameterEnumOption {
        variant_id: NO_MIDI_PORT_VARIANT.to_string(),
        value: ParamValue::Enum(NO_MIDI_PORT_VARIANT.to_string()),
        label: no_port_label.to_string(),
        tags: vec![],
        ordering: Some(0),
    }];

    options.extend(ports.iter().enumerate().map(|(index, port)| ParameterEnumOption {
        variant_id: port.variant_id.clone(),
        value: ParamValue::Enum(port.variant_id.clone()),
        label: port.label.clone(),
        tags: vec![],
        ordering: Some(10 + index as i32),
    }));

    options
}

fn missing_midi_port_option(port_variant: &str) -> ParameterEnumOption {
    let variant_id = port_variant.to_string();
    ParameterEnumOption {
        variant_id: variant_id.clone(),
        value: ParamValue::Enum(variant_id.clone()),
        label: format!("Missing: {}", human_midi_port_variant(variant_id.as_str())),
        tags: vec!["missing".to_string()],
        ordering: Some(5),
    }
}

fn midi_port_variant_id(index: usize, label: &str) -> String {
    format!("{index}|{label}")
}

fn human_midi_port_variant(variant_id: &str) -> String {
    variant_id
        .split_once('|')
        .map(|(_, label)| label.to_string())
        .unwrap_or_else(|| variant_id.to_string())
}
