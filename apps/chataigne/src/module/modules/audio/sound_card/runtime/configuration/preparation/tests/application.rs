use super::*;
use golden_core::node::Node;

#[test]
fn native_submission_and_stop_reject_already_queued_value_projection() {
    for stop in [false, true] {
        let (mut engine, module, mut native, wake) = fixture();
        let (request, result) = prepared(&engine, module, &native, wake);
        let mut runtime = runtime::SoundCardRuntime::start_selected(
            golden_audio::SampleRate::default(),
            Some(golden_audio::NullBackend::backend_id()),
        )
        .unwrap();
        runtime.submit(result.clone()).unwrap();
        let node = find_path(
            &engine.process_tree_snapshot(),
            module,
            "values/playback_status/active_voices",
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut queued = loop {
            let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
            runtime.poll(&mut ctx, module, true);
            if ctx
                .edits
                .pending
                .iter()
                .any(|request| matches!(request.edit, Edit::SetParam { node: target, .. } if target == node))
            {
                break ctx;
            }
            engine.absorb_edits(&mut ctx).unwrap();
            engine.apply_edits().unwrap();
            assert!(
                Instant::now() < deadline,
                "matching native application must publish values"
            );
            std::thread::sleep(Duration::from_millis(1));
        };
        engine.edits.push(Edit::SetParam {
            node,
            value: ParamValue::Int(7),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().unwrap();
        if stop {
            runtime.stop();
        } else {
            runtime.submit(result).unwrap();
        }
        engine.absorb_edits(&mut queued).unwrap();
        assert!(engine.apply_edits().is_err());
        assert_eq!(
            engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value,
            ParamValue::Int(7)
        );
        drop((runtime, request));
        native.shutdown().unwrap();
    }
}
