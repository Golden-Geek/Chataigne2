use super::*;
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPayload, NativeCallbackPreparer};
use golden_io::{PayloadPool, RetainedPayload};
use std::{
    mem::size_of,
    sync::{Arc, LazyLock},
    time::Instant,
};

pub(super) const INPUT_EVENT_BYTES: usize = 16 * 1024 * 1024;
#[derive(Debug)]
pub(crate) struct PreparedInput {
    pub message: MidiMessage,
    pub arrived: Option<Instant>,
    pub clock_timing: Option<MidiClockTiming>,
    pub hex: Option<ParamValue>,
    pub callbacks: Vec<NativeCallbackPayload>,
    pub log: Option<String>,
    pub weight: usize,
}
pub(super) struct InputPreparer {
    values: PayloadPool,
    callbacks: NativeCallbackPreparer,
}
pub(super) struct InputDraft {
    message: Option<MidiMessage>,
    arrived: Option<Instant>,
    clock_timing: Option<MidiClockTiming>,
    hex: Option<ParamValue>,
    callbacks: Vec<NativeCallbackDraft>,
    retained: Vec<NativeCallbackPayload>,
    log: Option<String>,
    own_weight: usize,
    prepared: Option<Arc<PreparedInput>>,
}
impl InputPreparer {
    pub(super) fn new() -> Result<Self, String> {
        static POOL: LazyLock<Result<PayloadPool, std::io::Error>> =
            LazyLock::new(|| PayloadPool::new("midi-input-values", 1024, 64 * 1024 * 1024));
        Ok(Self {
            values: POOL.as_ref().map_err(|e| e.to_string())?.clone(),
            callbacks: NativeCallbackPreparer::new()?,
        })
    }
    pub(super) fn retain(&self, draft: &mut InputDraft) -> Option<RetainedPayload<PreparedInput>> {
        if draft.prepared.is_none() {
            while let Some(callback) = draft.callbacks.get(draft.retained.len()) {
                draft.retained.push(self.callbacks.try_retain(callback)?);
            }
            draft.prepared = Some(Arc::new(PreparedInput {
                message: draft.message.take().expect("one measured input"),
                arrived: draft.arrived,
                clock_timing: draft.clock_timing,
                hex: draft.hex.take(),
                callbacks: std::mem::take(&mut draft.retained),
                log: draft.log.take(),
                weight: draft.own_weight
                    + draft
                        .callbacks
                        .iter()
                        .map(NativeCallbackDraft::retained_bytes)
                        .sum::<usize>(),
            }));
        }
        self.values
            .try_retain_arc(draft.prepared.as_ref()?.clone(), draft.own_weight)
            .ok()
    }
}
impl InputDraft {
    pub(super) fn new(
        message: MidiMessage,
        arrived: Option<Instant>,
        clock_timing: Option<MidiClockTiming>,
        log: bool,
    ) -> Result<Self, String> {
        let bytes = if let MidiMessage::System(MidiSystemMessage::Sysex { bytes }) = &message {
            if bytes.capacity() > 64 * 1024 {
                return Err("MIDI input exceeds 64 KiB retained SysEx storage".into());
            }
            bytes.capacity()
        } else {
            0
        };
        let hex = if let MidiMessage::System(MidiSystemMessage::Sysex { bytes }) = &message {
            Some(ParamValue::Str(format_midi_bytes(bytes)))
        } else {
            None
        };
        let mut callbacks = vec![NativeCallbackDraft::new(
            MIDI_MESSAGE_RECEIVED_CALLBACK,
            vec![midi_message_script_payload(&message)],
        )?];
        let specific = match &message {
            MidiMessage::NoteOn {
                channel,
                note,
                velocity: 0,
            } => Some((
                MIDI_NOTE_OFF_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(note),
                    serde_json::json!(0),
                ],
            )),
            MidiMessage::NoteOn {
                channel,
                note,
                velocity,
            } => Some((
                MIDI_NOTE_ON_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(note),
                    serde_json::json!(velocity),
                ],
            )),
            MidiMessage::NoteOff {
                channel,
                note,
                velocity,
            } => Some((
                MIDI_NOTE_OFF_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(note),
                    serde_json::json!(velocity),
                ],
            )),
            MidiMessage::ControlChange {
                channel,
                controller,
                value,
            } => Some((
                MIDI_CC_RECEIVED_CALLBACK,
                vec![
                    serde_json::json!(channel),
                    serde_json::json!(controller),
                    serde_json::json!(value),
                ],
            )),
            MidiMessage::System(MidiSystemMessage::Sysex { bytes }) => Some((
                MIDI_SYSEX_RECEIVED_CALLBACK,
                vec![crate::app::module::script_api::bytes_arg(bytes)],
            )),
            _ => None,
        };
        if let Some((name, args)) = specific {
            callbacks.reserve_exact(1);
            callbacks.push(NativeCallbackDraft::new(name, args)?);
        }
        let log = log.then(|| match &message {
            MidiMessage::System(MidiSystemMessage::Sysex { bytes }) => {
                format!("Received MIDI SysEx ({} bytes)", bytes.len())
            }
            _ => format!("Received MIDI {}", message_description(&message)),
        });
        let own_weight = size_of::<PreparedInput>()
            + bytes
            + hex.as_ref().map_or(0, ParamValue::retained_bytes)
            + callbacks.len() * size_of::<NativeCallbackPayload>()
            + log.as_ref().map_or(0, String::capacity)
            + 64;
        if own_weight + callbacks.iter().map(NativeCallbackDraft::retained_bytes).sum::<usize>()
            > INPUT_EVENT_BYTES - 256
        {
            return Err("MIDI prepared input exceeds 16 MiB".into());
        }
        let retained = Vec::with_capacity(callbacks.len());
        Ok(Self {
            message: Some(message),
            arrived,
            clock_timing,
            hex,
            callbacks,
            retained,
            log,
            own_weight,
            prepared: None,
        })
    }
    #[cfg(test)]
    pub(super) fn prepared(message: MidiMessage) -> RetainedPayload<PreparedInput> {
        InputPreparer::new()
            .unwrap()
            .retain(&mut Self::new(message, None, None, false).unwrap())
            .expect("test input capacity")
    }
}

#[cfg(test)]
mod tests;
