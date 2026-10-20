use super::*;
use crate::app::module_modules_audio_sound_card::{runtime, tests as sound_tests};
use golden_core::{
    edit::Edit,
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};
use std::time::{Duration, Instant};

fn fixture() -> (
    crate::app::AppEngine,
    NodeId,
    golden_audio::AudioEngine,
    RuntimeWakeSender,
) {
    let (mut engine, module) = sound_tests::create_sound_card();
    sound_tests::select_enum_parameter(&mut engine, module, "connection/input_device", "none");
    sound_tests::select_enum_parameter(&mut engine, module, "connection/output_device", "none");
    let native = golden_audio::AudioEngineBuilder::default()
        .without_backends()
        .with_backend(golden_audio::NullBackend)
        .build()
        .unwrap();
    let wake = RuntimeWakeSender::new(engine.node_waker(module).unwrap()).unwrap();
    (engine, module, native, wake)
}

fn prepared(
    engine: &crate::app::AppEngine,
    module: NodeId,
    native: &golden_audio::AudioEngine,
    wake: RuntimeWakeSender,
) -> (Preparation, RetainedPayload<PreparedConfiguration>) {
    let reader = runtime::observation::retain_reader(native.observations()).unwrap();
    let mut preparation = Preparation::start(&engine.process_tree_snapshot(), module, reader, wake).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(ready) = preparation.poll().unwrap() {
            return (preparation, ready);
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn worker_prepares_retained_configuration_and_fences_actual_edits_before_callbacks() {
    let (mut engine, module, mut native, wake) = fixture();
    let (request, result) = prepared(&engine, module, &native, wake);
    assert!(request.matches(&engine.process_tree_snapshot(), module));
    assert_eq!(result.sample_rate, golden_audio::SampleRate::default());
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
    assert!(
        !request.matches(&engine.process_tree_snapshot(), module),
        "fencing must not depend on dispatched change callbacks"
    );
    drop((request, result));
    native.shutdown().unwrap();
}

#[test]
fn input_admission_rejects_large_values_before_configuration_materialization() {
    let (mut engine, module, mut native, wake) = fixture();
    let parameter = find_path(&engine.process_tree_snapshot(), module, "connection/audio_driver").unwrap();
    let original = Key::from(&engine.process_tree_snapshot(), module);
    let mut constraints = engine
        .process_tree_snapshot()
        .node(parameter)
        .unwrap()
        .param_constraints
        .clone()
        .unwrap();
    // Exercise the preparation boundary with an authored value admitted by Core.
    constraints.enum_options.clear();
    engine.edits.push(Edit::SetParamConstraints {
        node: parameter,
        constraints,
    });
    engine.edits.push(Edit::SetParam {
        node: parameter,
        value: ParamValue::Enum("x".repeat(1024 * 1024 + 1)),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    assert_ne!(original, Key::from(&engine.process_tree_snapshot(), module));
    let reader = runtime::observation::retain_reader(native.observations()).unwrap();
    let mut preparation = Preparation::start(&engine.process_tree_snapshot(), module, reader, wake).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match preparation.poll() {
            Err(error) => {
                assert!(error.message.contains("source exceeds"));
                break;
            }
            Ok(None) => {}
            Ok(Some(_)) => panic!("oversized source admitted"),
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    native.shutdown().unwrap();
}

#[test]
fn bindings_promote_only_after_matching_native_application_and_survive_rejection() {
    let (engine, module, mut native, wake) = fixture();
    let (request, result) = prepared(&engine, module, &native, wake);
    let mut runtime = runtime::SoundCardRuntime::start_selected(
        golden_audio::SampleRate::default(),
        Some(golden_audio::NullBackend::backend_id()),
    )
    .unwrap();
    let first = runtime.submit(result.clone()).unwrap();
    assert!(runtime.active_configuration.is_none());
    assert!(runtime.is_applying_configuration());
    assert!(runtime.submit(result.clone()).is_err());
    assert_eq!(runtime.pending_application.as_ref().unwrap().0, first);
    let deadline = Instant::now() + Duration::from_secs(3);
    while runtime.is_applying_configuration() {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        runtime.poll(&mut ctx, module, true);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(runtime.binding_generation, first);
    let active = runtime.active_configuration.as_ref().unwrap().clone();
    runtime
        .control
        .submit(AudioCommand::ApplyConfiguration {
            generation: golden_audio::ConfigGeneration::new(1000),
            config: Arc::new(golden_audio::AudioConfiguration::empty()),
        })
        .unwrap();
    runtime.submit(result).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while runtime.is_applying_configuration() {
        let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
        runtime.poll(&mut ctx, module, true);
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(std::ptr::eq(
        &**runtime.active_configuration.as_ref().unwrap(),
        &*active
    ));
    assert_eq!(runtime.binding_generation, first);
    assert!(matches!(
        runtime.take_configuration_feedback(),
        Some(runtime::ConfigurationFeedback::Rejected(_))
    ));
    drop((runtime, request, active));
    native.shutdown().unwrap();
}

#[test]
fn preparation_capacity_and_dropped_jobs_are_retryable_without_disabling_audio() {
    let (engine, module, mut native, wake) = fixture();
    let admission = RetirementPool::new(NonZeroUsize::MIN);
    let state = Arc::new(State {
        device_captured: AtomicBool::new(false),
        device_revision: AtomicU64::new(0),
        cancelled: AtomicBool::new(false),
        complete: AtomicBool::new(false),
        result: Mutex::new(None),
        wake,
        _admission: admission.try_reserve().unwrap(),
    });
    let job: Job = Job {
        previous: None,
        snapshot: engine.process_tree_snapshot().try_lease().unwrap(),
        module,
        reader: runtime::observation::retain_reader(native.observations()).unwrap(),
        state: state.clone(),
    };
    let too_small = PayloadPool::new("config-small-test", 4, 1024).unwrap();
    let error = job.run(&too_small).unwrap_err();
    assert!(error.retryable);
    let no_warning_slot = PayloadPool::new("config-warning-capacity-test", 1, SCRATCH_BYTES).unwrap();
    let error = job.run(&no_warning_slot).unwrap_err();
    assert!(error.retryable);
    assert!(error.message.contains("warning-plan lifetime capacity"));
    assert!(native.observations().inspect(|snapshot| snapshot.enabled));
    drop(job);
    let result = state.result.lock().unwrap_or_else(|p| p.into_inner()).take().unwrap();
    assert!(
        result.unwrap_err().retryable,
        "worker exit must have an explicit completion"
    );
    assert!(state.complete.load(Ordering::Acquire));
    native.shutdown().unwrap();
}

#[test]
fn cancellation_releases_a_queued_preparation_without_manufacturing_a_completion() {
    let (engine, module, mut native, wake) = fixture();
    let admission = RetirementPool::new(NonZeroUsize::MIN);
    let state = Arc::new(State {
        device_captured: AtomicBool::new(false),
        device_revision: AtomicU64::new(0),
        cancelled: AtomicBool::new(true),
        complete: AtomicBool::new(false),
        result: Mutex::new(None),
        wake,
        _admission: admission.try_reserve().unwrap(),
    });
    let job: Job = Job {
        previous: None,
        snapshot: engine.process_tree_snapshot().try_lease().unwrap(),
        module,
        reader: runtime::observation::retain_reader(native.observations()).unwrap(),
        state: state.clone(),
    };
    let pool = PayloadPool::new("config-cancel-test", 1, SCRATCH_BYTES).unwrap();
    assert!(job.run(&pool).unwrap_err().message.contains("cancelled"));
    drop(job);
    assert!(state.result.lock().unwrap_or_else(|p| p.into_inner()).is_none());
    native.shutdown().unwrap();
}

#[test]
fn runtime_settles_old_warning_receipts_before_capturing_replacement_preparation() {
    let (mut engine, module, mut native, wake) = fixture();
    let mut runtime = runtime::SoundCardRuntime::start_selected(
        golden_audio::SampleRate::default(),
        Some(golden_audio::NullBackend::backend_id()),
    )
    .unwrap();
    let warning = runtime::RuntimeWarning {
        node: module,
        id: "sound-card-unresolved-channel".into(),
        message: "old projection".into(),
        detail: None,
    };
    let pool = PayloadPool::new("warning-preparation-fence-test", 1, 64 * 1024).unwrap();
    let plan = WarningPlan::prepare(&engine.process_tree_snapshot(), module, &[warning]).unwrap();
    let bytes = plan.bytes;
    runtime.warnings.start(pool.try_retain(plan, bytes).unwrap());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    runtime.poll(&mut ctx, module, true);
    assert!(!ctx.edits.pending.is_empty());
    assert!(runtime
        .prepare_configuration(&engine.process_tree_snapshot(), module, wake.clone())
        .unwrap()
        .is_none());
    assert!(runtime.preparation_quiescing);
    assert!(
        runtime.preparation.is_none(),
        "no source may be captured before admitted warning edits settle"
    );
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    runtime.poll(&mut ctx, module, true);
    runtime
        .prepare_configuration(&engine.process_tree_snapshot(), module, wake)
        .unwrap();
    assert!(runtime.preparation.is_some());
    assert!(!runtime.preparation_quiescing);
    drop(runtime);
    native.shutdown().unwrap();
}

#[test]
fn selection_ticket_ignores_value_edits_but_fences_actual_selector_edits_before_callbacks() {
    let (mut engine, module, mut native, wake) = fixture();
    let mut request = Preparation::<SelectionPlan>::start_selection(
        &engine.process_tree_snapshot(),
        module,
        runtime::observation::retain_reader(native.observations()).unwrap(),
        wake,
        None,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while request.poll().unwrap().is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let gain = find_path(
        &engine.process_tree_snapshot(),
        module,
        "parameters/output/master_gain_db",
    )
    .unwrap();
    engine.edits.push(Edit::SetParam {
        node: gain,
        value: ParamValue::Float(-3.0),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    assert!(request.matches(&engine.process_tree_snapshot(), module));
    let driver = find_path(&engine.process_tree_snapshot(), module, "connection/audio_driver").unwrap();
    engine.edits.push(Edit::SetParam {
        node: driver,
        value: ParamValue::Enum("none".into()),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    assert!(!request.matches(&engine.process_tree_snapshot(), module));
    native.shutdown().unwrap();
}

#[test]
fn stale_device_state_cannot_publish_a_preparation_error_or_disable_the_current_runtime() {
    let (engine, module, mut native, wake) = fixture();
    let mut runtime = runtime::SoundCardRuntime::start_selected(
        golden_audio::SampleRate::default(),
        Some(golden_audio::NullBackend::backend_id()),
    )
    .unwrap();
    let admission = RetirementPool::new(NonZeroUsize::MIN);
    runtime.preparation = Some(Preparation {
        key: Key::from(&engine.process_tree_snapshot(), module).unwrap(),
        state: Arc::new(State {
            device_captured: AtomicBool::new(true),
            device_revision: AtomicU64::new(u64::MAX),
            cancelled: AtomicBool::new(false),
            complete: AtomicBool::new(true),
            result: Mutex::new(Some(Err(ConfigurationError {
                node: module,
                retryable: false,
                message: "obsolete device failure".into(),
            }))),
            wake: wake.clone(),
            _admission: admission.try_reserve().unwrap(),
        }),
        ready: None,
    });
    assert!(runtime
        .prepare_configuration(&engine.process_tree_snapshot(), module, wake)
        .unwrap()
        .is_none());
    assert!(runtime.preparation.is_none());
    assert!(runtime
        .event_bridge
        .configuration_reader()
        .inspect(|snapshot| snapshot.enabled));
    drop(runtime);
    native.shutdown().unwrap();
}

#[test]
fn coarse_admission_retry_keeps_the_ready_selection_and_its_source_fence() {
    let (engine, module, mut native, wake) = fixture();
    let mut runtime = runtime::SoundCardRuntime::start_selected(
        golden_audio::SampleRate::default(),
        Some(golden_audio::NullBackend::backend_id()),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let ready = loop {
        if let Some(ready) = runtime
            .prepare_selection(&engine.process_tree_snapshot(), module, wake.clone())
            .unwrap()
        {
            break ready;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    runtime.defer_preparation_retry(&wake);
    assert!(runtime.preparation_retry_at.is_some());
    let retained = runtime.selection_preparation.as_ref().unwrap().ready.as_ref().unwrap();
    assert!(std::ptr::eq(&**retained, &*ready));
    assert!(runtime
        .selection_preparation
        .as_ref()
        .unwrap()
        .matches(&engine.process_tree_snapshot(), module));
    drop((runtime, ready));
    native.shutdown().unwrap();
}
