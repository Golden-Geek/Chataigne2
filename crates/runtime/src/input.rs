use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

use golden_values::Value;

use crate::{DirtySet, InputRoutingTable, InputSlot, RuntimeArenas};

/// Delivery semantics declared by a production module/input adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputDelivery {
    /// Continuous state: only the newest monotonic value per slot is required.
    LatestValue,
    /// Trigger, edge, or command: every admitted update is ordered and lossless.
    LosslessOrdered,
}

/// One parsed and timestamped module input ready for direct-slot routing.
#[derive(Clone, Debug, PartialEq)]
pub struct RuntimeInputUpdate {
    /// Dense generation input slot.
    pub slot: InputSlot,
    /// Canonical typed input value.
    pub value: Value,
    /// Source monotonic timestamp assigned outside semantic execution.
    pub source_time_ns: u64,
    /// Monotonic source revision used to reject stale continuous values.
    pub revision: u64,
    /// Explicit delivery semantics.
    pub delivery: InputDelivery,
}

/// Fixed-capacity input ingress configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputIngressConfig {
    /// Stable generation input-slot count.
    pub input_count: usize,
    /// Maximum admitted lossless events before producers receive backpressure.
    pub lossless_capacity: usize,
}

impl InputIngressConfig {
    /// Validates an ingress configuration.
    pub fn validate(self) -> Result<Self, InputIngressError> {
        if self.lossless_capacity == 0 {
            return Err(InputIngressError::ZeroLosslessCapacity);
        }
        Ok(self)
    }
}

struct InputQueues {
    latest: Vec<Option<RuntimeInputUpdate>>,
    last_revision: Vec<Option<u64>>,
    lossless: VecDeque<RuntimeInputUpdate>,
    lossless_capacity: usize,
}

/// Shared bounded ingress mailbox between module/IO producers and one semantic owner.
pub struct RuntimeInputMailbox {
    queues: Mutex<InputQueues>,
}

impl RuntimeInputMailbox {
    /// Creates a mailbox and its cloneable producer handle.
    pub fn new(config: InputIngressConfig) -> Result<(Arc<Self>, RuntimeInputHandle), InputIngressError> {
        let config = config.validate()?;
        let mailbox = Arc::new(Self {
            queues: Mutex::new(InputQueues {
                latest: (0..config.input_count).map(|_| None).collect(),
                last_revision: vec![None; config.input_count],
                lossless: VecDeque::with_capacity(config.lossless_capacity),
                lossless_capacity: config.lossless_capacity,
            }),
        });
        let handle = RuntimeInputHandle {
            mailbox: mailbox.clone(),
        };
        Ok((mailbox, handle))
    }

    /// Applies admitted inputs directly to dense slots and marks dependency routes dirty.
    ///
    /// `scratch` is caller-owned and reused so the semantic hot path does not allocate in
    /// proportion to the project size.
    pub fn drain_into(
        &self,
        arenas: &mut RuntimeArenas,
        routes: &InputRoutingTable,
        dirty: &mut DirtySet,
        scratch: &mut Vec<RuntimeInputUpdate>,
    ) -> Result<usize, InputIngressError> {
        scratch.clear();
        {
            let mut queues = self.queues.lock().map_err(|_| InputIngressError::Disconnected)?;
            for update in queues.latest.iter_mut().filter_map(Option::take) {
                scratch.push(update);
            }
            scratch.extend(queues.lossless.drain(..));
        }
        for update in scratch.iter() {
            arenas
                .set_input(update.slot, update.value.clone())
                .map_err(|_| InputIngressError::SlotOutOfBounds)?;
            for route in routes.routes_for(update.slot) {
                dirty
                    .mark(route.dependent)
                    .map_err(|_| InputIngressError::DependencyOutOfBounds)?;
            }
        }
        Ok(scratch.len())
    }
}

/// Cloneable production module/input adapter handle.
#[derive(Clone)]
pub struct RuntimeInputHandle {
    mailbox: Arc<RuntimeInputMailbox>,
}

impl RuntimeInputHandle {
    /// Publishes one parsed/timestamped input under its declared delivery policy.
    pub fn publish(&self, update: RuntimeInputUpdate) -> Result<(), InputIngressError> {
        let mut queues = self
            .mailbox
            .queues
            .lock()
            .map_err(|_| InputIngressError::Disconnected)?;
        let index = update.slot.index();
        if index >= queues.latest.len() {
            return Err(InputIngressError::SlotOutOfBounds);
        }
        match update.delivery {
            InputDelivery::LatestValue => {
                if queues.last_revision[index].is_some_and(|revision| revision >= update.revision) {
                    return Err(InputIngressError::StaleRevision);
                }
                queues.last_revision[index] = Some(update.revision);
                queues.latest[index] = Some(update);
            }
            InputDelivery::LosslessOrdered => {
                if queues.lossless.len() >= queues.lossless_capacity {
                    return Err(InputIngressError::LosslessBackpressure);
                }
                queues.lossless.push_back(update);
            }
        }
        Ok(())
    }
}

/// Explicit module-input admission or routing failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputIngressError {
    /// Lossless capacity must be nonzero.
    ZeroLosslessCapacity,
    /// Input slot is absent from the current generation.
    SlotOutOfBounds,
    /// A continuous update is older than the latest admitted source revision.
    StaleRevision,
    /// Lossless queue is full and the producer must apply its declared backpressure policy.
    LosslessBackpressure,
    /// A compiled dependency references missing work.
    DependencyOutOfBounds,
    /// The input mailbox is no longer usable.
    Disconnected,
}

impl fmt::Display for InputIngressError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLosslessCapacity => formatter.write_str("lossless input capacity must be nonzero"),
            Self::SlotOutOfBounds => formatter.write_str("runtime input slot is out of bounds"),
            Self::StaleRevision => formatter.write_str("runtime input revision is stale"),
            Self::LosslessBackpressure => formatter.write_str("lossless runtime input queue is full"),
            Self::DependencyOutOfBounds => formatter.write_str("runtime input dependency is out of bounds"),
            Self::Disconnected => formatter.write_str("runtime input mailbox is unavailable"),
        }
    }
}

impl std::error::Error for InputIngressError {}
