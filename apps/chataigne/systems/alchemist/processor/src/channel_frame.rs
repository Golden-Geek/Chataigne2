use std::sync::Arc;

use chataigne_alchemist::{ChannelLayout, ValueLaneKey, ValueTypeId};
use golden_values::Value as RuntimeValue;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelValidity {
    Valid,
    MissingSource,
    Disabled,
    Suppressed,
    Invalid,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChannelSlot {
    pub value: Option<RuntimeValue>,
    pub validity: ChannelValidity,
    pub changed: bool,
    pub deliver: bool,
}

impl Default for ChannelSlot {
    fn default() -> Self {
        Self {
            value: None,
            validity: ChannelValidity::MissingSource,
            changed: false,
            deliver: false,
        }
    }
}

/// Runtime values are indexed by an immutable authored layout. Missing and disabled inputs
/// occupy slots, so later channels never shift when a source disappears.
#[derive(Clone, Debug)]
pub struct ChannelFrame {
    layout: Arc<ChannelLayout>,
    slots: Vec<ChannelSlot>,
    logical_tick: u64,
    value_revision: u64,
}

impl ChannelFrame {
    #[must_use]
    pub fn new(layout: Arc<ChannelLayout>) -> Self {
        let slots = vec![ChannelSlot::default(); layout.channels().len()];
        Self {
            layout,
            slots,
            logical_tick: 0,
            value_revision: 0,
        }
    }

    #[must_use]
    pub fn layout(&self) -> &Arc<ChannelLayout> {
        &self.layout
    }

    #[must_use]
    pub fn slots(&self) -> &[ChannelSlot] {
        &self.slots
    }

    #[must_use]
    pub fn logical_tick(&self) -> u64 {
        self.logical_tick
    }

    #[must_use]
    pub fn value_revision(&self) -> u64 {
        self.value_revision
    }

    pub fn begin_tick(&mut self, logical_tick: u64) {
        self.logical_tick = logical_tick;
        for slot in &mut self.slots {
            slot.changed = false;
            slot.deliver = false;
        }
    }

    pub fn set(
        &mut self,
        index: usize,
        value: Option<RuntimeValue>,
        validity: ChannelValidity,
        deliver: bool,
    ) -> Result<(), ChannelFrameError> {
        let descriptor = self
            .layout
            .channels()
            .get(index)
            .ok_or(ChannelFrameError::InvalidIndex(index))?;
        if matches!(validity, ChannelValidity::Valid) && value.is_none() {
            return Err(ChannelFrameError::ValidWithoutValue(descriptor.id.clone()));
        }
        if let (Some(expected), Some(actual)) = (&descriptor.value_type, value.as_ref().map(RuntimeValue::value_type))
            && *expected != actual
        {
            return Err(ChannelFrameError::TypeMismatch {
                channel: descriptor.id.clone(),
                expected: expected.clone(),
                actual,
            });
        }
        let slot = &mut self.slots[index];
        let changed = slot.value != value || slot.validity != validity;
        if changed {
            self.value_revision += 1;
        }
        slot.value = value;
        slot.validity = validity;
        slot.changed = changed;
        slot.deliver = deliver;
        Ok(())
    }

    pub fn with_layout(&self, layout: Arc<ChannelLayout>) -> Self {
        let old_by_id = self
            .layout
            .channels()
            .iter()
            .zip(&self.slots)
            .map(|(channel, slot)| (&channel.id, slot))
            .collect::<std::collections::HashMap<_, _>>();
        let slots = layout
            .channels()
            .iter()
            .map(|channel| {
                old_by_id
                    .get(&channel.id)
                    .filter(|_| {
                        self.layout.index_of(&channel.id).is_some_and(|old_index| {
                            let previous = &self.layout.channels()[old_index];
                            previous.value_type == channel.value_type && previous.provenance == channel.provenance
                        })
                    })
                    .map(|slot| (*slot).clone())
                    .unwrap_or_default()
            })
            .collect();
        Self {
            layout,
            slots,
            logical_tick: self.logical_tick,
            value_revision: self.value_revision,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChannelFrameError {
    #[error("channel index `{0}` is outside the current layout")]
    InvalidIndex(usize),
    #[error("valid channel `{0:?}` has no value")]
    ValidWithoutValue(ValueLaneKey),
    #[error("channel `{channel:?}` expects `{expected}`, got `{actual}`")]
    TypeMismatch {
        channel: ValueLaneKey,
        expected: ValueTypeId,
        actual: ValueTypeId,
    },
}
