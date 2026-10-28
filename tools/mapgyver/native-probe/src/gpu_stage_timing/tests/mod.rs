use super::*;

fn frame() -> Frame {
    Frame {
        frame: 0,
        submitted_qpc: 100,
        observed_completed_qpc: 150,
        frequency: 10_000_000,
        collected: true,
        boundaries: [10, 15, 20, 20, 27, 30, 42],
    }
}

#[test]
fn stage_differences_telescope_without_converting_clocks() {
    let mut row = frame();
    let origin = (1_u64 << 53) + 1;
    row.boundaries = [
        origin,
        origin + 1,
        origin + 5,
        origin + 5,
        origin + 10,
        origin + 12,
        origin + 13,
    ];
    let differences = row
        .boundaries
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect::<Vec<_>>();
    assert_eq!(differences, [1, 4, 0, 5, 2, 1]);
    assert_eq!(
        differences.iter().sum::<u64>(),
        row.boundaries[BOUNDARY_COUNT - 1] - row.boundaries[0]
    );
    let mut trace = Trace::default();
    trace.record(row.clone()).unwrap();
    assert_eq!(trace.rows, [row]);
}

#[test]
fn invalid_rows_leave_the_retained_prefix_and_counters_unchanged() {
    let mut trace = Trace::default();
    trace.record(frame()).unwrap();
    let mut invalid = Vec::new();
    let mut zero_frequency = frame();
    zero_frequency.frequency = 0;
    invalid.push(zero_frequency);
    let mut no_submission = frame();
    no_submission.submitted_qpc = 0;
    invalid.push(no_submission);
    let mut early_observation = frame();
    early_observation.observed_completed_qpc = early_observation.submitted_qpc - 1;
    invalid.push(early_observation);
    for index in 0..BOUNDARY_COUNT - 1 {
        let mut unordered = frame();
        unordered.boundaries = [10; BOUNDARY_COUNT];
        unordered.boundaries[index] = 11;
        invalid.push(unordered);
    }
    for row in invalid {
        assert!(trace.record(row).is_err());
        assert_eq!(trace.rows, [frame()]);
        assert_eq!(trace.invalid, 0);
        assert_eq!(trace.skipped, 0);
        assert_eq!(trace.pending, 0);
        assert_eq!(trace.truncated, 0);
    }
}

#[test]
fn equal_boundaries_and_equal_submission_observation_are_valid() {
    let mut row = frame();
    row.observed_completed_qpc = row.submitted_qpc;
    row.boundaries = [u64::MAX; BOUNDARY_COUNT];
    let mut trace = Trace::default();
    trace.record(row.clone()).unwrap();
    assert_eq!(trace.rows, [row]);
}

#[test]
fn raw_clock_values_and_collection_identity_remain_exact_in_json() {
    let mut row = frame();
    row.frame = (1_u64 << 53) + 1;
    row.submitted_qpc = u64::MAX - 1;
    row.observed_completed_qpc = u64::MAX;
    row.frequency = (1_u64 << 53) + 3;
    row.collected = false;
    let mut trace = Trace::default();
    trace.record(row.clone()).unwrap();
    let value = serde_json::to_value(&trace).unwrap();
    let encoded = &value["rows"][0];
    assert_eq!(encoded["frame"].as_u64(), Some(row.frame));
    assert_eq!(encoded["submitted_qpc"].as_u64(), Some(row.submitted_qpc));
    assert_eq!(
        encoded["observed_completed_qpc"].as_u64(),
        Some(row.observed_completed_qpc)
    );
    assert_eq!(encoded["frequency"].as_u64(), Some(row.frequency));
    assert_eq!(encoded["collected"].as_bool(), Some(false));
    assert_eq!(encoded["boundaries"], serde_json::to_value(row.boundaries).unwrap());
    assert_eq!(value["stage_names"], serde_json::to_value(STAGE_NAMES).unwrap());
}

#[test]
fn capacity_preserves_the_prefix_and_counts_later_valid_rows() {
    let mut trace = Trace::default();
    for index in 0..MAX_SAMPLES + 2 {
        let mut row = frame();
        row.frame = index as u64;
        trace.record(row).unwrap();
    }
    assert_eq!(trace.rows.len(), MAX_SAMPLES);
    assert_eq!(trace.rows.first().unwrap().frame, 0);
    assert_eq!(trace.rows.last().unwrap().frame, (MAX_SAMPLES - 1) as u64);
    assert_eq!(trace.truncated, 2);
    assert_eq!(trace.invalid, 0);
    let mut invalid = frame();
    invalid.frequency = 0;
    assert!(trace.record(invalid).is_err());
    assert_eq!(trace.rows.len(), MAX_SAMPLES);
    assert_eq!(trace.truncated, 2);
}

#[test]
fn boundary_indices_and_segment_names_are_stable() {
    assert_eq!(INTERMEDIATE_COUNT, 5);
    assert_eq!(BOUNDARY_COUNT, 7);
    let boundaries = [
        Boundary::Sources,
        Boundary::Composition,
        Boundary::Spout,
        Boundary::NativeOutput,
        Boundary::Editor,
    ];
    for (index, boundary) in boundaries.into_iter().enumerate() {
        assert_eq!(boundary.index(), index);
    }
    assert_eq!(
        Trace::default().stage_names,
        [
            "source_handoff",
            "composition",
            "spout_send",
            "native_output",
            "editor",
            "tail"
        ]
    );
}
