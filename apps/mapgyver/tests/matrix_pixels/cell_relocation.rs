use super::*;
use crate::matrix_controls::{Caller, Scene as ControlScene, Scope, transfers::Transfers};
use mapgyver::applications::{protocol::*, runtime::ApplicationRuntime};

struct Scene {
    control: ControlScene,
    source: CellId,
    peer: CellId,
    routes: [ChannelReference; 2],
    destination: (LayerId, ColumnId),
}
impl Scene {
    fn new(action: CellTransferAction, application: Option<ApplicationDefinition>) -> Self {
        let mut control = ControlScene::new(application);
        let transfers = Transfers::prepare(&mut control);
        let authored = inspect(&mut control.engine, control.root.media);
        let source = authored.cells.iter().find(|cell| cell.id == transfers.source).unwrap();
        let peer = authored
            .cells
            .iter()
            .find(|cell| ![transfers.source, transfers.copy, transfers.reference].contains(&cell.id))
            .unwrap();
        let rows = [source.layer, peer.layer];
        let routes = rows.map(|row| {
            let item = authored
                .items
                .iter()
                .find(|item| item.id == MatrixItemId::Layer { id: row })
                .unwrap();
            control.matrix(MatrixEdit::ConfigureItem {
                item: item.id,
                settings: CompositeSettings {
                    expose_channel: true,
                    ..item.settings
                },
            });
            item.channel
        });
        let destination = if action == CellTransferAction::Drop {
            (peer.layer, peer.definition.column)
        } else {
            let row = authored
                .items
                .iter()
                .find_map(|item| match item.id {
                    MatrixItemId::Layer { id } if !rows.contains(&id) => Some(id),
                    _ => None,
                })
                .unwrap();
            (row, authored.columns[0].id)
        };
        Self {
            control,
            source: source.id,
            peer: peer.id,
            routes,
            destination,
        }
    }
    fn transfer(&mut self, action: CellTransferAction) {
        self.control.matrix(MatrixEdit::TransferCell {
            cell: self.source,
            layer: self.destination.0,
            column: self.destination.1,
            action,
        });
    }
    fn relaunch(&mut self) {
        let authored = inspect(&mut self.control.engine, self.control.root.media);
        self.control.matrix(MatrixEdit::ClearPlayback);
        for id in [self.source, self.peer] {
            let cell = authored.cells.iter().find(|cell| cell.id == id).unwrap();
            self.control.matrix(MatrixEdit::LaunchCell {
                layer: cell.layer,
                column: cell.definition.column,
                mode: LaunchMode::Cut,
            });
        }
    }
}

fn half(color: [f64; 4], x: f32) -> Image {
    let mut output = image([0.0; 4]);
    output.draw(
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
    output
}
fn pixels(
    fixture: &mut Fixture,
    scene: &Scene,
    adapter: AdapterId,
    action: CellTransferAction,
    relaunched: bool,
    label: &str,
) {
    let red = half([1.0, 0.0, 0.0, 1.0], 0.0);
    let blue = half([0.0, 0.0, 1.0, 1.0], 16.0);
    let empty = image([0.0; 4]);
    let mut combined = image([0.0; 4]);
    combined.draw(
        &Image::solid(16, 24, [1.0, 0.0, 0.0, 1.0]),
        &InstanceDefinition {
            transform: Transform2D {
                pivot: [0.0; 2],
                ..Default::default()
            },
            ..Default::default()
        },
    );
    combined.draw(
        &Image::solid(16, 24, [0.0, 0.0, 1.0, 1.0]),
        &InstanceDefinition {
            transform: Transform2D {
                position: [16.0, 0.0],
                pivot: [0.0; 2],
                ..Default::default()
            },
            ..Default::default()
        },
    );
    let layers = match (relaunched, action) {
        (false, _) => [&red, &blue],
        (true, CellTransferAction::Drop) => [&blue, &red],
        (true, CellTransferAction::Move) => [&empty, &blue],
        _ => panic!(),
    };
    fixture.publish_channels(&scene.control.engine, &scene.routes, scene.control.root, None);
    fixture.pixels_channels(adapter, &layers, &combined, label);
}

#[test]
fn relocation_scene_keeps_current_playback_and_exact_history_then_relaunches_authored_addresses() {
    for action in [CellTransferAction::Drop, CellTransferAction::Move] {
        let mut scene = Scene::new(action, None);
        let before = inspect(&mut scene.control.engine, scene.control.root.media);
        let playback = scene.control.playback();
        let history = scene.control.engine.undo_len();
        scene.transfer(action);
        assert_eq!(scene.control.engine.undo_len(), history + 1);
        assert_eq!(scene.control.playback(), playback);
        let after = inspect(&mut scene.control.engine, scene.control.root.media);
        let moved = after.cells.iter().find(|cell| cell.id == scene.source).unwrap();
        assert_eq!((moved.layer, moved.definition.column), scene.destination);
        assert!(apply_project_intent(&mut scene.control.engine, UiEditIntent::Undo).success);
        assert_eq!(inspect(&mut scene.control.engine, scene.control.root.media), before);
        assert!(apply_project_intent(&mut scene.control.engine, UiEditIntent::Redo).success);
        assert_eq!(inspect(&mut scene.control.engine, scene.control.root.media), after);
        assert_eq!(scene.control.playback(), playback);
        scene.relaunch();
        assert_eq!(inspect(&mut scene.control.engine, scene.control.root.media), after);
        let saved = golden_core::app::to_sparse_project_json_pretty(&scene.control.engine).unwrap();
        let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&saved).unwrap();
        golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
        assert_eq!(inspect(&mut loaded, scene.control.root.media), after);
    }
}

#[test]
#[ignore = "explicit M13/7 native move/swap, playing Layer scopes, history/reload and process/GPU retirement on both GPUs"]
fn native_cell_move_and_swap_preserve_playing_scopes_until_relaunch_and_retire_exactly() {
    let (done, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            completion.recv_timeout(Duration::from_secs(180)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Cell relocation native watchdog expired");
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
        for action in [CellTransferAction::Drop, CellTransferAction::Move] {
            let mut scene = Scene::new(
                action,
                Some(ApplicationDefinition {
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
                }),
            );
            let reconcile = |runtime: &mut ApplicationRuntime, scene: &Scene| {
                let capture =
                    mapgyver::media::capture_channels(&scene.control.engine, &[scene.control.root], 16).unwrap();
                let demand = capture.application_demand();
                assert_eq!(demand.len(), 2);
                runtime.reconcile(&scene.control.engine, &demand).unwrap();
                demand.into_iter().map(|entry| entry.application).collect::<Vec<_>>()
            };
            let mut runtime = ApplicationRuntime::start(&scene.control.engine, None).unwrap();
            let retirement = runtime.retirement();
            let owners = reconcile(&mut runtime, &scene);
            let original = super::scoped_stops::phase(&runtime, ApplicationPhase::Ready);
            let mut fixture = Fixture::new(adapter.id);
            fixture.renderer().bind_applications(runtime.plan_retainer());
            let control = fixture.renderer().native_control().unwrap().clone();
            pixels(
                &mut fixture,
                &scene,
                adapter.id,
                action,
                false,
                "relocation-initial-scopes",
            );
            let before = inspect(&mut scene.control.engine, scene.control.root.media);
            let playback = scene.control.playback();
            let history = scene.control.engine.undo_len();
            scene.transfer(action);
            assert_eq!(scene.control.engine.undo_len(), history + 1);
            assert_eq!(scene.control.playback(), playback);
            assert_eq!(reconcile(&mut runtime, &scene), owners);
            pixels(
                &mut fixture,
                &scene,
                adapter.id,
                action,
                false,
                "relocation-playing-scopes-preserved",
            );
            let after = inspect(&mut scene.control.engine, scene.control.root.media);
            for (intent, expected, label) in [
                (UiEditIntent::Undo, &before, "relocation-undo"),
                (UiEditIntent::Redo, &after, "relocation-redo"),
            ] {
                assert!(apply_project_intent(&mut scene.control.engine, intent).success);
                assert_eq!(inspect(&mut scene.control.engine, scene.control.root.media), *expected);
                assert_eq!(scene.control.playback(), playback);
                assert_eq!(reconcile(&mut runtime, &scene), owners);
                pixels(&mut fixture, &scene, adapter.id, action, false, label);
            }
            scene.relaunch();
            assert_eq!(reconcile(&mut runtime, &scene), owners);
            pixels(
                &mut fixture,
                &scene,
                adapter.id,
                action,
                true,
                "relocation-fresh-launch-new-addresses",
            );
            assert_eq!(
                super::scoped_stops::phase(&runtime, ApplicationPhase::Ready).process_id,
                original.process_id
            );
            let saved = golden_core::app::to_sparse_project_json_pretty(&scene.control.engine).unwrap();
            let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&saved).unwrap();
            golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
            assert!(runtime.reconcile(&loaded, &[]).is_err());
            drop(fixture);
            let old_inventory = control.allocations_snapshot();
            assert_eq!(old_inventory.plan.live, 0);
            assert_eq!(old_inventory.plan.created, old_inventory.plan.released);
            assert!(old_inventory.outputs.is_empty() && old_inventory.sources.is_empty());
            runtime.close();
            drop(runtime);
            assert!(retirement.wait_for_idle(Duration::from_secs(4)));
            scene.control.engine = loaded;
            scene.relaunch();
            let mut runtime = ApplicationRuntime::start(&scene.control.engine, None).unwrap();
            let retirement = runtime.retirement();
            assert_eq!(reconcile(&mut runtime, &scene), owners);
            let reloaded = super::scoped_stops::phase(&runtime, ApplicationPhase::Ready);
            let mut fixture = Fixture::new(adapter.id);
            fixture.renderer().bind_applications(runtime.plan_retainer());
            let control = fixture.renderer().native_control().unwrap().clone();
            pixels(
                &mut fixture,
                &scene,
                adapter.id,
                action,
                true,
                "relocation-reload-authored-addresses",
            );
            scene.control.stop(Scope::All, Caller::Script);
            runtime.reconcile(&scene.control.engine, &[]).unwrap();
            fixture.publish_channels(&scene.control.engine, &scene.routes, scene.control.root, None);
            let empty = image([0.0; 4]);
            fixture.pixels_channels(adapter.id, &[&empty, &empty], &empty, "relocation-stop-all-empty");
            super::scoped_stops::phase(&runtime, ApplicationPhase::Idle);
            drop(fixture);
            let inventory = control.allocations_snapshot();
            assert_eq!(inventory.plan.live, 0);
            assert_eq!(inventory.plan.created, inventory.plan.released);
            assert!(inventory.outputs.is_empty() && inventory.sources.is_empty());
            drop(scene);
            runtime.close();
            drop(runtime);
            assert!(retirement.wait_for_idle(Duration::from_secs(4)));
            eprintln!(
                "Cell relocation native passed adapter={:?} name={} action={action:?} PID={:?} reloaded_PID={:?} old_inventory={old_inventory:?} inventory={inventory:?}",
                adapter.id, adapter.description, original.process_id, reloaded.process_id
            );
        }
    }
    done.send(()).unwrap();
}
