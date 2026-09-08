use super::*;

#[test]
fn project_replacement_commits_one_generation_after_exclusive_device_handoff() {
    let _guard = REPLACEMENT_TEST_LOCK.lock().expect("replacement test lock");
    reset_replacement_probes();
    let runtime = runtime_with_old_project_owner();
    let result = runtime
        .replace_project(replacement_request(
            replacement_engine("Candidate Root", false),
            "successful-handoff",
        ))
        .expect("replacement should commit");

    assert_eq!(result.project_generation.get(), 2);
    assert_eq!(runtime.current_project_generation(), result.project_generation);
    assert_eq!(
        runtime.project_runtime_status(),
        ProjectRuntimeStatus::Active {
            generation: result.project_generation,
        }
    );
    assert!(OLD_PROJECT_DESTROYED.load(Ordering::SeqCst));
    assert!(OLD_PROJECT_DROPPED.load(Ordering::SeqCst));
    assert!(CANDIDATE_SAW_EXCLUSIVE_HANDOFF.load(Ordering::SeqCst));
    assert_eq!(LIVE_PROJECT_OWNER.load(Ordering::SeqCst), 2);
    assert!(result.retirement_error.is_none());
    let snapshot = runtime
        .snapshot(UiSubscriptionScope::WholeGraph)
        .expect("replacement snapshot");
    assert!(snapshot.nodes.iter().any(|node| node.meta.label == "Candidate Root"));
    assert!(!snapshot.nodes.iter().any(|node| node.meta.label == "Old Root"));
    assert!(!snapshot.history.can_undo && !snapshot.history.can_redo);

    runtime.stop(()).expect("candidate should stop");
}

#[test]
fn replacement_failures_before_handoff_keep_old_project_active() {
    let _guard = REPLACEMENT_TEST_LOCK.lock().expect("replacement test lock");
    for stage in [
        ProjectReplacementStage::CandidatePreparation,
        ProjectReplacementStage::ScriptPreparation,
        ProjectReplacementStage::CandidateCompilation,
    ] {
        reset_replacement_probes();
        let runtime = runtime_with_old_project_owner();
        runtime.set_project_replacement_fault_hook(Some(Arc::new(move |current| {
            if current == stage {
                Err(format!("injected {stage:?} failure"))
            } else {
                Ok(())
            }
        })));

        let error = runtime
            .replace_project(replacement_request(
                replacement_engine("Rejected Candidate", false),
                "pre-handoff-failure",
            ))
            .expect_err("injected replacement should fail");
        assert!(error.contains("injected"));
        assert_eq!(runtime.current_project_generation().get(), 1);
        assert_eq!(
            runtime.project_runtime_status(),
            ProjectRuntimeStatus::Active {
                generation: crate::app::ProjectGeneration::INITIAL,
            }
        );
        assert_eq!(LIVE_PROJECT_OWNER.load(Ordering::SeqCst), 1);
        assert!(!OLD_PROJECT_DESTROYED.load(Ordering::SeqCst));
        let snapshot = runtime.snapshot(UiSubscriptionScope::WholeGraph).expect("old snapshot");
        assert!(snapshot.nodes.iter().any(|node| node.meta.label == "Old Root"));
        assert!(
            !snapshot
                .nodes
                .iter()
                .any(|node| node.meta.label == "Rejected Candidate")
        );

        runtime.set_project_replacement_fault_hook(None);
        runtime.stop(()).expect("old project should stop");
    }
}

#[test]
fn script_evaluation_failure_is_rejected_before_device_handoff() {
    let _guard = REPLACEMENT_TEST_LOCK.lock().expect("replacement test lock");
    reset_replacement_probes();
    let runtime = runtime_with_old_project_owner();
    let mut candidate = replacement_engine("Invalid Script Candidate", false);
    candidate.add_node(
        ScriptNode::new(
            "Broken Script",
            ScriptNodeConfig {
                source: ScriptSource::Inline {
                    text: "function on_init( {".to_string(),
                },
            },
        )
        .into(),
        None,
    );

    let error = runtime
        .replace_project(replacement_request(candidate, "invalid-script"))
        .expect_err("invalid script should reject the detached candidate");
    assert!(error.contains("script") && error.contains("failed validation"));
    assert!(matches!(
        runtime.project_runtime_status(),
        ProjectRuntimeStatus::Active { generation }
            if generation == crate::app::ProjectGeneration::INITIAL
    ));
    assert_eq!(LIVE_PROJECT_OWNER.load(Ordering::SeqCst), 1);
    assert!(!OLD_PROJECT_DESTROYED.load(Ordering::SeqCst));

    runtime.stop(()).expect("old project should stop");
}

#[test]
fn replacement_failures_after_handoff_leave_old_project_explicitly_paused() {
    let _guard = REPLACEMENT_TEST_LOCK.lock().expect("replacement test lock");
    for stage in [
        ProjectReplacementStage::DeviceActivation,
        ProjectReplacementStage::ActivatedCompilation,
        ProjectReplacementStage::PublicationPreparation,
    ] {
        reset_replacement_probes();
        let runtime = runtime_with_old_project_owner();
        runtime.set_project_replacement_fault_hook(Some(Arc::new(move |current| {
            if current == stage {
                Err(format!("injected {stage:?} failure"))
            } else {
                Ok(())
            }
        })));

        runtime
            .replace_project(replacement_request(
                replacement_engine("Rejected Candidate", false),
                "post-handoff-failure",
            ))
            .expect_err("injected replacement should fail");
        assert_eq!(runtime.current_project_generation().get(), 1);
        let ProjectRuntimeStatus::Paused { generation, error } = runtime.project_runtime_status() else {
            panic!("old project should be explicitly paused");
        };
        assert_eq!(generation, crate::app::ProjectGeneration::INITIAL);
        assert!(error.contains("failed"));
        assert!(OLD_PROJECT_DESTROYED.load(Ordering::SeqCst));
        assert!(CANDIDATE_DESTROYED.load(Ordering::SeqCst));
        assert_eq!(LIVE_PROJECT_OWNER.load(Ordering::SeqCst), 0);
        let snapshot = runtime
            .snapshot(UiSubscriptionScope::WholeGraph)
            .expect("old paused snapshot");
        assert!(snapshot.nodes.iter().any(|node| node.meta.label == "Old Root"));
        assert!(
            !snapshot
                .nodes
                .iter()
                .any(|node| node.meta.label == "Rejected Candidate")
        );
        let rejected = runtime.apply_ui_transaction(UiEditIntent::ReevaluateGraph, Some("paused-project"));
        assert!(!rejected.acknowledgement.success);
        assert_eq!(
            rejected.acknowledgement.error_code.as_deref(),
            Some("project_runtime_paused")
        );

        runtime.set_project_replacement_fault_hook(None);
    }
}

#[test]
fn activation_failure_releases_partial_candidate_ownership_and_preserves_old_projection() {
    let _guard = REPLACEMENT_TEST_LOCK.lock().expect("replacement test lock");
    reset_replacement_probes();
    let runtime = runtime_with_old_project_owner();

    let error = runtime
        .replace_project(replacement_request(
            replacement_engine("Failing Candidate", true),
            "activation-failure",
        ))
        .expect_err("invalid ready edit should reject activation");

    assert!(error.contains("missing node"));
    assert!(CANDIDATE_SAW_EXCLUSIVE_HANDOFF.load(Ordering::SeqCst));
    assert!(CANDIDATE_DESTROYED.load(Ordering::SeqCst));
    assert_eq!(LIVE_PROJECT_OWNER.load(Ordering::SeqCst), 0);
    assert!(matches!(
        runtime.project_runtime_status(),
        ProjectRuntimeStatus::Paused { generation, .. }
            if generation == crate::app::ProjectGeneration::INITIAL
    ));
    let snapshot = runtime
        .snapshot(UiSubscriptionScope::WholeGraph)
        .expect("old paused snapshot");
    assert!(snapshot.nodes.iter().any(|node| node.meta.label == "Old Root"));
    assert!(!snapshot.nodes.iter().any(|node| node.meta.label == "Failing Candidate"));
}

#[test]
fn retirement_failure_is_reported_after_new_generation_is_committed() {
    let _guard = REPLACEMENT_TEST_LOCK.lock().expect("replacement test lock");
    reset_replacement_probes();
    let runtime = runtime_with_old_project_owner();
    runtime.set_project_replacement_fault_hook(Some(Arc::new(|stage| {
        if stage == ProjectReplacementStage::Retirement {
            Err("injected retirement failure".to_string())
        } else {
            Ok(())
        }
    })));

    let result = runtime
        .replace_project(replacement_request(
            replacement_engine("Committed Candidate", false),
            "retirement-failure",
        ))
        .expect("retirement diagnostics must not roll back committed generation");
    assert_eq!(result.project_generation.get(), 2);
    assert_eq!(result.retirement_error.as_deref(), Some("injected retirement failure"));
    assert!(matches!(
        runtime.project_runtime_status(),
        ProjectRuntimeStatus::Active { generation } if generation.get() == 2
    ));
    assert_eq!(LIVE_PROJECT_OWNER.load(Ordering::SeqCst), 2);

    runtime.set_project_replacement_fault_hook(None);
    runtime.stop(()).expect("candidate should stop");
}

#[test]
fn superseded_candidate_is_discarded_before_device_handoff() {
    let _guard = REPLACEMENT_TEST_LOCK.lock().expect("replacement test lock");
    reset_replacement_probes();
    let runtime = runtime_with_old_project_owner();
    let (compile_entered_tx, compile_entered_rx) = mpsc::channel();
    let (release_compile_tx, release_compile_rx) = mpsc::channel();
    let release_compile_rx = Arc::new(Mutex::new(release_compile_rx));
    let (second_prepare_tx, second_prepare_rx) = mpsc::channel();
    let preparation_calls = Arc::new(AtomicUsize::new(0));
    let compilation_calls = Arc::new(AtomicUsize::new(0));
    runtime.set_project_replacement_fault_hook(Some(Arc::new(move |stage| {
        if stage == ProjectReplacementStage::CandidatePreparation
            && preparation_calls.fetch_add(1, Ordering::SeqCst) == 1
        {
            let _ = second_prepare_tx.send(());
        }
        if stage == ProjectReplacementStage::CandidateCompilation
            && compilation_calls.fetch_add(1, Ordering::SeqCst) == 0
        {
            let _ = compile_entered_tx.send(());
            release_compile_rx
                .lock()
                .expect("compile release lock")
                .recv()
                .expect("compile release signal");
        }
        Ok(())
    })));

    let first_runtime = runtime.clone();
    let first = thread::spawn(move || {
        first_runtime.replace_project(replacement_request(
            replacement_engine("Superseded Candidate", false),
            "superseded",
        ))
    });
    compile_entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("first candidate should reach compilation gate");

    let second_runtime = runtime.clone();
    let second = thread::spawn(move || {
        second_runtime.replace_project(replacement_request(
            replacement_engine("Winning Candidate", false),
            "winner",
        ))
    });
    second_prepare_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("newer generation should begin detached preparation");
    release_compile_tx.send(()).expect("release first candidate");

    let first_error = first
        .join()
        .expect("first replacement thread")
        .expect_err("older candidate should be superseded");
    assert!(first_error.contains("superseded"));
    let second_result = second
        .join()
        .expect("second replacement thread")
        .expect("newer candidate should commit");
    assert_eq!(second_result.project_generation.get(), 3);
    assert_eq!(runtime.current_project_generation().get(), 3);
    assert!(CANDIDATE_SAW_EXCLUSIVE_HANDOFF.load(Ordering::SeqCst));
    let snapshot = runtime
        .snapshot(UiSubscriptionScope::WholeGraph)
        .expect("winning snapshot");
    assert!(snapshot.nodes.iter().any(|node| node.meta.label == "Winning Candidate"));
    assert!(
        !snapshot
            .nodes
            .iter()
            .any(|node| node.meta.label == "Superseded Candidate")
    );

    runtime.set_project_replacement_fault_hook(None);
    runtime.stop(()).expect("winner should stop");
}
