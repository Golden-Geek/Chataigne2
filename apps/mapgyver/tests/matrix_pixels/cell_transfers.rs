use super::*;
use crate::matrix_controls::{Caller, Scene as ControlScene, Scope, transfers::Transfers};
use mapgyver::applications::{protocol::*, runtime::ApplicationRuntime};

fn expected(target: Option<[f64; 4]>) -> Image {
    let mut final_image = image([0.0; 4]);
    for (color, x) in [(target, 0.0), (Some([0.0, 0.0, 1.0, 1.0]), 16.0)] {
        if let Some(color) = color {
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

fn publish(fixture: &mut Fixture, scene: &ControlScene, adapter: AdapterId, target: Option<[f64; 4]>, label: &str) {
    fixture.publish_channels(&scene.engine, &[scene.root, scene.root], scene.root, None);
    fixture.pixels(adapter, &expected(target), label);
}

#[test]
fn transfer_native_scene_preserves_exact_owned_and_shared_identities_and_source_recovery() {
    for action in [CellTransferAction::Copy, CellTransferAction::Reference] {
        let mut scene = ControlScene::new(None);
        scene.play();
        let transfers = Transfers::prepare(&mut scene);
        transfers.launch(&mut scene, action);
        scene.stop(Scope::Cell, Caller::Transport);
        transfers.color_source(&mut scene, [0.0, 1.0, 0.0, 1.0]);
        let before = inspect(&mut scene.engine, scene.root.media);
        let copied = before.cells.iter().find(|cell| cell.id == transfers.copy).unwrap();
        let linked = before.cells.iter().find(|cell| cell.id == transfers.reference).unwrap();
        assert_ne!(copied.media, linked.media);
        assert_ne!(
            copied.definition.source.unwrap().channel,
            transfers.source_output.channel
        );
        assert_eq!(
            linked.definition.source.unwrap().channel,
            transfers.source_output.channel
        );
        assert_eq!(linked.definition.application.0, transfers.source.0);
        transfers.clear_source(&mut scene);
        let missing = inspect(&mut scene.engine, scene.root.media);
        assert!(
            missing
                .cells
                .iter()
                .find(|cell| cell.id == transfers.reference)
                .unwrap()
                .definition
                .source
                .is_none()
        );
        assert!(apply_project_intent(&mut scene.engine, UiEditIntent::Undo).success);
        assert_eq!(inspect(&mut scene.engine, scene.root.media), before);
        transfers.remove_source_owner(&mut scene);
        assert!(scene.engine.node_id_by_uuid(transfers.source.0).is_none());
        assert!(apply_project_intent(&mut scene.engine, UiEditIntent::Undo).success);
        assert_eq!(inspect(&mut scene.engine, scene.root.media), before);
    }
}

#[test]
#[ignore = "explicit M13/7 Copy/Reference native pixels, source recovery and process/GPU retirement on both GPUs"]
fn native_cell_transfers_preserve_live_pixels_exact_source_recovery_and_balanced_retirement() {
    let (done, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            completion.recv_timeout(Duration::from_secs(180)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Cell transfer native/process watchdog expired");
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
        for action in [CellTransferAction::Copy, CellTransferAction::Reference] {
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
            let transfers = Transfers::prepare(&mut scene);
            transfers.launch(&mut scene, action);
            let mut runtime = ApplicationRuntime::start(&scene.engine, None).unwrap();
            let process_retirement = runtime.retirement();
            let reconcile = |runtime: &mut ApplicationRuntime, scene: &ControlScene| {
                let capture = mapgyver::media::capture_channels(&scene.engine, &[scene.root], 16).unwrap();
                runtime.reconcile(&scene.engine, &capture.application_demand()).unwrap();
            };
            reconcile(&mut runtime, &scene);
            let original = super::scoped_stops::phase(&runtime, ApplicationPhase::Ready);
            assert_eq!(
                original.consumers.len(),
                if action == CellTransferAction::Copy { 3 } else { 2 }
            );
            let mut fixture = Fixture::new(adapter.id);
            fixture.renderer().bind_applications(runtime.plan_retainer());
            let control = fixture.renderer().native_control().unwrap().clone();
            publish(
                &mut fixture,
                &scene,
                adapter.id,
                Some([1.0, 0.0, 0.0, 1.0]),
                "transfer-running-original-and-duplicate",
            );
            let authored = inspect(&mut scene.engine, scene.root.media);
            let duplicate = authored
                .cells
                .iter()
                .find(|cell| {
                    cell.id
                        == if action == CellTransferAction::Copy {
                            transfers.copy
                        } else {
                            transfers.reference
                        }
                })
                .unwrap()
                .definition
                .source
                .unwrap();
            let channels = fixture.renderer().channels();
            let original_target = channels
                .iter()
                .find(|channel| channel.reference == transfers.source_output)
                .unwrap()
                .target;
            let duplicate_target = channels
                .iter()
                .find(|channel| channel.reference == duplicate)
                .unwrap()
                .target;
            assert_eq!(
                original_target == duplicate_target,
                action == CellTransferAction::Reference,
                "References reuse the physical producer target; Copies own another target"
            );
            scene.stop(Scope::Cell, Caller::Transport);
            reconcile(&mut runtime, &scene);
            publish(
                &mut fixture,
                &scene,
                adapter.id,
                Some([1.0, 0.0, 0.0, 1.0]),
                "transfer-original-stopped-duplicate-survives",
            );
            transfers.color_source(&mut scene, [0.0, 1.0, 0.0, 1.0]);
            let live = if action == CellTransferAction::Copy {
                [1.0, 0.0, 0.0, 1.0]
            } else {
                [0.0, 1.0, 0.0, 1.0]
            };
            publish(
                &mut fixture,
                &scene,
                adapter.id,
                Some(live),
                "transfer-independent-copy-or-linked-live-edit",
            );
            transfers.clear_source(&mut scene);
            reconcile(&mut runtime, &scene);
            let missing = (action == CellTransferAction::Copy).then_some(live);
            publish(
                &mut fixture,
                &scene,
                adapter.id,
                Some(live),
                "transfer-cleared-owned-producer-retained-by-running-clip",
            );
            transfers.launch(&mut scene, action);
            reconcile(&mut runtime, &scene);
            publish(
                &mut fixture,
                &scene,
                adapter.id,
                missing,
                "transfer-fresh-launch-after-content-clear",
            );
            assert!(apply_project_intent(&mut scene.engine, UiEditIntent::Undo).success);
            reconcile(&mut runtime, &scene);
            publish(
                &mut fixture,
                &scene,
                adapter.id,
                Some(live),
                "transfer-content-undo-exact-recovery",
            );
            transfers.remove_source_owner(&mut scene);
            reconcile(&mut runtime, &scene);
            publish(
                &mut fixture,
                &scene,
                adapter.id,
                Some(live),
                "transfer-removed-owner-producer-retained-by-running-clip",
            );
            transfers.launch(&mut scene, action);
            reconcile(&mut runtime, &scene);
            publish(
                &mut fixture,
                &scene,
                adapter.id,
                missing,
                "transfer-fresh-launch-after-owner-removal",
            );
            assert!(apply_project_intent(&mut scene.engine, UiEditIntent::Undo).success);
            reconcile(&mut runtime, &scene);
            publish(
                &mut fixture,
                &scene,
                adapter.id,
                Some(live),
                "transfer-owner-undo-exact-recovery",
            );
            assert_eq!(
                super::scoped_stops::phase(&runtime, ApplicationPhase::Ready).process_id,
                original.process_id
            );
            let saved = golden_core::app::to_sparse_project_json_pretty(&scene.engine).unwrap();
            let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&saved).unwrap();
            golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
            assert!(
                runtime.reconcile(&loaded, &[]).is_err(),
                "old process owner must reject another project instance"
            );
            drop(fixture);
            let old_inventory = control.allocations_snapshot();
            assert_eq!(old_inventory.plan.live, 0);
            assert_eq!(old_inventory.plan.created, old_inventory.plan.released);
            assert!(old_inventory.outputs.is_empty() && old_inventory.sources.is_empty());
            runtime.close();
            drop(runtime);
            assert!(process_retirement.wait_for_idle(Duration::from_secs(4)));
            scene.engine = loaded;
            scene.play();
            transfers.launch(&mut scene, action);
            scene.stop(Scope::Cell, Caller::Transport);
            let mut runtime = ApplicationRuntime::start(&scene.engine, None).unwrap();
            let process_retirement = runtime.retirement();
            reconcile(&mut runtime, &scene);
            let reloaded = super::scoped_stops::phase(&runtime, ApplicationPhase::Ready);
            let mut fixture = Fixture::new(adapter.id);
            fixture.renderer().bind_applications(runtime.plan_retainer());
            let control = fixture.renderer().native_control().unwrap().clone();
            publish(
                &mut fixture,
                &scene,
                adapter.id,
                Some(live),
                "transfer-reload-current-producer-identities",
            );
            scene.stop(Scope::All, Caller::Script);
            runtime.reconcile(&scene.engine, &[]).unwrap();
            fixture.publish_channels(&scene.engine, &[scene.root, scene.root], scene.root, None);
            fixture.pixels(adapter.id, &image([0.0; 4]), "transfer-stop-all-empty");
            super::scoped_stops::phase(&runtime, ApplicationPhase::Idle);
            drop(fixture);
            let inventory = control.allocations_snapshot();
            assert_eq!(inventory.plan.live, 0);
            assert_eq!(inventory.plan.created, inventory.plan.released);
            assert!(inventory.outputs.is_empty() && inventory.sources.is_empty());
            drop(scene);
            runtime.close();
            drop(runtime);
            assert!(process_retirement.wait_for_idle(Duration::from_secs(4)));
            eprintln!(
                "Cell transfer native passed adapter={:?} name={} action={action:?} PID={:?} reloaded_PID={:?} old_inventory={old_inventory:?} inventory={inventory:?}",
                adapter.id, adapter.description, original.process_id, reloaded.process_id
            );
        }
    }
    done.send(()).unwrap();
}
