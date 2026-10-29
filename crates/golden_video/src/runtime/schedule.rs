use golden_time::{MonotonicTime, RationalRate, TimeSnapshot};
use uuid::Uuid;

use super::CompletionToken;

/// One scheduler opportunity, independent of backend execution and presentation clocks.
#[derive(Clone, Copy, Debug)]
pub struct FrameSchedule {
    /// Distinct cadence lifetime; resets never reuse phase identity.
    pub cadence: Uuid,
    /// Monotonic origin for this absolute rational schedule.
    pub anchor: MonotonicTime,
    /// Exact normalized cadence rate.
    pub rate: RationalRate,
    /// Absolute slot ordinal, including skipped opportunities.
    pub frame_index: u64,
    /// Start of this opportunity.
    pub slot_start: MonotonicTime,
    /// Next slot boundary, used for the software submission deadline observation.
    pub slot_end: MonotonicTime,
    /// Clock authority observation used to consume this slot or close its coverage.
    pub observed: TimeSnapshot,
    /// Slots coalesced immediately before this consumed opportunity.
    pub coalesced_slots: u64,
    /// Index gap since the previous submitted opportunity; first submission uses its index.
    pub skipped_slots: u64,
}

/// Software admission outcome, without implying presentation or GPU completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameAdmission {
    /// The backend established this submitted device/batch token.
    Submitted(CompletionToken),
    /// Existing in-flight work prevented a new batch.
    Busy,
    /// No current demanded output could be submitted.
    NoDemand,
    /// A backend/core failure rejected this opportunity.
    Rejected,
    /// Final observed coverage of this cadence; the index is its next unconsumed slot.
    Fence,
}
