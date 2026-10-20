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
        cancelled: AtomicBool::new(false),
        complete: AtomicBool::new(false),
        result: Mutex::new(None),
        wake,
        _admission: admission.try_reserve().unwrap(),
    });
    let job = Job {
        snapshot: engine.process_tree_snapshot().try_lease().unwrap(),
        module,
        reader: runtime::observation::retain_reader(native.observations()).unwrap(),
        state: state.clone(),
    };
    let too_small = PayloadPool::new("config-small-test", 4, 1024).unwrap();
    let error = job.run(&too_small).unwrap_err();
    assert!(error.retryable);
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
        cancelled: AtomicBool::new(true),
        complete: AtomicBool::new(false),
        result: Mutex::new(None),
        wake,
        _admission: admission.try_reserve().unwrap(),
    });
    let job = Job {
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
