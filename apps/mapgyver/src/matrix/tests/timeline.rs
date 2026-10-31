use super::super::timeline::{MAX_MIX_COLUMNS, Timeline};
use super::*;
use golden_time::{MonotonicTime, PlaybackRate, ShowPosition, TimeAuthority, TimeSnapshot};

struct Clock;
impl golden_time::Clock for Clock {
    fn now(&self) -> Result<MonotonicTime, golden_time::TimeError> {
        Ok(MonotonicTime::from_nanos(0))
    }
}
fn anchor() -> TimeSnapshot {
    TimeAuthority::new(Clock, ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap()
        .sample()
        .unwrap()
}
fn at(anchor: TimeSnapshot, millis: u64) -> TimeSnapshot {
    anchor
        .at_monotonic(MonotonicTime::from_nanos(millis * 1_000_000))
        .unwrap()
}
fn desired(column: ColumnId, duration_ms: u32) -> MatrixPlaybackSnapshot {
    MatrixPlaybackSnapshot {
        desired_column: Some(column),
        launch_id: Some(uuid::Uuid::new_v4()),
        transition: TransitionDefinition {
            duration_ms,
            curve: TransitionCurve::Linear,
        },
    }
}
fn column() -> ColumnId {
    ColumnId(golden_core::model::NodeUuid(uuid::Uuid::new_v4()))
}

#[test]
fn interruption_samples_current_weights_with_one_shared_anchor_and_drops_only_completed_outgoing_columns() {
    let time = anchor();
    let [a, b, c] = [column(), column(), column()];
    let mut timeline = Timeline::default();
    timeline.frame(&desired(a, 0), Some(time), 0).unwrap();
    let incoming = desired(b, 1000);
    let first = timeline.frame(&incoming, Some(time), 0).unwrap().unwrap();
    assert_eq!(
        first
            .clips
            .iter()
            .map(|entry| (entry.clip.column, entry.from, entry.to))
            .collect::<Vec<_>>(),
        [(a, 1.0, 0.0), (b, 0.0, 1.0)]
    );
    let incoming = desired(c, 1000);
    let interrupted = timeline.frame(&incoming, Some(at(time, 500)), 0).unwrap().unwrap();
    assert_eq!(
        interrupted
            .clips
            .iter()
            .map(|entry| (entry.clip.column, entry.from, entry.to))
            .collect::<Vec<_>>(),
        [(a, 0.5, 0.0), (b, 0.5, 0.0), (c, 0.0, 1.0)]
    );
    let interval = interrupted.interval.unwrap();
    let midpoint = interrupted
        .clips
        .iter()
        .map(|entry| interval.envelope(entry.from, entry.to).sample(at(time, 1000)).get())
        .collect::<Vec<_>>();
    assert_eq!(midpoint, [0.25, 0.25, 0.5]);
    assert_eq!(
        timeline
            .frame(&incoming, Some(at(time, 1499)), 0)
            .unwrap()
            .unwrap()
            .clips
            .len(),
        3
    );
    let completed = timeline.frame(&incoming, Some(at(time, 1500)), 0).unwrap().unwrap();
    assert_eq!(
        completed
            .clips
            .iter()
            .map(|entry| (entry.clip.column, entry.from, entry.to))
            .collect::<Vec<_>>(),
        [(c, 1.0, 1.0)]
    );
    assert!(completed.interval.is_none());
}

#[test]
fn interrupted_mix_admission_is_fixed_and_cut_or_completion_recovers_without_history_or_queue_growth() {
    let time = anchor();
    let mut timeline = Timeline::default();
    timeline.frame(&desired(column(), 0), Some(time), 0).unwrap();
    let mut last = None;
    for index in 1..MAX_MIX_COLUMNS {
        let request = desired(column(), 1000);
        assert!(timeline.accepts(request.desired_column.unwrap(), 0));
        timeline.frame(&request, Some(at(time, index as u64)), 0).unwrap();
        last = Some(request);
    }
    let overflow = desired(column(), 1000);
    assert!(!timeline.accepts(overflow.desired_column.unwrap(), 0));
    assert!(timeline.frame(&overflow, Some(at(time, 40)), 0).is_err());
    assert_eq!(
        timeline
            .frame(last.as_ref().unwrap(), Some(at(time, 40)), 0)
            .unwrap()
            .unwrap()
            .clips
            .len(),
        MAX_MIX_COLUMNS
    );
    timeline.settle(at(time, 2000));
    assert!(timeline.accepts(overflow.desired_column.unwrap(), 0));
    timeline.frame(&overflow, Some(at(time, 2000)), 0).unwrap();
    let cut = desired(column(), 0);
    let frame = timeline.frame(&cut, None, 0).unwrap().unwrap();
    assert_eq!(
        frame
            .clips
            .iter()
            .map(|entry| (entry.clip.column, entry.from, entry.to))
            .collect::<Vec<_>>(),
        [(cut.desired_column.unwrap(), 1.0, 1.0)]
    );
    assert!(frame.interval.is_none());
}
