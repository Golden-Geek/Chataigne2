//! Callback-safe ownership and control primitives.
//!
//! All callback-facing methods are bounded and avoid allocation, deallocation, locks, logging,
//! formatting, and blocking waits. Constructors and reclamation methods are control-thread APIs.

mod gain_mailbox;
mod guard;
mod ownership;
mod plan_exchange;
mod pressure;
#[cfg(all(feature = "analysis", feature = "playback"))]
mod priority;

pub use gain_mailbox::{
    GainMailboxTarget, OrderedRealtimeControlReader, OrderedRealtimeControlWriter, RealtimeBarrier,
    RealtimeBarrierKind, RealtimeControlUpdate, ordered_realtime_controls,
};
pub use guard::{RealtimeScope, assert_not_realtime, is_realtime_thread};
pub use ownership::{
    AnalysisCaptureError, AnalysisFrame, AnalysisFrameReader, AnalysisFrameTag, AnalysisFrameWriter,
    AnalysisRecycleError, AnalysisWriterRetirement, PreparedVoice, RealtimeVoiceRetirement, RealtimeVoiceSlots,
    RetiredVoice, VoiceRetirementReason, VoiceSlotController, analysis_frame_pool, voice_slot_pool,
};
pub use plan_exchange::{
    PlanPublishError, PlanSwapResult, RealtimePlanRetirement, RealtimePlanSlot, RealtimePlanSlotMetrics,
    RenderPlanPublisher, acknowledged_plan_exchange,
};
pub use pressure::{QueuePressureCounters, QueuePressureSnapshot};
#[cfg(all(feature = "analysis", feature = "playback"))]
pub(crate) use priority::AudioThreadPriorityGuard;

#[cfg(test)]
mod tests;
