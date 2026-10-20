use std::mem::size_of;

use golden_audio::ConfigGeneration;
use golden_core::{node::NodeId, parameter::ParamValue};

use super::super::super::{SoundCardValueBindings, OBSERVATION_EPSILON};

const MAX_ENTRIES: usize = 131_072;
const MAX_BYTES: usize = 16 * 1024 * 1024;
pub(super) const TEXT_BYTES: usize = 64;

/// Prepared once on the native worker and retained by its configuration. Actor
/// writes cannot grow storage or perform a wholesale cache destruction.
pub(crate) struct ValueCache {
    slots: Box<[Slot]>,
    #[cfg(test)]
    drop_thread: Option<std::sync::mpsc::Sender<std::thread::ThreadId>>,
}

struct Slot {
    node: u64,
    applied: Option<(ConfigGeneration, CachedValue)>,
}

#[derive(Clone, Copy)]
enum CachedValue {
    Bool(bool),
    Int(i32),
    Float(f64),
    Text { bytes: [u8; TEXT_BYTES], length: u8 },
}

impl CachedValue {
    fn from_param(value: &ParamValue) -> Option<Self> {
        Some(match value {
            ParamValue::Bool(value) => Self::Bool(*value),
            ParamValue::Int(value) => Self::Int(*value),
            ParamValue::Float(value) => Self::Float(*value),
            ParamValue::Str(value) if value.len() <= TEXT_BYTES => {
                let mut bytes = [0; TEXT_BYTES];
                bytes[..value.len()].copy_from_slice(value.as_bytes());
                Self::Text {
                    bytes,
                    length: value.len() as u8,
                }
            }
            _ => return None,
        })
    }

    fn matches(self, next: &ParamValue) -> bool {
        match (self, next) {
            (Self::Bool(left), ParamValue::Bool(right)) => left == *right,
            (Self::Int(left), ParamValue::Int(right)) => left == *right,
            (Self::Float(left), ParamValue::Float(right)) => (left - right).abs() <= OBSERVATION_EPSILON,
            (Self::Text { bytes, length }, ParamValue::Str(right)) => &bytes[..usize::from(length)] == right.as_bytes(),
            _ => false,
        }
    }
}

impl ValueCache {
    pub(crate) fn prepare(bindings: &SoundCardValueBindings) -> Result<Self, &'static str> {
        let count = bindings.runtime_values.len();
        if count > MAX_ENTRIES
            || count
                .checked_mul(size_of::<Slot>())
                .is_none_or(|bytes| bytes > MAX_BYTES)
        {
            return Err("Sound Card value cache exceeds 131072 slots / 16 MiB");
        }
        let mut slots = bindings
            .runtime_values
            .iter()
            .map(|node| Slot {
                node: node.0,
                applied: None,
            })
            .collect::<Vec<_>>();
        slots.sort_unstable_by_key(|slot| slot.node);
        Ok(Self {
            slots: slots.into_boxed_slice(),
            #[cfg(test)]
            drop_thread: None,
        })
    }

    pub(crate) fn retained_heap_bytes(&self) -> usize {
        self.slots.len() * size_of::<Slot>()
    }

    pub(super) fn matches(&self, node: NodeId, generation: ConfigGeneration, next: &ParamValue) -> bool {
        self.slots
            .binary_search_by_key(&node.0, |slot| slot.node)
            .ok()
            .and_then(|index| self.slots[index].applied)
            .is_some_and(|(epoch, value)| epoch == generation && value.matches(next))
    }

    pub(super) fn store(&mut self, node: NodeId, generation: ConfigGeneration, value: &ParamValue) {
        if let (Ok(index), Some(value)) = (
            self.slots.binary_search_by_key(&node.0, |slot| slot.node),
            CachedValue::from_param(value),
        ) {
            self.slots[index].applied = Some((generation, value));
        }
    }

    #[cfg(test)]
    pub(super) fn applied_count(&self, generation: ConfigGeneration) -> usize {
        self.slots
            .iter()
            .filter(|slot| slot.applied.is_some_and(|(epoch, _)| epoch == generation))
            .count()
    }
}

impl std::fmt::Debug for ValueCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValueCache").field("slots", &self.slots.len()).finish()
    }
}

#[cfg(test)]
impl Drop for ValueCache {
    fn drop(&mut self) {
        if let Some(probe) = self.drop_thread.take() {
            let _ = probe.send(std::thread::current().id());
        }
    }
}

#[cfg(test)]
mod tests;
