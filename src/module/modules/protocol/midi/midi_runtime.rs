use std::sync::mpsc::{self, Receiver};

use golden_core::{
    node::{Node, NodeId},
    parameter::{ParamValue, Parameter, ParameterEnumOption},
    process_ctx::ProcessCtx,
};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

pub(crate) const NO_MIDI_PORT_VARIANT: &str = "none";

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MidiInputEvent {
    Message(Vec<u8>),
}

pub(crate) struct MidiInputHandle {
    connection: Option<MidiInputConnection<()>>,
    event_rx: Receiver<MidiInputEvent>,
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
        let connection = input
            .connect(
                &port,
                "chataigne2-midi-input-connection",
                move |_timestamp, message, _| {
                    let _ = event_tx.send(MidiInputEvent::Message(message.to_vec()));
                },
                (),
            )
            .map_err(|error| format!("failed to connect MIDI input {label}: {error}"))?;

        Ok(Self {
            connection: Some(connection),
            event_rx,
        })
    }

    pub(crate) fn try_recv(&self) -> Result<MidiInputEvent, mpsc::TryRecvError> {
        self.event_rx.try_recv()
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

pub(crate) fn sync_midi_port_enum_options(
    ctx: &mut ProcessCtx,
    param_id: NodeId,
    options: Vec<ParameterEnumOption>,
) {
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
            && !next_options
                .iter()
                .any(|option| option.variant_id == current_variant)
        {
            next_options.insert(1, missing_midi_port_option(current_variant.as_str()));
        }

        let next_value = if next_options
            .iter()
            .any(|option| option.variant_id == current_variant)
        {
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
