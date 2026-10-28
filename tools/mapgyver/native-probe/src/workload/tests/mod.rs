use super::{ConsumerMode, GenerationEnd, PresentationPolicy, RunSpec, Scenario};

#[test]
fn consumer_modes_parse_only_explicit_choices_and_keep_typed_wire_values() {
    for (name, mode) in [("active", ConsumerMode::Active), ("paused", ConsumerMode::Paused)] {
        assert_eq!(name.parse::<ConsumerMode>().unwrap(), mode);
        assert_eq!(mode.name(), name);
        let encoded = serde_json::to_value(mode).unwrap();
        assert_eq!(encoded, name);
        assert_eq!(serde_json::from_value::<ConsumerMode>(encoded).unwrap(), mode);
    }
    for invalid in ["", "pause", "Active", "Paused", "0", "active ", " paused"] {
        assert!(invalid.parse::<ConsumerMode>().is_err(), "{invalid:?}");
    }
}

#[test]
fn every_exercise_defaults_to_active_consumption() {
    for spec in [
        RunSpec::new(Scenario::Diagnostic, 8, 0).unwrap(),
        RunSpec::new(Scenario::Q01, 8, 30).unwrap(),
        RunSpec::new(Scenario::Q02, 120, 30).unwrap().with_gpu_stages().unwrap(),
        RunSpec::recreation(Scenario::Q01, 1).unwrap(),
        RunSpec::editor_isolation(Scenario::Q02).unwrap(),
    ] {
        assert_eq!(spec.consumer_mode(), ConsumerMode::Active);
        assert_eq!(serde_json::to_value(spec).unwrap()["consumer_mode"], "active");
    }
}

#[test]
fn paused_consumption_requires_the_staged_fixed_scene_with_exact_warmup() {
    for scenario in [Scenario::Q01, Scenario::Q02] {
        let baseline = RunSpec::new(scenario, 8, 30).unwrap();
        assert!(baseline.with_paused_consumer().is_err());
        let paused = baseline.with_gpu_stages().unwrap().with_paused_consumer().unwrap();
        assert_eq!(paused.consumer_mode(), ConsumerMode::Paused);
        assert_eq!(serde_json::to_value(paused).unwrap()["consumer_mode"], "paused");
        assert_eq!(paused.warmup_seconds(), 30);
        assert_eq!(paused.total_seconds(), baseline.total_seconds());
        assert!(paused.gpu_stages());
        assert_eq!(paused.presentation_policy(), PresentationPolicy::Latest);
        assert!(
            paused
                .with_presentation_policy(PresentationPolicy::OutputVblank)
                .is_err()
        );
        assert_eq!(
            paused
                .with_presentation_policy(PresentationPolicy::Latest)
                .unwrap()
                .consumer_mode(),
            ConsumerMode::Paused
        );
        assert_eq!(paused.with_gpu_stages().unwrap().consumer_mode(), ConsumerMode::Paused);
        assert_eq!(
            paused.with_paused_consumer().unwrap().consumer_mode(),
            ConsumerMode::Paused
        );
        for warmup in [0, 29, 31] {
            assert!(
                RunSpec::new(scenario, 8, warmup)
                    .unwrap()
                    .with_gpu_stages()
                    .unwrap()
                    .with_paused_consumer()
                    .is_err()
            );
        }
        assert!(
            baseline
                .with_presentation_policy(PresentationPolicy::OutputVblank)
                .unwrap()
                .with_paused_consumer()
                .is_err()
        );
    }
    for spec in [
        RunSpec::new(Scenario::Diagnostic, 8, 30).unwrap(),
        RunSpec::recreation(Scenario::Q01, 1).unwrap(),
        RunSpec::editor_isolation(Scenario::Q02).unwrap(),
    ] {
        assert!(spec.with_paused_consumer().is_err());
    }
}

#[test]
fn gpu_stage_queries_are_explicit_in_timed_fixed_scene_reports() {
    for scenario in [Scenario::Q01, Scenario::Q02] {
        let baseline = RunSpec::new(scenario, 120, 30).unwrap();
        assert!(!baseline.gpu_stages());
        assert_eq!(serde_json::to_value(baseline).unwrap()["gpu_stages"], false);
        let staged = baseline.with_gpu_stages().unwrap();
        assert!(staged.gpu_stages());
        assert!(!baseline.gpu_stages());
        assert_eq!(staged.presentation_policy(), PresentationPolicy::Latest);
        assert_eq!(staged.total_seconds(), baseline.total_seconds());
        assert_eq!(staged.warmup_seconds(), baseline.warmup_seconds());
        assert_eq!(serde_json::to_value(staged).unwrap()["gpu_stages"], true);
        assert!(staged.with_gpu_stages().unwrap().gpu_stages());
        assert!(
            staged
                .with_presentation_policy(PresentationPolicy::Latest)
                .unwrap()
                .gpu_stages()
        );
    }
}

#[test]
fn diagnostic_and_lifecycle_exercises_keep_gpu_stage_queries_disabled() {
    for spec in [
        RunSpec::new(Scenario::Diagnostic, 8, 0).unwrap(),
        RunSpec::recreation(Scenario::Q01, 1).unwrap(),
        RunSpec::recreation(Scenario::Q02, 2).unwrap(),
        RunSpec::editor_isolation(Scenario::Q01).unwrap(),
        RunSpec::editor_isolation(Scenario::Q02).unwrap(),
    ] {
        assert!(!spec.gpu_stages());
        assert_eq!(serde_json::to_value(spec).unwrap()["gpu_stages"], false);
        assert!(spec.with_gpu_stages().is_err());
    }
}

#[test]
fn gpu_stage_queries_and_output_vblank_are_excluded_in_both_configuration_orders() {
    let baseline = RunSpec::new(Scenario::Q02, 120, 30).unwrap();
    let vblank = baseline
        .with_presentation_policy(PresentationPolicy::OutputVblank)
        .unwrap();
    assert!(vblank.with_gpu_stages().is_err());
    let staged = baseline.with_gpu_stages().unwrap();
    assert!(
        staged
            .with_presentation_policy(PresentationPolicy::OutputVblank)
            .is_err()
    );
    assert!(
        vblank
            .with_presentation_policy(PresentationPolicy::Latest)
            .unwrap()
            .with_gpu_stages()
            .unwrap()
            .gpu_stages()
    );
}

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
