//! Bounded handoff from a renderer to a CPU preview consumer.

use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};

use crate::runtime::PlanIdentity;

/// A CPU consumer with room for one completed frame awaiting pickup.
#[derive(Clone, Debug)]
pub struct ReadbackSink(SyncSender<ReadbackFrame>);

/// CPU-owned image with the publication fence of the GPU pass that produced it.
pub struct ReadbackFrame {
    /// Plan that populated staging, which may differ from the current active plan.
    pub plan: PlanIdentity,
    /// Tightly packed top-down opaque sRGB BGRA8 rows.
    pub pixels: Vec<u8>,
}

/// Creates an owned BGRA8 frame handoff. A slow receiver cannot enqueue more frames.
pub fn readback_channel() -> (ReadbackSink, Receiver<ReadbackFrame>) {
    let (sender, receiver) = sync_channel(1);
    (ReadbackSink(sender), receiver)
}

impl ReadbackSink {
    /// Offers a completed frame without blocking or losing ownership on failure.
    pub fn try_send(&self, frame: ReadbackFrame) -> Result<(), TrySendError<ReadbackFrame>> {
        self.0.try_send(frame)
    }
}
