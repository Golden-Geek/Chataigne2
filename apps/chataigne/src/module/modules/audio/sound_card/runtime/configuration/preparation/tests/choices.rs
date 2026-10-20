use super::*;
use golden_core::parameter::ParameterEventBehaviour;

#[test]
fn choice_preparation_fences_each_active_selector_without_invalidating_disjoint_audio_values() {
    for (key, next_value) in [
        ("audio_driver", "none"),
        ("device", "unavailable"),
        ("input_device", "unavailable"),
        ("output_device", "none"),
        ("sample_rate", "44100"),
        ("buffer_size", "128"),
    ] {
        let (mut engine, module) = sound_tests::create_sound_card();
        let Some(parameter) = find_path(&engine.process_tree_snapshot(), module, &format!("connection/{key}")) else {
            assert_eq!(key, "device", "only the inactive duplex selector is absent");
            continue;
        };
        let mut constraints = engine
            .process_tree_snapshot()
            .node(parameter)
            .unwrap()
            .param_constraints
            .clone()
            .unwrap();
        constraints.enum_options = Default::default();
        engine.edits.push(Edit::SetParamConstraints {
            node: parameter,
            constraints,
        });
        engine.apply_edits().unwrap();
        assert_ne!(
            engine.process_tree_snapshot().node(parameter).unwrap().param_value,
            Some(ParamValue::Enum(next_value.into())),
            "the source-fence fixture must perform an actual value change",
        );
        let inventory = runtime::observation::fixture_inventory(golden_audio::AudioDeviceInspectorState::default(), 7);
        let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
        let mut request = Preparation::start_choices(&engine.process_tree_snapshot(), module, inventory, wake).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !request.is_complete() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(request.poll().unwrap().is_some());
        assert!(request.matches_device_revision(7));
        assert!(!request.matches_device_revision(8));
        let gain = find_path(
            &engine.process_tree_snapshot(),
            module,
            "parameters/output/master_gain_db",
        )
        .unwrap();
        engine.edits.push(Edit::SetParam {
            node: gain,
            value: ParamValue::Float(-6.0),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().unwrap();
        assert!(request.matches(&engine.process_tree_snapshot(), module));
        engine.edits.push(Edit::SetParam {
            node: parameter,
            value: ParamValue::Enum(next_value.into()),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().unwrap();
        assert!(
            !request.matches(&engine.process_tree_snapshot(), module),
            "{key} must fence before callbacks dispatch"
        );
    }
}

#[test]
fn choice_capacity_and_worker_exit_have_explicit_revision_fenced_completions() {
    let (engine, module) = sound_tests::create_sound_card();
    let source = runtime::observation::fixture_inventory(golden_audio::AudioDeviceInspectorState::default(), 7);
    let admission = RetirementPool::new(NonZeroUsize::MIN);
    let state = Arc::new(State {
        device_captured: AtomicBool::new(false),
        device_revision: AtomicU64::new(0),
        cancelled: AtomicBool::new(false),
        complete: AtomicBool::new(false),
        result: Mutex::new(None),
        wake: RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap(),
        _admission: admission.try_reserve().unwrap(),
    });
    let job = Job::<ChoicePlan, DeviceInventory> {
        previous: None,
        snapshot: engine.process_tree_snapshot().try_lease().unwrap(),
        module,
        source,
        state: state.clone(),
    };
    let too_small = PayloadPool::new("choice-capacity-test", 1, SCRATCH_BYTES - 1).unwrap();
    let error = job.run(&too_small).err().unwrap();
    assert!(error.retryable);
    assert!(state.device_captured.load(Ordering::Acquire));
    assert_eq!(state.device_revision.load(Ordering::Acquire), 7);
    drop(job);
    assert!(state.complete.load(Ordering::Acquire));
    let result = state
        .result
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .take()
        .unwrap();
    assert!(result.err().unwrap().message.contains("stopped before completion"));
    assert!(
        admission.try_reserve().is_err(),
        "completion owner still holds the request ticket"
    );
    drop(state);
    assert!(admission.try_reserve().is_ok());
}

#[test]
fn cancelled_choice_job_releases_its_source_without_inventing_a_failure() {
    let (engine, module) = sound_tests::create_sound_card();
    let source = runtime::observation::fixture_inventory(golden_audio::AudioDeviceInspectorState::default(), 7);
    let admission = RetirementPool::new(NonZeroUsize::MIN);
    let state = Arc::new(State {
        device_captured: AtomicBool::new(false),
        device_revision: AtomicU64::new(0),
        cancelled: AtomicBool::new(true),
        complete: AtomicBool::new(false),
        result: Mutex::new(None),
        wake: RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap(),
        _admission: admission.try_reserve().unwrap(),
    });
    let job = Job::<ChoicePlan, DeviceInventory> {
        previous: None,
        snapshot: engine.process_tree_snapshot().try_lease().unwrap(),
        module,
        source,
        state: state.clone(),
    };
    let pool = PayloadPool::new("choice-cancel-test", 1, SCRATCH_BYTES).unwrap();
    assert!(job.run(&pool).err().unwrap().message.contains("superseded"));
    drop(job);
    assert!(!state.complete.load(Ordering::Acquire));
    assert!(state
        .result
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .is_none());
    drop(state);
    assert!(admission.try_reserve().is_ok());
}
