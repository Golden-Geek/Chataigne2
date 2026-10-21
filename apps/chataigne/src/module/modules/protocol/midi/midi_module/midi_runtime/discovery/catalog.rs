use super::super::*;
use golden_core::parameter::{ParamValue, ParameterEnumOption, ParameterEnumOptions};
use golden_io::PayloadPool;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Selections {
    pub input: String,
    pub output: String,
}
impl Selections {
    pub fn new(input: &str, output: &str) -> Result<Self, String> {
        if input.len() > 1024 || output.len() > 1024 {
            return Err("MIDI catalog selection exceeds 1024 bytes".into());
        }
        Ok(Self {
            input: input.into(),
            output: output.into(),
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PortCatalog {
    pub selection: String,
    pub options: ParameterEnumOptions,
}
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PreparedCatalog {
    pub input: PortCatalog,
    pub output: PortCatalog,
}
impl PreparedCatalog {
    pub(super) fn prepare(
        raw: MidiPortOptions,
        selections: &Selections,
        previous: Option<&Self>,
        pool: &PayloadPool,
    ) -> Result<Self, String> {
        // Native inventory admission precedes enum allocation or conversion.
        for ports in [&raw.inputs, &raw.outputs] {
            if ports.capacity() > 64
                || ports
                    .iter()
                    .any(|p| p.label.capacity() > 512 || p.variant_id.capacity() > 1024 || p.details.capacity() > 128)
            {
                return Err("MIDI catalog exceeds 64 ports or its metadata field bounds".into());
            }
        }
        Ok(Self {
            input: prepare(
                raw.inputs,
                "No Input",
                &selections.input,
                previous.map(|p| &p.input),
                pool,
            )?,
            output: prepare(
                raw.outputs,
                "No Output",
                &selections.output,
                previous.map(|p| &p.output),
                pool,
            )?,
        })
    }
    pub(super) fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + [&self.input, &self.output]
                .into_iter()
                .map(|p| p.selection.capacity() + p.options.retained_bytes())
                .sum::<usize>()
    }
}
fn prepare(
    ports: Vec<DiscoveredMidiPort>,
    label: &str,
    selection: &str,
    previous: Option<&PortCatalog>,
    pool: &PayloadPool,
) -> Result<PortCatalog, String> {
    // Exact capacity includes the None row and a possible missing-selection row.
    let mut options = Vec::with_capacity(ports.len() + 2);
    options.push(ParameterEnumOption {
        variant_id: NO_MIDI_PORT_VARIANT.into(),
        value: ParamValue::Enum(NO_MIDI_PORT_VARIANT.into()),
        label: label.into(),
        tags: vec![],
        ordering: Some(0),
    });
    let mut variants = std::collections::HashSet::new();
    variants.insert(NO_MIDI_PORT_VARIANT.to_string());
    for (index, port) in ports.into_iter().enumerate() {
        if !variants.insert(port.variant_id.clone()) {
            return Err("MIDI catalog contains duplicate variants".into());
        }
        options.push(ParameterEnumOption {
            value: ParamValue::Enum(port.variant_id.clone()),
            variant_id: port.variant_id,
            label: port.label,
            tags: vec![],
            ordering: Some(10 + index as i32),
        });
    }
    if !variants.contains(selection) {
        options.push(ParameterEnumOption {
            variant_id: selection.into(),
            value: ParamValue::Enum(selection.into()),
            label: format!(
                "Missing: {}",
                selection.split_once('|').map_or(selection, |(_, name)| name)
            ),
            tags: vec!["missing".into()],
            ordering: Some(5),
        });
    }
    let mut options: ParameterEnumOptions = options.into();
    if options.len() > 66 || options.retained_bytes() > 256 * 1024 {
        return Err("MIDI prepared catalog exceeds 66 options / 256 KiB".into());
    }
    if let Some(previous) = previous.filter(|p| p.options == options) {
        options = previous.options.clone();
    } else {
        pool.try_retain_arc(options.shared().clone(), options.retained_bytes())
            .map_err(|_| "MIDI catalog lifetime capacity exhausted".to_string())?;
    }
    Ok(PortCatalog {
        selection: selection.into(),
        options,
    })
}
