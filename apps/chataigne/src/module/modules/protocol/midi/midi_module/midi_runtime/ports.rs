use super::*;
use midir::{MidiInput, MidiOutput};

#[cfg(test)]
mod tests;

pub(crate) fn available_midi_port_options() -> Result<MidiPortOptions, String> {
    Ok(MidiPortOptions {
        inputs: available_input_ports()?,
        outputs: available_output_ports()?,
    })
}

pub(super) fn midi_input_port_available(selection: &str) -> Result<bool, String> {
    midi_port_available(selection, available_input_ports)
}

pub(super) fn midi_output_port_available(selection: &str) -> Result<bool, String> {
    midi_port_available(selection, available_output_ports)
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

    Ok(discover_ports()?
        .into_iter()
        .any(|port| port.variant_id == selection || port.label == selection))
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
    if ports.capacity() > 64 {
        return Err("MIDI discovery exceeds 64 retained ports".into());
    }
    let mut discovered = Vec::with_capacity(ports.len());
    for (index, port) in ports.iter().enumerate() {
        let label = port_name(port).map_err(|error| format!("failed to read MIDI port name: {error}"))?;
        if label.capacity() > 512 {
            return Err("MIDI port name exceeds 512 bytes".into());
        }
        discovered.push(DiscoveredMidiPort {
            variant_id: midi_port_variant_id(index, label.as_str()),
            label,
            details: format!("index={index}"),
        });
    }

    discovered.sort_by(|left, right| left.label.cmp(&right.label));
    Ok(discovered)
}

pub(super) fn resolve_input_port(
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

pub(super) fn resolve_output_port(
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

    if ports.capacity() > 64 {
        return Err("MIDI port inventory exceeds 64 retained ports".into());
    }
    let mut fallback_by_name = None;
    for (index, port) in ports.into_iter().enumerate() {
        let label = port_name(index, &port)?;
        if label.capacity() > 512 {
            return Err("MIDI port name exceeds 512 bytes".into());
        }
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

fn midi_port_variant_id(index: usize, label: &str) -> String {
    format!("{index}|{label}")
}

fn human_midi_port_variant(variant_id: &str) -> String {
    variant_id
        .split_once('|')
        .map(|(_, label)| label.to_string())
        .unwrap_or_else(|| variant_id.to_string())
}
