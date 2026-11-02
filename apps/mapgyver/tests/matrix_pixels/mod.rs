use crate::support::{native_pixels::Fixture, raster::Image};
use golden_core::{application::apply_project_intent, ui_sync::UiEditIntent};
use golden_video::{Extent2D, native::AdapterId};
use mapgyver::{
    app::{AppEngine, AppNode},
    matrix::protocol::*,
    media::{
        capture_channels_at, channel,
        composition::protocol::{BlendMode, InstanceDefinition, Transform2D},
        protocol::*,
        rendering::ChannelRoute,
    },
};
use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

mod transitions;

fn edit(engine: &mut AppEngine, operation: MediaEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(operation).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}
fn matrix(engine: &mut AppEngine, media: MediaId, edit_value: MatrixEdit) -> MediaEditResult {
    edit(
        engine,
        MediaEdit::Matrix {
            media,
            edit: edit_value,
        },
    )
}
fn inspect(engine: &mut AppEngine, media: MediaId) -> MatrixSnapshot {
    let MediaEditResult::Matrix { snapshot } = matrix(engine, media, MatrixEdit::Inspect) else {
        panic!()
    };
    snapshot
}
fn create(engine: &mut AppEngine, kind: MediaKind) -> ChannelReference {
    let MediaEditResult::Created { reference } = edit(
        engine,
        MediaEdit::CreateMedia {
            media_kind: kind,
            label: None,
        },
    ) else {
        panic!()
    };
    reference
}
fn color(engine: &mut AppEngine, rgba: [f32; 4]) -> ChannelReference {
    let reference = create(engine, MediaKind::SolidColor);
    let mut definition = channel(engine, reference).unwrap().definition;
    definition.source = ChannelSource::SolidColor { rgba };
    let size = SizePolicy::Fixed {
        extent: Extent2D::new(1, 1).unwrap(),
    };
    edit(engine, MediaEdit::ConfigureChannel { reference, definition });
    edit(
        engine,
        MediaEdit::ConfigureMediaResolution {
            media: reference.media,
            size,
        },
    );
    reference
}
fn launch(engine: &mut AppEngine, root: ChannelReference, column: ColumnId, mode: LaunchMode) {
    matrix(engine, root.media, MatrixEdit::LaunchColumn { column, mode });
}
fn image(color: [f64; 4]) -> Image {
    Image::solid(32, 24, color)
}
fn draw(target: &mut Image, source: &Image, opacity: f32, blend: BlendMode) {
    target.draw(
        source,
        &InstanceDefinition {
            opacity,
            blend,
            transform: Transform2D {
                pivot: [0.0, 0.0],
                ..Default::default()
            },
            ..Default::default()
        },
    );
}

struct Scene {
    root: ChannelReference,
    red: ChannelReference,
    group_channel: ChannelReference,
    first_channel: ChannelReference,
    second: MatrixItemId,
    outside: MatrixItemId,
    columns: Vec<ColumnId>,
    blend: BlendMode,
    outside_first: bool,
}
impl Scene {
    fn new(engine: &mut AppEngine) -> Self {
        let red = color(engine, [1.0, 0.0, 0.0, 0.5]);
        let green = color(engine, [0.0, 1.0, 0.0, 0.75]);
        let blue = color(engine, [0.0, 0.0, 1.0, 0.25]);
        let root = create(engine, MediaKind::Matrix);
        matrix(
            engine,
            root.media,
            MatrixEdit::Configure {
                definition: MatrixDefinition {
                    canvas: Extent2D::new(32, 24).unwrap(),
                    transition: TransitionDefinition {
                        duration_ms: 4000,
                        curve: TransitionCurve::Linear,
                    },
                    ..Default::default()
                },
            },
        );
        let MediaEditResult::MatrixCreated { items, .. } = matrix(
            engine,
            root.media,
            MatrixEdit::AddGroup {
                parent: None,
                label: None,
            },
        ) else {
            panic!()
        };
        let group = items[0];
        let MatrixItemId::Group { id } = group else { panic!() };
        let MediaEditResult::MatrixCreated { items: layers, .. } = matrix(
            engine,
            root.media,
            MatrixEdit::AddLayers {
                parent: Some(id),
                count: 2,
            },
        ) else {
            panic!()
        };
        let MediaEditResult::MatrixCreated { items: outside, .. } =
            matrix(engine, root.media, MatrixEdit::AddLayers { parent: None, count: 1 })
        else {
            panic!()
        };
        matrix(engine, root.media, MatrixEdit::AddColumns { count: 3 });
        let snapshot = inspect(engine, root.media);
        let columns = snapshot.columns.iter().map(|c| c.id).collect::<Vec<_>>();
        let group_channel = snapshot.items.iter().find(|item| item.id == group).unwrap().channel;
        let first_channel = snapshot.items.iter().find(|item| item.id == layers[0]).unwrap().channel;
        for (item, opacity) in [(group, 0.5), (layers[0], 0.75), (layers[1], 0.6)] {
            matrix(
                engine,
                root.media,
                MatrixEdit::ConfigureItem {
                    item,
                    settings: CompositeSettings {
                        opacity,
                        ..Default::default()
                    },
                },
            );
        }
        let mut assignments = Vec::new();
        for (column, sources) in [(columns[0], [red, green, blue]), (columns[1], [green, red, blue])] {
            for (item, source) in [layers[0], layers[1], outside[0]].into_iter().zip(sources) {
                let MatrixItemId::Layer { id } = item else { panic!() };
                assignments.push(CellAssignment {
                    layer: id,
                    column,
                    source: Some(source),
                });
            }
        }
        matrix(engine, root.media, MatrixEdit::SetCells { assignments });
        launch(engine, root, columns[0], LaunchMode::Cut);
        Self {
            root,
            red,
            group_channel,
            first_channel,
            second: layers[1],
            outside: outside[0],
            columns,
            blend: BlendMode::Normal,
            outside_first: false,
        }
    }
    fn routes(&self) -> [ChannelReference; 3] {
        [self.first_channel, self.group_channel, self.root]
    }
    fn expected(&self, missing_red: bool) -> [Image; 3] {
        let red = if missing_red {
            image([0.0; 4])
        } else {
            image([1.0, 0.0, 0.0, 0.5])
        };
        let green = image([0.0, 1.0, 0.0, 0.75]);
        let blue = image([0.0, 0.0, 1.0, 0.25]);
        let mut first = image([0.0; 4]);
        draw(&mut first, &red, 0.75, BlendMode::Normal);
        let mut second = image([0.0; 4]);
        draw(&mut second, &green, 0.6, BlendMode::Normal);
        let mut scratch = image([0.0; 4]);
        draw(&mut scratch, &first, 1.0, BlendMode::Normal);
        draw(&mut scratch, &second, 1.0, self.blend);
        let mut group = image([0.0; 4]);
        draw(&mut group, &scratch, 0.5, BlendMode::Normal);
        let mut final_image = image([0.0; 4]);
        if self.outside_first {
            draw(&mut final_image, &blue, 1.0, BlendMode::Normal);
        }
        draw(&mut final_image, &group, 1.0, BlendMode::Normal);
        if !self.outside_first {
            draw(&mut final_image, &blue, 1.0, BlendMode::Normal);
        }
        [first, group, final_image]
    }
    fn check(&self, fixture: &mut Fixture, engine: &AppEngine, adapter: AdapterId, missing: bool, label: &str) {
        fixture.publish_channels(engine, &self.routes(), self.root, None);
        let images = self.expected(missing);
        fixture.pixels_channels(adapter, &images.iter().collect::<Vec<_>>(), &images[2], label);
        bounded(fixture);
    }
}

fn bounded(fixture: &mut Fixture) {
    let snapshot = fixture.renderer().snapshot();
    let resources = snapshot.resources.unwrap();
    assert!(resources.bytes_used <= resources.byte_limit);
    assert!(resources.in_flight <= 2 && resources.retiring_generations <= 2 && resources.prepared_generations <= 1);
    assert!(snapshot.outstanding <= 8);
    assert!(fixture.renderer().channels().len() <= 16);
}

fn large_and_burst(engine: &mut AppEngine, scene: &Scene, fixture: &mut Fixture, adapter: AdapterId) {
    let started = Instant::now();
    matrix(
        engine,
        scene.root.media,
        MatrixEdit::AddLayers {
            parent: None,
            count: 997,
        },
    );
    matrix(engine, scene.root.media, MatrixEdit::AddColumns { count: 997 });
    let snapshot = inspect(engine, scene.root.media);
    assert_eq!(snapshot.columns.len(), 1000);
    assert_eq!(
        snapshot
            .items
            .iter()
            .filter(|item| matches!(item.id, MatrixItemId::Layer { .. }))
            .count(),
        1000
    );
    assert_eq!(snapshot.cells.len(), 6, "large grid remains sparse");
    scene.check(fixture, engine, adapter, false, "large-sparse-grid");
    let history = engine.undo_len();
    let mut accepted = 0;
    let burst_started = Instant::now();
    let routes = scene.routes();
    for index in 0..1000 {
        std::thread::sleep(
            (burst_started + Duration::from_millis(index as u64 * 10)).saturating_duration_since(Instant::now()),
        );
        launch(engine, scene.root, scene.columns[index % 2], LaunchMode::Configured);
        let renderer = fixture.renderer();
        let capture = capture_channels_at(engine, &routes, 64, renderer.read_time().unwrap()).unwrap();
        let outputs = routes
            .iter()
            .enumerate()
            .map(|(i, channel)| ChannelRoute {
                output: golden_video::OutputId::new(i as u64 + 1).unwrap(),
                channel: *channel,
            })
            .collect();
        if renderer.publish(capture, outputs).is_ok() {
            accepted += 1;
        }
        bounded(fixture);
    }
    let launch_span = burst_started.elapsed();
    assert!(accepted > 0 && accepted <= 8);
    assert_eq!(engine.undo_len(), history, "launches never create authored history");
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut outcomes = Vec::new();
    while outcomes.len() < accepted {
        fixture
            .renderer()
            .drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        assert!(Instant::now() < deadline, "burst outcomes did not drain");
        std::thread::sleep(Duration::from_millis(5));
    }
    launch(engine, scene.root, scene.columns[0], LaunchMode::Cut);
    scene.check(fixture, engine, adapter, false, "burst-recovered");
    eprintln!(
        "Matrix large grid 1000x1000 cells=6 configured_launch_requests=1000 scheduled_period_ms=10 admitted={accepted} launch_span_ms={} burst_and_recovery_ms={} elapsed_ms={}",
        launch_span.as_millis(),
        burst_started.elapsed().as_millis(),
        started.elapsed().as_millis()
    );
}

#[test]
#[ignore = "explicit M07 native Matrix output/preview raster, transition and large-grid acceptance"]
fn native_matrix_pixels_cover_exports_blends_order_recovery_scale_and_interruption() {
    let (done, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            completion.recv_timeout(Duration::from_secs(180)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Matrix qualification/retirement watchdog expired");
            std::process::exit(1);
        }
    });
    let hardware = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|a| !a.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let mut scene = Scene::new(&mut engine);
        let mut fixture = Fixture::with_outputs(adapter.id, 3);
        for (blend, label) in [
            (BlendMode::Normal, "normal"),
            (BlendMode::Add, "add"),
            (BlendMode::Multiply, "multiply"),
            (BlendMode::Screen, "screen"),
        ] {
            scene.blend = blend;
            matrix(
                &mut engine,
                scene.root.media,
                MatrixEdit::ConfigureItem {
                    item: scene.second,
                    settings: CompositeSettings {
                        opacity: 0.6,
                        blend,
                        ..Default::default()
                    },
                },
            );
            scene.check(&mut fixture, &engine, adapter.id, false, label);
        }
        matrix(
            &mut engine,
            scene.root.media,
            MatrixEdit::MoveItem {
                item: scene.outside,
                parent: None,
                after: None,
            },
        );
        scene.outside_first = true;
        scene.check(&mut fixture, &engine, adapter.id, false, "reordered");
        assert!(engine.undo().unwrap());
        scene.outside_first = false;
        scene.check(&mut fixture, &engine, adapter.id, false, "undo-order");
        edit(&mut engine, MediaEdit::RemoveMedia { media: scene.red.media });
        scene.check(&mut fixture, &engine, adapter.id, true, "missing-source");
        assert!(engine.undo().unwrap());
        scene.check(&mut fixture, &engine, adapter.id, false, "undo-source");
        launch(&mut engine, scene.root, scene.columns[2], LaunchMode::Cut);
        fixture.publish_channels(&engine, &scene.routes(), scene.root, None);
        let transparent = image([0.0; 4]);
        fixture.pixels_channels(
            adapter.id,
            &[&transparent, &transparent, &transparent],
            &transparent,
            "empty-column",
        );
        launch(&mut engine, scene.root, scene.columns[0], LaunchMode::Cut);
        large_and_burst(&mut engine, &scene, &mut fixture, adapter.id);
        drop(fixture);
        let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json).unwrap();
        loaded.apply_edits().unwrap();
        launch(&mut loaded, scene.root, scene.columns[0], LaunchMode::Cut);
        let mut fixture = Fixture::with_outputs(adapter.id, 3);
        scene.check(&mut fixture, &loaded, adapter.id, false, "reload-recreate");
        drop(fixture);
        transitions::check(adapter.id);
        eprintln!(
            "M07 native Matrix adapter={:?} description={} completed",
            adapter.id, adapter.description
        );
    }
    done.send(()).unwrap();
}
