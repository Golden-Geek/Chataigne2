use crate::events::{Inbox};

pub enum ExecutionPhase {
    NormalTick,
    EventResolution,
    FlushImmediate
}

pub struct ProcessCtx {
    /// The execution context passed to node behaviour methods (process, update, init, destroy).
    /// Provides read access to the model, safe surfaces for emitting edits, and phase/origin info.
    inbox: Inbox,
    execution_phase: ExecutionPhase,
}
