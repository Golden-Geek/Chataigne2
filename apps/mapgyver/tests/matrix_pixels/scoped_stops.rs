use super::*;
use crate::matrix_controls::{Caller, SCOPES, Scene as ControlScene, Scope};
use mapgyver::applications::{protocol::*, runtime::ApplicationRuntime};

pub(super) fn phase(runtime: &ApplicationRuntime, expected: ApplicationPhase) -> ApplicationRuntimeStatus {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = runtime.snapshot();
        if let Some(status) = snapshot.applications.first()
            && status.phase == expected
        {
            return status.clone();
        }
        assert!(Instant::now() < deadline, "expected {expected:?}, got {snapshot:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn expected(target: bool, peer: bool) -> Image {
    let mut final_image = image([0.0; 4]);
    for (enabled, x, color) in [(target, 0.0, [1.0, 0.0, 0.0, 1.0]), (peer, 16.0, [0.0, 0.0, 1.0, 1.0])] {
        if enabled {
            final_image.draw(
                &Image::solid(16, 24, color),
                &InstanceDefinition {
                    transform: Transform2D {
                        position: [x, 0.0],
                        pivot: [0.0; 2],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
        }
    }
    final_image
}

#[test]
fn scoped_native_scene_preserves_authored_state_and_exact_unrelated_requests_for_every_caller() {
    let mut scene = ControlScene::new(None);
    let saved = golden_core::app::to_sparse_project_json_pretty(&scene.engine).unwrap();
    for caller in [Caller::Transport, Caller::Script] {
        for scope in SCOPES {
            scene.play();
            scene.stop(scope, caller);
            scene.check_stopped(scope);
            let capture = mapgyver::media::capture_channels(&scene.engine, &[scene.root], 16).unwrap();
            assert!(capture.application_demand().is_empty());
        }
    }
    assert_eq!(
        golden_core::app::to_sparse_project_json_pretty(&scene.engine).unwrap(),
        saved
    );
}

#[test]
#[ignore = "explicit M13/6 all-scope native pixels and aggregate process/native retirement on both GPUs"]
fn native_scoped_stops_preserve_peer_pixels_aggregate_process_and_balanced_gpu_retirement() {
    let (done, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            completion.recv_timeout(Duration::from_secs(180)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("scoped stop native/process retirement watchdog expired");
            std::process::exit(1);
        }
    });
    let hardware = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        for scope in SCOPES {
            let mut scene = ControlScene::new(Some(ApplicationDefinition {
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
            let demand = mapgyver::media::capture_channels(&scene.engine, &[scene.root], 16)
                .unwrap()
                .application_demand();
            assert_eq!(demand.len(), 2);
            runtime.reconcile(&scene.engine, &demand).unwrap();
            let original = phase(&runtime, ApplicationPhase::Ready);
            assert_eq!(runtime.snapshot().applications.len(), 1);
            assert_eq!(original.consumers.len(), 2);
            let mut fixture = Fixture::new(adapter.id);
            fixture.renderer().bind_applications(runtime.plan_retainer());
            let control = fixture.renderer().native_control().unwrap().clone();
            fixture.publish_channels(&scene.engine, &[scene.root, scene.root], scene.root, None);
            fixture.pixels(adapter.id, &expected(true, true), "scoped-initial-independent-clips");
            scene.stop(scope, Caller::Transport);
            scene.check_stopped(scope);
            let demand = mapgyver::media::capture_channels(&scene.engine, &[scene.root], 16)
                .unwrap()
                .application_demand();
            assert_eq!(demand.len(), usize::from(scope != Scope::All));
            runtime.reconcile(&scene.engine, &demand).unwrap();
            fixture.publish_channels(&scene.engine, &[scene.root, scene.root], scene.root, None);
            fixture.pixels(
                adapter.id,
                &expected(false, scope != Scope::All),
                "scoped-target-withdrawn-peer-preserved",
            );
            if scope != Scope::All {
                assert_eq!(phase(&runtime, ApplicationPhase::Ready).process_id, original.process_id);
                scene.stop(Scope::All, Caller::Script);
                runtime.reconcile(&scene.engine, &[]).unwrap();
                fixture.publish_channels(&scene.engine, &[scene.root, scene.root], scene.root, None);
                fixture.pixels(adapter.id, &expected(false, false), "scoped-stop-all-empty");
            }
            phase(&runtime, ApplicationPhase::Idle);
            let before = control.allocations_snapshot();
            assert!(before.plan.live > 0);
            drop(fixture);
            let final_inventory = control.allocations_snapshot();
            assert_eq!(final_inventory.plan.live, 0);
            assert_eq!(final_inventory.plan.created, final_inventory.plan.released);
            assert!(final_inventory.outputs.is_empty() && final_inventory.sources.is_empty());
            drop(scene);
            runtime.close();
            drop(runtime);
            assert!(process_retirement.wait_for_idle(Duration::from_secs(4)));
            eprintln!(
                "Scoped native/process passed adapter={:?} name={} scope={scope:?} PID={:?} final_inventory={final_inventory:?}",
                adapter.id, adapter.description, original.process_id
            );
        }
    }
    done.send(()).unwrap();
}
