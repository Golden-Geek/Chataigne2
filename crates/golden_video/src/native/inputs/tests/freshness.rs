use spout2_sys as sdk;

use super::super::receiver::{Freshness, Received, classify};
use crate::plan::SpoutFrameCount;

#[test]
fn unknown_foreign_counters_never_suppress_successful_copies() {
    let mut freshness = Freshness::new(SpoutFrameCount::Unversioned);
    for _ in 0..3 {
        assert!(!freshness.unchanged(Some(712)));
        freshness.transferred(Received::Frame, Some(712));
        assert_eq!(freshness.last_copied(), None);
    }
    assert!(!freshness.unchanged(None));
    assert!(!freshness.unchanged(Some(0)));
}

#[test]
fn only_a_successful_copy_establishes_versioned_unchanged_preflight() {
    let mut freshness = Freshness::new(SpoutFrameCount::VersionedV1);
    assert!(!freshness.unchanged(Some(12)));
    freshness.transferred(Received::Frame, Some(12));
    assert!(freshness.unchanged(Some(12)));
    for outcome in [Received::Busy, Received::Unchanged] {
        freshness.transferred(outcome, Some(13));
        assert_eq!(freshness.last_copied(), Some(12));
        assert!(freshness.unchanged(Some(12)));
        assert!(!freshness.unchanged(Some(13)));
    }
    freshness.transferred(Received::Frame, None);
    assert!(
        !freshness.unchanged(Some(12)),
        "missing per-copy availability cannot reuse a counter"
    );
}

#[test]
fn counter_reset_zero_wrap_and_endpoint_changes_require_receive() {
    let mut freshness = Freshness::new(SpoutFrameCount::VersionedV1);
    freshness.transferred(Received::Frame, Some(i32::MAX));
    for observed in [None, Some(0), Some(-1), Some(1)] {
        assert!(!freshness.unchanged(observed));
    }
    freshness.transferred(Received::Frame, Some(0));
    assert_eq!(freshness.last_copied(), Some(0));
    assert!(!freshness.unchanged(Some(0)));
    freshness.transferred(Received::Frame, Some(1));
    assert!(freshness.unchanged(Some(1)));
    freshness.reset();
    assert!(
        !freshness.unchanged(Some(1)),
        "new sender handle/descriptor cannot inherit history"
    );
    for outcome in [Received::Waiting, Received::Absent, Received::Mismatch] {
        freshness.transferred(Received::Frame, Some(1));
        freshness.transferred(outcome, None);
        assert_eq!(freshness.last_copied(), None);
    }
}

#[test]
fn detailed_receive_copied_busy_and_unchanged_are_distinct() {
    assert_eq!(classify(sdk::SPOUT_DX_TRANSFER_COPIED).unwrap(), Received::Frame);
    assert_eq!(classify(sdk::SPOUT_DX_TRANSFER_BUSY).unwrap(), Received::Busy);
    assert_eq!(classify(sdk::SPOUT_DX_TRANSFER_UNCHANGED).unwrap(), Received::Unchanged);
    assert_eq!(classify(sdk::SPOUT_DX_TRANSFER_UPDATED).unwrap(), Received::Waiting);
    assert_eq!(classify(sdk::SPOUT_DX_TRANSFER_DISCONNECTED).unwrap(), Received::Absent);
    assert!(classify(sdk::SPOUT_DX_TRANSFER_FAILED).is_err());
}
