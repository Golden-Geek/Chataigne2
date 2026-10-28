use super::super::protocol::{
    ObservationReady, Request, Response, checkpoint_before_sample, validate_observation_finish,
    validate_observation_window,
};
use mapgyver_native_probe::{allocation::Operations, sample_window::Window, workload::ConsumerMode};

fn window() -> Window {
    Window {
        start_qpc: (1 << 60) + 3,
        frequency: 10_000_000,
        warmup_seconds: 30,
        maximum_seconds: 38,
    }
}

fn ready() -> ObservationReady {
    let window = window();
    ObservationReady {
        window,
        consumer_mode: ConsumerMode::Paused,
        completed_qpc: window.start_qpc + window.frequency * 7,
        operations_at_fence: Operations {
            sdk_receive_calls: 12,
            sdk_texture_queries: 10,
            sdk_texture_observations: 8,
            diagnostic_readback_pixels: 32,
            explicit_gpu_copy_calls: 8,
            explicit_gpu_copy_bytes: 128,
            ..Operations::default()
        },
        completion_markers: 6,
        skipped_completion_markers: 0,
    }
}

#[test]
fn startup_mode_and_observation_window_are_explicit_protocol_values() {
    for consumer_mode in [ConsumerMode::Active, ConsumerMode::Paused] {
        let ready = Response::Ready {
            process_id: 123,
            adapter_luid: "selected-adapter".into(),
            consumer_mode,
        };
        let bytes = crate::wire::line(&ready).unwrap();
        match serde_json::from_slice::<Response>(&bytes).unwrap() {
            Response::Ready {
                consumer_mode: observed,
                ..
            } => assert_eq!(observed, consumer_mode),
            _ => panic!("startup response changed kind"),
        }
    }
    for bytes in [
        br#"{"event":"ready","process_id":123,"adapter_luid":"selected-adapter"}"#.as_slice(),
        br#"{"event":"ready","process_id":123,"adapter_luid":"selected-adapter","consumer_mode":"suspended"}"#
            .as_slice(),
    ] {
        assert!(serde_json::from_slice::<Response>(bytes).is_err());
    }
    let request = Request::BeginObservation { window: window() };
    let bytes = super::super::protocol::request_line(&request).unwrap();
    assert!(bytes.len() <= 256);
    match serde_json::from_slice::<Request>(&bytes).unwrap() {
        Request::BeginObservation { window: observed } => assert_eq!(observed, window()),
        _ => panic!("observation request changed kind"),
    }
    assert!(serde_json::from_slice::<Request>(br#"{"command":"begin_frame_age","config":{}}"#).is_err());
}

#[test]
fn readiness_preserves_exact_clock_and_operation_words() {
    let mut expected = ready();
    expected.completion_markers = u32::MAX;
    expected.skipped_completion_markers = u32::MAX;
    let bytes = crate::wire::line(&Response::ObservationReady {
        ready: expected.clone(),
    })
    .unwrap();
    assert!(bytes.len() as u64 <= crate::wire::MAX_LINE_BYTES);
    match serde_json::from_slice::<Response>(&bytes).unwrap() {
        Response::ObservationReady { ready: decoded } => assert!(expected.same_as(&decoded)),
        _ => panic!("readiness response changed kind"),
    }
    let received = expected.completed_qpc + 1;
    for consumer_mode in [ConsumerMode::Active, ConsumerMode::Paused] {
        let mut ready = expected.clone();
        ready.consumer_mode = consumer_mode;
        ready.validate(window(), consumer_mode, received).unwrap();
    }
}

#[test]
fn readiness_marker_counts_are_required_exact_protocol_fields() {
    let encoded = serde_json::to_value(Response::ObservationReady { ready: ready() }).unwrap();
    for field in ["completion_markers", "skipped_completion_markers"] {
        let mut missing = encoded.clone();
        missing["ready"].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Response>(missing).is_err());
        let mut malformed = encoded.clone();
        malformed["ready"][field] = serde_json::Value::Bool(false);
        assert!(serde_json::from_value::<Response>(malformed).is_err());
    }
}

#[test]
fn readiness_rejects_wrong_mode_window_and_clock_boundaries() {
    let expected = ready();
    let received = expected.completed_qpc + 1;
    assert!(expected.validate(window(), ConsumerMode::Active, received).is_err());
    let mut changed = window();
    changed.start_qpc += 1;
    assert!(expected.validate(changed, ConsumerMode::Paused, received).is_err());
    assert!(
        expected
            .validate(window(), ConsumerMode::Paused, expected.completed_qpc - 1)
            .is_err()
    );
    assert!(
        expected
            .validate(window(), ConsumerMode::Paused, window().sample_begin_qpc())
            .is_err()
    );
    for completed_qpc in [window().start_qpc - 1, window().sample_begin_qpc()] {
        let mut ready = expected.clone();
        ready.completed_qpc = completed_qpc;
        assert!(ready.validate(window(), ConsumerMode::Paused, completed_qpc).is_err());
    }
    let mut changed = expected;
    changed.window.warmup_seconds = 0;
    assert!(
        changed
            .validate(changed.window, ConsumerMode::Paused, received)
            .is_err()
    );
}

#[test]
fn paused_window_is_validated_before_collector_creation() {
    validate_observation_window(window(), ConsumerMode::Paused).unwrap();
    let mut immediate = window();
    immediate.warmup_seconds = 0;
    validate_observation_window(immediate, ConsumerMode::Active).unwrap();
    assert!(validate_observation_window(immediate, ConsumerMode::Paused).is_err());
    immediate.frequency = 0;
    assert!(validate_observation_window(immediate, ConsumerMode::Active).is_err());
}

#[test]
fn checkpoint_reads_cannot_cross_a_thirty_second_sample_boundary() {
    let window = window();
    checkpoint_before_sample(Some(window), window.sample_begin_qpc() - 1).unwrap();
    assert!(checkpoint_before_sample(Some(window), window.sample_begin_qpc()).is_err());
    assert!(checkpoint_before_sample(Some(window), window.sample_begin_qpc() + 1).is_err());
    checkpoint_before_sample(None, u64::MAX).unwrap();
    let mut immediate = window;
    immediate.warmup_seconds = 0;
    checkpoint_before_sample(Some(immediate), immediate.sample_begin_qpc()).unwrap();
}

#[test]
fn paused_completion_detects_receive_copy_and_readback_work_after_the_fence() {
    let expected = ready();
    assert!(expected.operations_unchanged(&expected.operations_at_fence));
    let mut changed = expected.clone();
    changed.operations_at_fence.sdk_receive_calls += 1;
    assert!(!expected.same_as(&changed));
    let mut changed = expected.operations_at_fence.clone();
    changed.explicit_gpu_copy_calls += 1;
    assert!(!expected.operations_unchanged(&changed));
    let mut changed = expected.operations_at_fence.clone();
    changed.diagnostic_readback_pixels += 1;
    assert!(!expected.operations_unchanged(&changed));
}

#[test]
fn paused_completion_rejects_resource_markers_even_without_new_texture_work() {
    let expected = ready();
    assert!(expected.resource_activity_unchanged(&expected.operations_at_fence, 6, 0));
    assert!(!expected.resource_activity_unchanged(&expected.operations_at_fence, 7, 0));
    assert!(!expected.resource_activity_unchanged(&expected.operations_at_fence, 6, 1));
    let mut changed = expected.clone();
    changed.completion_markers += 1;
    assert!(!expected.same_as(&changed));
    changed = expected.clone();
    changed.skipped_completion_markers += 1;
    assert!(!expected.same_as(&changed));
}

#[test]
fn paused_finish_cannot_start_cleanup_inside_the_observation_window() {
    let window = window();
    let end_qpc = window.start_qpc + window.frequency * u64::from(window.maximum_seconds);
    assert!(validate_observation_finish(Some(window), ConsumerMode::Paused, Some(end_qpc - 1)).is_err());
    validate_observation_finish(Some(window), ConsumerMode::Paused, Some(end_qpc)).unwrap();
    validate_observation_finish(Some(window), ConsumerMode::Paused, Some(end_qpc + 1)).unwrap();
    validate_observation_finish(Some(window), ConsumerMode::Active, Some(window.start_qpc)).unwrap();
    validate_observation_finish(None, ConsumerMode::Active, None).unwrap();
    for (clock, finish_qpc) in [(Some(window), None), (None, Some(end_qpc)), (None, None)] {
        assert!(validate_observation_finish(clock, ConsumerMode::Paused, finish_qpc).is_err());
    }
    assert!(validate_observation_finish(Some(window), ConsumerMode::Active, Some(window.start_qpc - 1)).is_err());
}
