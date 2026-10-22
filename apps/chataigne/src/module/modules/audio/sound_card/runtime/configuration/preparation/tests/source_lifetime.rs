use super::*;

#[test]
fn queued_configuration_survives_continuous_value_only_writes() {
    let (mut engine, module, mut native, wake) = fixture();
    let mut runtime = runtime::SoundCardRuntime::start_selected(
        golden_audio::SampleRate::default(),
        Some(golden_audio::NullBackend::backend_id()),
    )
    .unwrap();
    let snapshot = engine.process_tree_snapshot();
    let admission = RetirementPool::new(NonZeroUsize::MIN);
    let state = Arc::new(State {
        device_captured: AtomicBool::new(false),
        device_revision: AtomicU64::new(0),
        cancelled: AtomicBool::new(false),
        complete: AtomicBool::new(false),
        result: Mutex::new(None),
        wake: wake.clone(),
        _admission: admission.try_reserve().unwrap(),
    });
    runtime.preparation = Some(Preparation {
        key: Key::from(&snapshot, module).unwrap(),
        state: state.clone(),
        ready: None,
    });
    // Retain a real job without running it, as if it were queued behind slow native work.
    // Its source and cancellation state are exactly those consumed by the runtime facade.
    let job: Job = Job {
        previous: None,
        snapshot: snapshot.try_lease().unwrap(),
        module,
        source: runtime.event_bridge.configuration_reader(),
        state: state.clone(),
    };
    let value = find_path(&snapshot, module, "values/playback_status/active_voices").unwrap();
    for count in 1..=8 {
        engine.edits.push(Edit::SetParam {
            node: value,
            value: ParamValue::Int(count),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().unwrap();
        assert!(runtime
            .prepare_configuration(&engine.process_tree_snapshot(), module, wake.clone())
            .unwrap()
            .is_none());
        assert!(
            !state.cancelled.load(Ordering::Acquire),
            "value-only telemetry must not cancel queued configuration work"
        );
        assert!(Arc::ptr_eq(&runtime.preparation.as_ref().unwrap().state, &state));
    }
    let pool = PayloadPool::new("slow-configuration-test", 4, SCRATCH_BYTES * 2).unwrap();
    job.finish(job.run(&pool));
    let prepared = runtime
        .prepare_configuration(&engine.process_tree_snapshot(), module, wake)
        .unwrap()
        .expect("the original completed job must remain admissible");
    assert!(runtime
        .preparation
        .as_ref()
        .unwrap()
        .matches(&engine.process_tree_snapshot(), module));
    runtime.submit(prepared).unwrap();
    drop(job);
    drop(runtime);
    native.shutdown().unwrap();
}

#[test]
fn ready_configuration_ignores_value_warnings_but_fences_named_binding_changes_before_callbacks() {
    let (mut engine, module, mut native, wake) = fixture();
    let (request, _ready) = prepared(&engine, module, &native, wake);
    let value = find_path(
        &engine.process_tree_snapshot(),
        module,
        "values/playback_status/active_voices",
    )
    .unwrap();
    engine.edits.push(Edit::SetNodeWarning {
        node: value,
        warning: golden_core::node::NodeWarning {
            id: "value-probe".into(),
            message: "only presentation".into(),
            detail: None,
        },
    });
    engine.apply_edits().unwrap();
    assert!(request.matches(&engine.process_tree_snapshot(), module));
    engine.edits.push(Edit::PatchMeta {
        node: value,
        patch: golden_core::node::NodeMetaPatch {
            short_name: Some("renamed_voice_count".into()),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(!request.matches(&engine.process_tree_snapshot(), module));
    native.shutdown().unwrap();
}

#[test]
fn same_uuid_value_replacement_and_missing_binding_root_invalidate_ready_work() {
    use golden_core::node::{Folder, Node};
    use golden_core::parameter::{Parameter, ParameterChangeCheck};
    let (mut engine, module, mut native, wake) = fixture();
    let (request, _ready) = prepared(&engine, module, &native, wake.clone());
    let snapshot = engine.process_tree_snapshot();
    let value = find_path(&snapshot, module, "values/playback_status/active_voices").unwrap();
    let mut replacement = Parameter::new("Active Voices", ParamValue::Int(0), ParameterChangeCheck::ValueChange);
    replacement.node_data_mut().meta.uuid = snapshot.node(value).unwrap().uuid;
    engine.edits.push(Edit::ReplaceNode {
        node: value,
        new_node: Box::new(replacement),
    });
    engine.apply_edits().unwrap();
    assert!(!request.matches(&engine.process_tree_snapshot(), module));
    let (current, _ready) = prepared(&engine, module, &native, wake.clone());
    let values = find_path(&engine.process_tree_snapshot(), module, "values").unwrap();
    engine.edits.push(Edit::RemoveNode { node: values });
    engine.apply_edits().unwrap();
    assert!(!current.matches(&engine.process_tree_snapshot(), module));
    let (absent, _ready) = prepared(&engine, module, &native, wake);
    engine.add_node(Folder::new("values").into(), Some(module));
    engine.apply_edits().unwrap();
    assert!(!absent.matches(&engine.process_tree_snapshot(), module));
    native.shutdown().unwrap();
}
