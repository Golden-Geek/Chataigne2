use std::fmt;
use std::sync::Arc;

use crate::{EffectRoutingTable, EffectSlot, RuntimeMetrics};

/// Effect staged into a generation-assigned slot by semantic execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedEffect<T> {
    /// Effect payload routed to the app or module I/O boundary.
    pub payload: T,
}

/// Single authoritative external effect boundary.
pub trait EffectSink<T> {
    /// Dispatch failure.
    type Error;

    /// Dispatches one effect in compile-time deterministic order.
    fn dispatch(&mut self, effect: T) -> Result<(), Self::Error>;
}

impl<T, E, F> EffectSink<T> for F
where
    F: FnMut(T) -> Result<(), E>,
{
    type Error = E;

    fn dispatch(&mut self, effect: T) -> Result<(), Self::Error> {
        self(effect)
    }
}

/// Whether staged effects may leave the semantic runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectCommitMode {
    /// Dispatch through the sole production output adapter.
    Authoritative,
    /// Observe semantics while suppressing every external effect.
    ShadowSuppressed,
}

/// Effect commit counts for diagnostics and parity evidence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EffectCommitReport {
    /// Effects dispatched externally.
    pub committed: usize,
    /// Effects suppressed in shadow mode.
    pub suppressed: usize,
}

/// Fixed staging slots reused across ticks.
pub struct EffectBuffer<T> {
    slots: Vec<Option<StagedEffect<T>>>,
    metrics: Arc<RuntimeMetrics>,
}

impl<T> EffectBuffer<T> {
    /// Allocates one reusable buffer for a generation.
    pub fn new(slot_count: usize, metrics: Arc<RuntimeMetrics>) -> Self {
        Self {
            slots: (0..slot_count).map(|_| None).collect(),
            metrics,
        }
    }

    /// Stages an effect without growing the buffer.
    pub fn stage(&mut self, slot: EffectSlot, payload: T) -> Result<(), EffectBufferError> {
        let target = self
            .slots
            .get_mut(slot.index())
            .ok_or(EffectBufferError::SlotOutOfBounds)?;
        if target.is_some() {
            return Err(EffectBufferError::SlotAlreadyOccupied);
        }
        *target = Some(StagedEffect { payload });
        Ok(())
    }

    /// Commits staged effects in the generation's precompiled order.
    pub fn commit<S>(
        &mut self,
        routes: &EffectRoutingTable,
        mode: EffectCommitMode,
        sink: &mut S,
    ) -> Result<EffectCommitReport, S::Error>
    where
        S: EffectSink<T>,
    {
        let mut report = EffectCommitReport::default();
        for route in routes.routes.iter() {
            let Some(effect) = self.slots[route.slot.index()].take() else {
                continue;
            };
            match mode {
                EffectCommitMode::Authoritative => {
                    sink.dispatch(effect.payload)?;
                    report.committed += 1;
                }
                EffectCommitMode::ShadowSuppressed => report.suppressed += 1,
            }
        }
        self.metrics.effects_finished(report.committed, report.suppressed);
        Ok(report)
    }

    /// Clears uncommitted effects before generation replacement or rollback.
    pub fn clear(&mut self) {
        self.slots.iter_mut().for_each(|slot| *slot = None);
    }
}

/// Invalid effect staging operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectBufferError {
    /// Slot is absent from the current generation.
    SlotOutOfBounds,
    /// Two workers attempted to write the same compile-assigned slot.
    SlotAlreadyOccupied,
}

impl fmt::Display for EffectBufferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SlotOutOfBounds => formatter.write_str("effect slot is out of bounds"),
            Self::SlotAlreadyOccupied => formatter.write_str("effect slot is already occupied"),
        }
    }
}

impl std::error::Error for EffectBufferError {}
