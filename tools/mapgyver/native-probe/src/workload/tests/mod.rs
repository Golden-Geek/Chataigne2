use super::{GenerationEnd, PresentationPolicy, RunSpec, Scenario};

#[test]
fn presentation_comparison_parses_only_explicit_cli_choices_and_records_policy() {
    assert_eq!(
        "latest".parse::<PresentationPolicy>().unwrap(),
        PresentationPolicy::Latest
    );
    assert_eq!(
        "output-vblank".parse::<PresentationPolicy>().unwrap(),
        PresentationPolicy::OutputVblank
    );
    for invalid in ["", "vblank", "output_vblank", "OutputVblank", "1", "latest "] {
        assert!(
            invalid.parse::<PresentationPolicy>().is_err(),
            "unexpected policy {invalid:?}"
        );
    }
    let spec = RunSpec::new(Scenario::Q02, 120, 30)
        .unwrap()
        .with_presentation_policy(PresentationPolicy::OutputVblank)
        .unwrap();
    let report = serde_json::to_value(spec).unwrap();
    assert_eq!(report["presentation_policy"], "output_vblank");
    assert_eq!(report["scenario"], "q02");
    assert_eq!(report["sample_seconds"], 120);
    assert_eq!(report["warmup_seconds"], 30);
    assert_eq!(spec.total_seconds(), 150);
    assert_eq!(spec.scenario.extent(), [3840, 2160]);
    assert!(spec.validate_generation_end(0, GenerationEnd::DeadlineReached).is_ok());
    assert!(spec.validate_generation_end(0, GenerationEnd::WindowClosed).is_err());
}

#[test]
fn lifecycle_and_editor_exercises_retain_the_baseline_and_cannot_select_comparison_policy() {
    for spec in [
        RunSpec::new(Scenario::Diagnostic, 8, 0).unwrap(),
        RunSpec::recreation(Scenario::Q02, 2).unwrap(),
        RunSpec::editor_isolation(Scenario::Q01).unwrap(),
    ] {
        assert_eq!(spec.presentation_policy(), PresentationPolicy::Latest);
        assert_eq!(serde_json::to_value(spec).unwrap()["presentation_policy"], "latest");
        assert!(spec.with_presentation_policy(PresentationPolicy::OutputVblank).is_err());
    }
    let baseline = RunSpec::new(Scenario::Q02, 120, 30).unwrap();
    assert_eq!(baseline.presentation_policy(), PresentationPolicy::Latest);
    assert_eq!(serde_json::to_value(baseline).unwrap()["presentation_policy"], "latest");
    let comparison = baseline
        .with_presentation_policy(PresentationPolicy::OutputVblank)
        .unwrap();
    let restored = comparison.with_presentation_policy(PresentationPolicy::Latest).unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(baseline).unwrap()
    );
}

#[test]
fn editor_isolation_cannot_finish_by_timeout_close_or_missing_restore() {
    let spec = RunSpec::editor_isolation(Scenario::Q01).unwrap();
    assert!(spec.isolates_editor());
    assert_eq!(spec.total_seconds(), 185);
    assert!(
        spec.validate_generation_end(0, GenerationEnd::EditorIsolationComplete)
            .is_ok()
    );
    for end in [
        GenerationEnd::DeadlineReached,
        GenerationEnd::WindowClosed,
        GenerationEnd::FinalGenerationComplete,
    ] {
        assert!(spec.validate_generation_end(0, end).is_err());
    }
    assert!(RunSpec::editor_isolation(Scenario::Diagnostic).is_err());
    assert!(
        RunSpec::new(Scenario::Q01, 8, 0)
            .unwrap()
            .validate_generation_end(0, GenerationEnd::EditorIsolationComplete)
            .is_err()
    );
}

#[test]
fn duration_admission_checks_sum_before_allocating_or_starting_children() {
    assert!(RunSpec::new(Scenario::Q01, u32::MAX, 1).is_err());
    assert!(RunSpec::new(Scenario::Q02, 3590, 30).is_err());
    assert!(RunSpec::new(Scenario::Q01, 7, 30).is_err());
    let spec = RunSpec::new(Scenario::Q02, 120, 30).unwrap();
    assert_eq!(spec.total_seconds(), 150);
    assert_eq!(spec.scenario.extent(), [3840, 2160]);
}

#[test]
fn recreation_workload_bounds_generations_and_leaves_recovery_time() {
    assert!(RunSpec::recreation(Scenario::Q01, 0).is_err());
    assert!(RunSpec::recreation(Scenario::Q02, 101).is_err());
    assert!(RunSpec::recreation(Scenario::Diagnostic, 1).is_err());
    let spec = RunSpec::recreation(Scenario::Q02, 100).unwrap();
    assert_eq!(spec.device_recreations(), 100);
    assert_eq!(spec.total_seconds(), 418);
    assert_eq!(spec.settling_seconds(), 10);
    assert_eq!(RunSpec::new(Scenario::Q01, 120, 30).unwrap().settling_seconds(), 0);
}

#[test]
fn closing_after_lifecycle_checks_cannot_complete_a_timed_qualification() {
    let timed = RunSpec::new(Scenario::Q01, 120, 2).unwrap();
    assert!(timed.validate_generation_end(0, GenerationEnd::WindowClosed).is_err());
    assert!(timed.validate_generation_end(0, GenerationEnd::DeadlineReached).is_ok());
    assert!(
        timed
            .validate_generation_end(0, GenerationEnd::FinalGenerationComplete)
            .is_err()
    );
    let interactive = RunSpec::new(Scenario::Diagnostic, 120, 0).unwrap();
    assert!(
        interactive
            .validate_generation_end(0, GenerationEnd::WindowClosed)
            .is_ok()
    );
}

#[test]
fn acknowledgement_of_the_last_device_does_not_make_watchdog_expiry_a_success() {
    let recreation = RunSpec::recreation(Scenario::Q02, 2).unwrap();
    // All three scopes may exist and the final marker may already be acknowledged, but
    // only the explicit end after its operating interval and settling qualifies the last generation.
    assert!(
        recreation
            .validate_generation_end(2, GenerationEnd::DeadlineReached)
            .is_err()
    );
    assert!(
        recreation
            .validate_generation_end(2, GenerationEnd::WindowClosed)
            .is_err()
    );
    assert!(recreation.validate_generation_end(2, GenerationEnd::Recreated).is_err());
    assert!(
        recreation
            .validate_generation_end(1, GenerationEnd::FinalGenerationComplete)
            .is_err()
    );
    assert!(recreation.validate_generation_end(0, GenerationEnd::Recreated).is_ok());
    assert!(recreation.validate_generation_end(1, GenerationEnd::Recreated).is_ok());
    assert!(
        recreation
            .validate_generation_end(2, GenerationEnd::FinalGenerationComplete)
            .is_ok()
    );
}
