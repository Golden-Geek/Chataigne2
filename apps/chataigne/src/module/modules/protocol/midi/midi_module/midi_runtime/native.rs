use super::{
    input::{InputConnection, InputSink},
    output::OutputConnection,
    ports::*,
    *,
};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

pub(super) struct NativeInput {
    _connection: MidiInputConnection<()>,
    selection: String,
}
impl InputConnection for NativeInput {
    fn is_present(&mut self) -> Result<bool, String> {
        midi_input_port_available(&self.selection)
    }
}
pub(super) fn open_input(config: &MidiInputConfig, sink: InputSink) -> Result<NativeInput, String> {
    let mut input =
        MidiInput::new("chataigne2-midi-input").map_err(|e| format!("failed to initialize MIDI input: {e}"))?;
    input.ignore(Ignore::None);
    let (port, label) =
        resolve_input_port(input.ports(), &input, &config.port_variant)?.ok_or("MIDI input port is not selected")?;
    let connection = input
        .connect(
            &port,
            "chataigne2-midi-input-connection",
            move |_, bytes, _| {
                sink.push(bytes);
            },
            (),
        )
        .map_err(|e| format!("failed to connect MIDI input {label}: {e}"))?;
    Ok(NativeInput {
        _connection: connection,
        selection: config.port_variant.clone(),
    })
}

pub(super) struct NativeOutput {
    connection: MidiOutputConnection,
    selection: String,
}
impl OutputConnection for NativeOutput {
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.connection
            .send(bytes)
            .map_err(|e| format!("failed to send MIDI bytes: {e}"))
    }
    fn is_present(&mut self) -> Result<bool, String> {
        midi_output_port_available(&self.selection)
    }
}
pub(super) fn open_output(config: &MidiOutputConfig) -> Result<NativeOutput, String> {
    let output =
        MidiOutput::new("chataigne2-midi-output").map_err(|e| format!("failed to initialize MIDI output: {e}"))?;
    let (port, label) = resolve_output_port(output.ports(), &output, &config.port_variant)?
        .ok_or("MIDI output port is not selected")?;
    let connection = output
        .connect(&port, "chataigne2-midi-output-connection")
        .map_err(|e| format!("failed to connect MIDI output {label}: {e}"))?;
    Ok(NativeOutput {
        connection,
        selection: config.port_variant.clone(),
    })
}
