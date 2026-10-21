use super::midi_message::decode_midi_message;
use golden_io::{BoundedWorker, PendingDrainState, RetirementPool, WorkerContext, WorkerLimits, WorkerNotifier};
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, Instant},
};

mod clock;
mod discovery;
mod input;
mod native;
mod output;
mod ports;
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use discovery::PreparedCatalog;
pub(crate) use discovery::{CatalogEvent, MidiDiscovery, PortCatalog};
pub(crate) use input::MidiInputHandle;
pub(crate) use output::{MidiOutputEvent, MidiOutputHandle};
pub(crate) use ports::{format_midi_bytes, midi_port_selected};

pub(crate) const NO_MIDI_PORT_VARIANT: &str = "none";
const MESSAGE_BYTES: usize = 64 * 1024;
const TURN: usize = 32;

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
#[derive(Clone, Debug)]
pub(crate) enum MidiInputEvent {
    Ready,
    Error(String),
    Rejected(u64),
    Message(golden_io::RetainedPayload<super::midi_input::PreparedInput>),
    UnsupportedMessage(String),
}
impl MidiInputEvent {
    fn weight(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Message(input) => input.weight,
                Self::UnsupportedMessage(error) | Self::Error(error) => error.capacity(),
                _ => 0,
            }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MidiClockTiming {
    pub beat_triggered: bool,
    pub bpm: Option<f64>,
}

fn lifetimes() -> &'static RetirementPool {
    static POOL: OnceLock<RetirementPool> = OnceLock::new();
    POOL.get_or_init(|| RetirementPool::new(NonZeroUsize::new(32).unwrap()))
}
fn validate_selection(selection: &str) -> Result<(), String> {
    if selection.len() > 1024 {
        Err("MIDI selection exceeds 1024 bytes".into())
    } else if !midi_port_selected(selection) {
        Err("MIDI port is not selected".into())
    } else {
        Ok(())
    }
}
fn diagnostic(error: String) -> String {
    if error.capacity() > 4096 {
        "MIDI native diagnostic exceeds 4096 bytes".into()
    } else {
        error
    }
}

struct Readiness {
    state: Arc<AtomicBool>,
    notifier: WorkerNotifier,
}
impl Drop for Readiness {
    fn drop(&mut self) {
        self.state.store(false, Ordering::Release);
        self.notifier.notify();
    }
}
fn publish<C, E>(ctx: &WorkerContext<C, E>, event: E, weight: usize) -> bool {
    ctx.publish_weighted(event, weight).is_ok()
}
