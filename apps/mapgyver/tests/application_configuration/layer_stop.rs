use super::{
    scoped_stops::{held_config, installed},
    *,
};
use crate::matrix_controls::{Caller, Scene, Scope};
use golden_core::parameter::{ParamValue, ParameterEventBehaviour};
use golden_video::{
    OutputId,
    runtime::{NullConfig, VideoRuntime},
};
use mapgyver::{
    matrix::protocol::{LayerId, MatrixEdit, MatrixItemId},
    media::{
        capture_channels_at,
        rendering::{ChannelRenderer, ChannelRoute},
    },
};

#[test]
fn timed_layer_stop_retires_the_shared_process_only_after_held_final_use_and_project_close() {
    let mut scene = Scene::with_owned_process();
    let duration = scene.parameter_field(Scope::Layer, "duration_ms");
    let ack = apply_project_intent(
        &mut scene.engine,
        UiEditIntent::SetParam {
            node: duration,
            value: ParamValue::Float(1200.0),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    scene.play();
    let root = scene.root;
    let stopped_cell = ApplicationId(scene.owner(Scope::Cell));
    let mut runtime = ApplicationRuntime::start(&scene.engine, None).unwrap();
    let process_retirement = runtime.retirement();
    let config = held_config();
    let video = VideoRuntime::spawn_null(
        config.clone(),
        NullConfig {
            auto_complete: false,
            ..Default::default()
        },
    )
    .unwrap();
    let control = video.null_control().unwrap();
    let retired = video.retirement();
    let mut renderer = ChannelRenderer::new(video, config.plan_limits);
    renderer.bind_applications(runtime.plan_retainer());
    let capture = |scene: &Scene, renderer: &ChannelRenderer| {
        capture_channels_at(&scene.engine, &[root], 16, renderer.read_time().unwrap()).unwrap()
    };
    let initial = capture(&scene, &renderer).application_demand();
    assert_eq!(initial.len(), 2);
    runtime.reconcile(&scene.engine, &initial).unwrap();
    let original = wait(&runtime, ApplicationPhase::Ready);
    assert_eq!(runtime.snapshot().applications.len(), 1);
    assert_eq!(original.consumers.len(), 2);

    scene.stop(Scope::Layer, Caller::Script);
    let fading = capture(&scene, &renderer);
    let outgoing = fading.application_demand();
    assert_eq!(outgoing.len(), 2);
    assert!(
        outgoing
            .iter()
            .any(|entry| { entry.application == stopped_cell && entry.reasons.contains(&DemandReason::Outgoing) })
    );
    runtime.reconcile(&scene.engine, &outgoing).unwrap();
    let request = renderer
        .publish(
            fading,
            vec![ChannelRoute {
                output: OutputId::new(1).unwrap(),
                channel: root,
            }],
        )
        .unwrap();
    installed(&renderer, request);
    let until = Instant::now() + Duration::from_secs(4);
    while renderer.snapshot().frames_submitted == 0 {
        assert!(Instant::now() < until, "held fading frame was not submitted");
        std::thread::sleep(Duration::from_millis(5));
    }
    let held = renderer.snapshot().resources.unwrap();
    assert!(held.in_flight > 0 && held.last_completed.serial < held.last_submitted.serial);
    let remaining = loop {
        let demand = capture(&scene, &renderer).application_demand();
        if demand.iter().all(|entry| entry.application != stopped_cell) {
            break demand;
        }
        assert!(Instant::now() < until, "logical Layer Stop demand did not expire");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(remaining.len(), 1, "the peer remains playing after the fade");
    let layer = LayerId(scene.owner(Scope::Layer));
    scene.matrix(MatrixEdit::RemoveItem {
        item: MatrixItemId::Layer { id: layer },
    });
    let after_deletion = capture(&scene, &renderer).application_demand();
    assert_eq!(after_deletion, remaining);
    runtime.reconcile(&scene.engine, &remaining).unwrap();
    assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, original.process_id);
    scene.stop(Scope::All, Caller::Transport);
    assert!(capture(&scene, &renderer).application_demand().is_empty());
    runtime.reconcile(&scene.engine, &[]).unwrap();
    drop(scene);
    runtime.close();
    renderer.close();
    drop(renderer);
    assert!(!retired.wait(Duration::from_millis(100)));
    assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, original.process_id);

    assert!(control.complete_through(control.snapshot().last_submitted.unwrap()));
    wait(&runtime, ApplicationPhase::Idle);
    assert!(retired.wait(Duration::from_secs(4)));
    drop(runtime);
    assert!(process_retirement.wait_for_idle(Duration::from_secs(4)));
    eprintln!(
        "Held fading plan retired with the shared process PID {:?}",
        original.process_id
    );
}
