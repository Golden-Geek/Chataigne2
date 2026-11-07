use super::*;
use crate::matrix_controls::{Caller, SCOPES, Scene, Scope};
use golden_video::{
    OutputId,
    runtime::{NullConfig, VideoRuntime},
};
use mapgyver::media::{
    capture_channels,
    rendering::{ChannelRenderer, ChannelRoute},
};
use std::num::NonZeroUsize;

#[test]
fn every_stop_scope_uses_real_transport_and_script_parameters_through_rapid_history_and_reload() {
    for caller in [Caller::Transport, Caller::Script] {
        let mut scene = Scene::new(None);
        let saved = to_sparse_project_json_pretty(&scene.engine).unwrap();
        let history = scene.engine.undo_len();
        for scope in SCOPES {
            for _ in 0..20 {
                scene.play();
                scene.stop(scope, caller);
                scene.check_stopped(scope);
            }
        }
        assert_eq!(scene.engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&scene.engine).unwrap(), saved);
        scene.engine = from_sparse_project_json::<AppNode>(&saved).unwrap();
        golden_core::app::prepare_engine_for_runtime(&mut scene.engine).unwrap();
        assert!(scene.playback().layers.is_empty());
        assert_eq!(scene.playback().desired_column, None);
        for scope in SCOPES {
            scene.play();
            scene.stop(scope, caller);
            scene.check_stopped(scope);
        }
    }
}

#[test]
fn every_stop_scope_cancels_queued_pulses_on_disable_and_undo_never_replays_them() {
    use golden_core::{
        edit::Edit,
        parameter::{ParamValue, ParameterEventBehaviour},
        ui_sync::NodeMetaPatch,
    };
    for scope in SCOPES {
        let mut scene = Scene::new(None);
        scene.play();
        let owner = scene.engine.node_id_by_uuid(scene.owner(scope)).unwrap();
        let parameter = scene.parameter(scope);
        scene.engine.edits.push(Edit::SetParam {
            node: parameter,
            value: ParamValue::Trigger(),
            behaviour: ParameterEventBehaviour::Append,
        });
        scene.engine.apply_edits().unwrap();
        let ack = apply_project_intent(
            &mut scene.engine,
            UiEditIntent::PatchMeta {
                node: owner,
                patch: NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                },
            },
        );
        assert!(ack.success, "{scope:?}: {ack:?}");
        let disabled = scene.playback();
        golden_core::application::service_script_app_operations(&mut scene.engine).unwrap();
        assert_eq!(scene.playback(), disabled);
        assert!(scene.engine.undo().unwrap());
        let restored = scene.playback();
        golden_core::application::service_script_app_operations(&mut scene.engine).unwrap();
        assert_eq!(
            scene.playback(),
            restored,
            "{scope:?}: undo must not replay the old pulse"
        );
        scene.play();
        scene.stop(scope, Caller::Transport);
        scene.check_stopped(scope);
    }
}

fn installed(renderer: &ChannelRenderer, request: golden_video::runtime::RequestId) {
    let until = Instant::now() + Duration::from_secs(4);
    loop {
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        if let Some(outcome) = outcomes.iter().find(|outcome| outcome.request == request) {
            assert!(
                matches!(outcome.result, golden_video::runtime::OutcomeKind::Installed { .. }),
                "{outcome:?}"
            );
            return;
        }
        assert!(Instant::now() < until, "scoped renderer installation deadline");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn every_scoped_stop_preserves_aggregate_owned_process_until_held_renderer_completion() {
    for scope in SCOPES {
        let mut scene = Scene::new(Some(ApplicationDefinition {
            executable: Some(env!("CARGO_BIN_EXE_application_process_fixture").into()),
            stop: ApplicationStop {
                graceful_ms: 30,
                retirement_ms: 1000,
            },
            restart: ApplicationRestart {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        }));
        scene.play();
        let mut runtime = ApplicationRuntime::start(&scene.engine, None).unwrap();
        let process_retirement = runtime.retirement();
        let capture = capture_channels(&scene.engine, &[scene.root], 16).unwrap();
        runtime.reconcile(&scene.engine, &capture.application_demand()).unwrap();
        let original = wait(&runtime, ApplicationPhase::Ready);
        assert_eq!(runtime.snapshot().applications.len(), 1);
        assert_eq!(original.consumers.len(), 2);
        let config = golden_video::runtime::RuntimeConfig {
            plan_limits: golden_video::plan::PlanLimits {
                max_passes: 16,
                max_external_textures: 16,
                max_outputs: 4,
                max_dependencies: 64,
                max_texture_dimension: 4096,
                max_texture_bytes: 64 * 1024 * 1024,
            },
            resource_limits: golden_video::runtime::ResourceLimits {
                max_bytes: 128 * 1024 * 1024,
                max_in_flight: NonZeroUsize::new(2).unwrap(),
                max_retiring: 2,
            },
            frame_rate: golden_time::RationalRate::new(60, 1).unwrap(),
            max_outstanding: NonZeroUsize::new(16).unwrap(),
            max_input_bytes: NonZeroUsize::new(65536).unwrap(),
            max_retained_input_bytes: NonZeroUsize::new(262144).unwrap(),
            recovery_delay: Duration::from_millis(10),
            max_recovery_delay: Duration::from_millis(100),
            notifier: None,
        };
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
        let root = scene.root;
        let routes = || {
            vec![ChannelRoute {
                output: OutputId::new(1).unwrap(),
                channel: root,
            }]
        };
        let first = renderer.publish(capture, routes()).unwrap();
        installed(&renderer, first);
        let until = Instant::now() + Duration::from_secs(4);
        while renderer.snapshot().frames_submitted == 0 {
            assert!(Instant::now() < until, "initial held frame deadline");
            std::thread::sleep(Duration::from_millis(2));
        }
        scene.stop(scope, Caller::Transport);
        scene.check_stopped(scope);
        let capture = capture_channels(&scene.engine, &[scene.root], 16).unwrap();
        assert_eq!(capture.application_demand().len(), usize::from(scope != Scope::All));
        runtime.reconcile(&scene.engine, &capture.application_demand()).unwrap();
        // A held, full in-flight queue may keep the replacement pending. Admission is
        // sufficient here; the native pixel suite proves the installed scoped result.
        renderer.publish(capture, routes()).unwrap();
        assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, original.process_id);
        if scope != Scope::All {
            scene.stop(Scope::All, Caller::Script);
            let capture = capture_channels(&scene.engine, &[scene.root], 16).unwrap();
            assert!(capture.application_demand().is_empty());
            runtime.reconcile(&scene.engine, &[]).unwrap();
            renderer.publish(capture, routes()).unwrap();
        }
        drop(scene);
        runtime.close();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(
            wait(&runtime, ApplicationPhase::Ready).process_id,
            original.process_id,
            "{scope:?}: held final use must preserve the process after Stop All and project close"
        );
        let held = renderer.snapshot().resources.unwrap();
        assert!(held.in_flight > 0);
        assert!(held.last_completed.serial < held.last_submitted.serial);
        renderer.close();
        drop(renderer);
        assert!(
            !retired.wait(Duration::from_millis(100)),
            "{scope:?}: retirement must wait for real final use"
        );
        assert!(control.complete_through(control.snapshot().last_submitted.unwrap()));
        wait(&runtime, ApplicationPhase::Idle);
        let until = Instant::now() + Duration::from_secs(4);
        while !retired.wait(Duration::from_millis(5)) {
            if let Some(token) = control.snapshot().last_submitted {
                control.complete_through(token);
            }
            assert!(Instant::now() < until, "{scope:?}: final retirement deadline");
        }
        drop(runtime);
        assert!(process_retirement.wait_for_idle(Duration::from_secs(4)));
        eprintln!(
            "Scoped process final-use passed: {scope:?}, PID={:?}",
            original.process_id
        );
    }
}
