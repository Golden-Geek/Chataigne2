use super::*;
use crate::matrix_controls::{Caller, Scene, Scope, transfers::Transfers};
use golden_video::{
    OutputId,
    runtime::{NullConfig, VideoRuntime},
};
use mapgyver::{
    matrix::protocol::CellTransferAction,
    media::{
        capture_channels,
        rendering::{ChannelRenderer, ChannelRoute},
    },
};

#[test]
fn running_cell_copy_and_reference_preserve_demand_and_owned_process_until_held_final_use() {
    for action in [CellTransferAction::Copy, CellTransferAction::Reference] {
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
        let transfers = Transfers::prepare(&mut scene);
        assert_ne!(transfers.copy, transfers.reference);
        let initial = capture_channels(&scene.engine, &[scene.root], 16).unwrap();
        assert_eq!(
            initial.application_demand().len(),
            2,
            "authoring must not launch the duplicates"
        );
        transfers.launch(&mut scene, action);
        let capture = capture_channels(&scene.engine, &[scene.root], 16).unwrap();
        let demand = capture.application_demand();
        let applications = if action == CellTransferAction::Copy { 3 } else { 2 };
        assert_eq!(demand.len(), applications);
        let copy_owner = ApplicationId(transfers.copy.0);
        let source_owner = ApplicationId(transfers.source.0);
        assert_eq!(
            demand.iter().filter(|entry| entry.application == source_owner).count(),
            1
        );
        assert_eq!(
            demand.iter().any(|entry| entry.application == copy_owner),
            action == CellTransferAction::Copy
        );
        let mut runtime = ApplicationRuntime::start(&scene.engine, None).unwrap();
        let process_retirement = runtime.retirement();
        runtime.reconcile(&scene.engine, &demand).unwrap();
        let original = wait(&runtime, ApplicationPhase::Ready);
        assert_eq!(
            runtime.snapshot().applications.len(),
            1,
            "equivalent definitions share one actual process"
        );
        assert_eq!(original.consumers.len(), applications);
        scene.stop(Scope::Cell, Caller::Transport);
        transfers.color_source(&mut scene, [0.0, 1.0, 0.0, 1.0]);
        assert!(scene.engine.node_id_by_uuid(transfers.source_output.media.0).is_some());
        transfers.clear_source(&mut scene);
        let after_clear = capture_channels(&scene.engine, &[scene.root], 16).unwrap();
        assert_eq!(
            after_clear.application_demand().len(),
            2,
            "configured source owner remains valid without media"
        );
        runtime
            .reconcile(&scene.engine, &after_clear.application_demand())
            .unwrap();
        assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, original.process_id);
        assert!(apply_project_intent(&mut scene.engine, UiEditIntent::Undo).success);
        transfers.remove_source_owner(&mut scene);
        let missing = capture_channels(&scene.engine, &[scene.root], 16).unwrap();
        let missing_demand = missing.application_demand();
        assert_eq!(missing_demand.len(), 2);
        let retained_source = missing_demand.iter().find(|entry| entry.application == source_owner);
        if action == CellTransferAction::Reference {
            // A running clip retains its exact application identity after authored deletion.
            assert_eq!(retained_source.unwrap().availability, DefinitionAvailability::Missing);
        } else {
            assert!(retained_source.is_none());
        }
        runtime.reconcile(&scene.engine, &missing_demand).unwrap();
        assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, original.process_id);
        assert!(apply_project_intent(&mut scene.engine, UiEditIntent::Undo).success);
        let restored = capture_channels(&scene.engine, &[scene.root], 16).unwrap();
        assert_eq!(restored.application_demand().len(), 2);
        runtime
            .reconcile(&scene.engine, &restored.application_demand())
            .unwrap();
        assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, original.process_id);

        let config = super::scoped_stops::held_config();
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
        let request = renderer.publish(restored, routes()).unwrap();
        super::scoped_stops::installed(&renderer, request);
        let until = Instant::now() + Duration::from_secs(4);
        while renderer.snapshot().frames_submitted == 0 {
            assert!(Instant::now() < until, "transfer held-frame deadline");
            std::thread::sleep(Duration::from_millis(2));
        }
        scene.stop(Scope::All, Caller::Script);
        let stopped = capture_channels(&scene.engine, &[scene.root], 16).unwrap();
        assert!(stopped.application_demand().is_empty());
        runtime.reconcile(&scene.engine, &[]).unwrap();
        renderer.publish(stopped, routes()).unwrap();
        drop(scene);
        runtime.close();
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(wait(&runtime, ApplicationPhase::Ready).process_id, original.process_id);
        let held = renderer.snapshot().resources.unwrap();
        assert!(held.in_flight > 0 && held.last_completed.serial < held.last_submitted.serial);
        renderer.close();
        drop(renderer);
        assert!(!retired.wait(Duration::from_millis(100)));
        let until = Instant::now() + Duration::from_secs(4);
        while !retired.wait(Duration::from_millis(5)) {
            if let Some(token) = control.snapshot().last_submitted {
                control.complete_through(token);
            }
            assert!(Instant::now() < until, "transfer final-use retirement deadline");
        }
        wait(&runtime, ApplicationPhase::Idle);
        drop(runtime);
        assert!(process_retirement.wait_for_idle(Duration::from_secs(4)));
        eprintln!(
            "Cell transfer held completion passed: {action:?}, PID={:?}",
            original.process_id
        );
    }
}
