use super::*;

fn frame(id: u64, duration: u64) -> Frame {
    let mut frame = Frame::new(
        Stamp {
            frame: id,
            qpc: 9_007_199_254_741_000 + id,
        },
        0,
    );
    frame.insert(
        Operation::PollSourceA,
        Span {
            offset_ns: 2,
            elapsed_ns: duration,
        },
    );
    frame.elapsed_ns = duration + 2;
    frame
}

#[test]
fn warmup_and_missing_operations_do_not_become_sampled_zeros() {
    let mut recorder = Recorder::new(10_000_000);
    recorder.record(frame(0, 67_000_000), false);
    recorder.record(frame(1, 500_000), true);
    let report = recorder.finish();
    assert_eq!(report.warmup.outliers.total, 1);
    assert_eq!(report.sampled.frames, 1);
    assert_eq!(report.sampled.outliers.total, 0);
    assert!(report.sampled_by_operation[Operation::Resources as usize].is_none());
    assert_eq!(
        report.sampled_by_operation[Operation::PollSourceA as usize]
            .as_ref()
            .unwrap()
            .p95_ms,
        0.5
    );
}

#[test]
fn a_full_log_counts_omission_and_keeps_the_later_largest_stall() {
    let mut recorder = Recorder::new(10_000_000);
    for id in 0..OUTLIER_CAPACITY as u64 + 3 {
        recorder.record(frame(id, 67_000_000 + id), true);
    }
    let report = recorder.finish();
    assert_eq!(report.sampled.outliers.events.len(), OUTLIER_CAPACITY);
    assert_eq!(report.sampled.outliers.omitted, 3);
    let maximum = report.sampled.outliers.maximum.unwrap();
    assert_eq!(maximum.stamp.frame, OUTLIER_CAPACITY as u64 + 2);
    assert_eq!(maximum.spans[Operation::PollSourceA as usize].unwrap().offset_ns, 2);
    assert!(maximum.stamp.qpc > 9_007_199_254_740_992);
}

#[test]
fn query_gaps_and_late_wakes_are_retained_without_a_slow_operation() {
    let mut recorder = Recorder::new(10_000_000);
    let mut late = frame(0, 1);
    late.scheduled_delay_ns = FRAME_PERIOD_NS;
    recorder.record(late, true);
    let mut skipped = frame(1, 1);
    skipped.gpu_query_skipped = true;
    recorder.record(skipped, true);
    assert_eq!(recorder.finish().sampled.outliers.total, 2);
    let mut events = Outliers::<GpuEvent>::default();
    events.record_gpu(GpuEvent {
        stamp: Stamp { frame: 1, qpc: 10 },
        observed_completed_qpc: 1_000,
        start_ticks: 100,
        end_ticks: 120,
        frequency: 1_000,
    });
    assert_eq!(events.total, 1);
    assert_eq!(events.events[0].stamp.frame, 1);
    assert_eq!(events.events[0].end_ticks - events.events[0].start_ticks, 20);
}

#[test]
fn coalesced_gpu_work_is_a_distinct_event_without_fabricated_query_or_receive_time() {
    let mut recorder = Recorder::new(10_000_000);
    let mut skipped = Frame::new(Stamp { frame: 0, qpc: 10 }, 0);
    skipped.gpu_admission_busy = true;
    recorder.record(skipped, true);
    let report = recorder.finish();
    assert_eq!(report.sampled.frames, 1);
    let event = &report.sampled.outliers.events[0];
    assert!(event.gpu_admission_busy);
    assert!(!event.gpu_query_skipped);
    assert!(report.sampled_by_operation.iter().all(Option::is_none));
}
