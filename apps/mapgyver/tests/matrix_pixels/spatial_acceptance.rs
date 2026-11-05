use super::*;
use golden_core::app::{create_new_project_engine, from_sparse_project_json, to_sparse_project_json_pretty};
use golden_video::plan::Point;
use mapgyver::media::spatial::protocol::{SpatialRect, SpatialResizeHandle};

struct SpatialScene {
    root: ChannelReference,
    source: ChannelReference,
    layer: LayerId,
    channel: ChannelReference,
    column: ColumnId,
    cell: CellId,
}

struct ExpectedPlacement {
    dimensions: [usize; 2],
    position: [f32; 2],
    scale: [f32; 2],
    blend: BlendMode,
}

impl SpatialScene {
    fn new(engine: &mut AppEngine) -> Self {
        let source = color(engine, [1.0, 0.0, 0.0, 0.5]);
        edit(
            engine,
            MediaEdit::ConfigureMediaResolution {
                media: source.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(8, 4).unwrap(),
                },
            },
        );
        let blue = color(engine, [0.0, 0.0, 1.0, 0.25]);
        edit(
            engine,
            MediaEdit::ConfigureMediaResolution {
                media: blue.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(32, 24).unwrap(),
                },
            },
        );
        let root = create(engine, MediaKind::Matrix);
        edit(
            engine,
            MediaEdit::ConfigureMediaResolution {
                media: root.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(32, 24).unwrap(),
                },
            },
        );
        let mut parent = None;
        for opacity in [0.4, 0.5] {
            let MediaEditResult::MatrixCreated { items, .. } =
                matrix(engine, root.media, MatrixEdit::AddGroup { parent, label: None })
            else {
                panic!()
            };
            let MatrixItemId::Group { id } = items[0] else { panic!() };
            matrix(
                engine,
                root.media,
                MatrixEdit::ConfigureItem {
                    item: items[0],
                    settings: CompositeSettings {
                        opacity,
                        ..Default::default()
                    },
                },
            );
            parent = Some(id);
        }
        let MediaEditResult::MatrixCreated { items, .. } =
            matrix(engine, root.media, MatrixEdit::AddLayers { parent, count: 2 })
        else {
            panic!()
        };
        let snapshot = inspect(engine, root.media);
        let column = snapshot.columns[0].id;
        for (item, reference, position) in [(items[0], blue, [0.0, 0.0]), (items[1], source, [2.0, 1.0])] {
            let MatrixItemId::Layer { id } = item else { panic!() };
            matrix(
                engine,
                root.media,
                MatrixEdit::ConfigureLayer {
                    layer: id,
                    settings: LayerSettings {
                        position,
                        size: Extent2D::new(32, 24).unwrap(),
                        ..Default::default()
                    },
                },
            );
            matrix(
                engine,
                root.media,
                MatrixEdit::SetCells {
                    assignments: vec![CellAssignment {
                        layer: id,
                        column,
                        source: Some(reference),
                    }],
                },
            );
        }
        let MatrixItemId::Layer { id: layer } = items[1] else {
            panic!()
        };
        matrix(
            engine,
            root.media,
            MatrixEdit::ConfigureItem {
                item: items[1],
                settings: CompositeSettings {
                    opacity: 0.75,
                    expose_channel: true,
                    ..Default::default()
                },
            },
        );
        let snapshot = inspect(engine, root.media);
        let cell = snapshot.cells.iter().find(|cell| cell.layer == layer).unwrap().id;
        let channel = snapshot.items.iter().find(|item| item.id == items[1]).unwrap().channel;
        matrix(
            engine,
            root.media,
            MatrixEdit::ConfigureCell {
                layer,
                column,
                settings: CellSettings {
                    position: [3.0, 2.0],
                    size: Some(Extent2D::new(16, 8).unwrap()),
                    opacity: 0.5,
                    ..Default::default()
                },
            },
        );
        launch(engine, root, column, LaunchMode::Cut);
        Self {
            root,
            source,
            layer,
            channel,
            column,
            cell,
        }
    }

    fn settings(&self, engine: &mut AppEngine, column: ColumnId) -> CellSettings {
        inspect(engine, self.root.media)
            .cells
            .into_iter()
            .find(|cell| cell.layer == self.layer && cell.definition.column == column)
            .unwrap()
            .definition
            .settings
    }

    fn check(
        &self,
        engine: &AppEngine,
        fixture: &mut Fixture,
        adapter: AdapterId,
        expected: ExpectedPlacement,
        label: &str,
    ) {
        let ExpectedPlacement {
            dimensions,
            position,
            scale,
            blend,
        } = expected;
        let source = Image::solid(dimensions[0], dimensions[1], [1.0, 0.0, 0.0, 0.5]);
        let mut layer = image([0.0; 4]);
        layer.draw(
            &source,
            &InstanceDefinition {
                transform: Transform2D {
                    position,
                    scale,
                    pivot: [0.0; 2],
                    ..Default::default()
                },
                opacity: 0.5 * 0.75,
                ..Default::default()
            },
        );
        let blue = image([0.0, 0.0, 1.0, 0.25]);
        let mut scratch = image([0.0; 4]);
        draw(&mut scratch, &blue, 1.0, BlendMode::Normal);
        scratch.draw(
            &layer,
            &InstanceDefinition {
                transform: Transform2D {
                    position: [2.0, 1.0],
                    pivot: [0.0; 2],
                    ..Default::default()
                },
                blend,
                ..Default::default()
            },
        );
        let mut inner = image([0.0; 4]);
        draw(&mut inner, &scratch, 0.5, BlendMode::Normal);
        let mut outer = image([0.0; 4]);
        draw(&mut outer, &inner, 0.4, BlendMode::Normal);
        let mut final_image = image([0.0; 4]);
        draw(&mut final_image, &outer, 1.0, BlendMode::Normal);
        fixture.publish_channels(engine, &[self.channel, self.root], self.root, None);
        fixture.pixels_channels(adapter, &[&layer, &final_image], &final_image, label);
        bounded(fixture);
    }
}

#[test]
#[ignore = "explicit M12/7 live spatial geometry, nested alpha/blend, source resize, copy and reload pixels"]
fn native_spatial_live_geometry_source_resize_copy_reload_and_inactive_editor_on_each_adapter() {
    let adapters = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!adapters.is_empty());
    for adapter in adapters {
        let mut engine = create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let scene = SpatialScene::new(&mut engine);
        let mut fixture = Fixture::new(adapter.id);
        for blend in [
            BlendMode::Normal,
            BlendMode::Add,
            BlendMode::Multiply,
            BlendMode::Screen,
        ] {
            matrix(
                &mut engine,
                scene.root.media,
                MatrixEdit::ConfigureItem {
                    item: MatrixItemId::Layer { id: scene.layer },
                    settings: CompositeSettings {
                        opacity: 0.75,
                        expose_channel: true,
                        blend,
                        ..Default::default()
                    },
                },
            );
            scene.check(
                &engine,
                &mut fixture,
                adapter.id,
                ExpectedPlacement {
                    dimensions: [8, 4],
                    position: [3.0, 2.0],
                    scale: [2.0, 2.0],
                    blend,
                },
                &format!("spatial-nested-{blend:?}"),
            );
        }
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: scene.source.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(16, 4).unwrap(),
                },
            },
        );
        assert_eq!(
            scene.settings(&mut engine, scene.column).size,
            Some(Extent2D::new(16, 8).unwrap())
        );
        scene.check(
            &engine,
            &mut fixture,
            adapter.id,
            ExpectedPlacement {
                dimensions: [16, 4],
                position: [3.0, 4.0],
                scale: [1.0, 1.0],
                blend: BlendMode::Screen,
            },
            "spatial-source-resize-fixed-Fit",
        );
        let natural = CellSettings {
            size: None,
            ..scene.settings(&mut engine, scene.column)
        };
        matrix(
            &mut engine,
            scene.root.media,
            MatrixEdit::ConfigureCell {
                layer: scene.layer,
                column: scene.column,
                settings: natural,
            },
        );
        scene.check(
            &engine,
            &mut fixture,
            adapter.id,
            ExpectedPlacement {
                dimensions: [16, 4],
                position: [3.0, 2.0],
                scale: [1.0, 1.0],
                blend: BlendMode::Screen,
            },
            "spatial-natural-source-resize",
        );
        matrix(
            &mut engine,
            scene.root.media,
            MatrixEdit::EditCellGeometry {
                cell: scene.cell,
                edit: CellGeometryEdit::Translate {
                    origin: natural.position,
                    start: Point { x: 0.0, y: 0.0 },
                    point: Point { x: 4.0, y: 3.0 },
                },
            },
        );
        assert_eq!(scene.settings(&mut engine, scene.column).size, None);
        scene.check(
            &engine,
            &mut fixture,
            adapter.id,
            ExpectedPlacement {
                dimensions: [16, 4],
                position: [7.0, 5.0],
                scale: [1.0, 1.0],
                blend: BlendMode::Screen,
            },
            "spatial-live-translate",
        );
        matrix(
            &mut engine,
            scene.root.media,
            MatrixEdit::EditCellGeometry {
                cell: scene.cell,
                edit: CellGeometryEdit::Resize {
                    origin: SpatialRect {
                        position: [7.0, 5.0],
                        size: Extent2D::new(16, 4).unwrap(),
                    },
                    handle: SpatialResizeHandle::TopLeft,
                    start: Point { x: 0.0, y: 0.0 },
                    point: Point { x: -8.0, y: -2.0 },
                },
            },
        );
        let resized = scene.settings(&mut engine, scene.column);
        assert_eq!(resized.position, [-1.0, 3.0]);
        assert_eq!(resized.size, Some(Extent2D::new(24, 6).unwrap()));
        scene.check(
            &engine,
            &mut fixture,
            adapter.id,
            ExpectedPlacement {
                dimensions: [16, 4],
                position: [-1.0, 3.0],
                scale: [1.5, 1.5],
                blend: BlendMode::Screen,
            },
            "spatial-live-resize",
        );
        assert!(engine.undo().unwrap());
        scene.check(
            &engine,
            &mut fixture,
            adapter.id,
            ExpectedPlacement {
                dimensions: [16, 4],
                position: [7.0, 5.0],
                scale: [1.0, 1.0],
                blend: BlendMode::Screen,
            },
            "spatial-live-resize-Undo",
        );
        assert!(engine.redo().unwrap());
        scene.check(
            &engine,
            &mut fixture,
            adapter.id,
            ExpectedPlacement {
                dimensions: [16, 4],
                position: [-1.0, 3.0],
                scale: [1.5, 1.5],
                blend: BlendMode::Screen,
            },
            "spatial-live-resize-Redo",
        );
        let MediaEditResult::MatrixCreated { columns, .. } = matrix(
            &mut engine,
            scene.root.media,
            MatrixEdit::DuplicateColumn { column: scene.column },
        ) else {
            panic!()
        };
        let copied = columns[0];
        assert_eq!(scene.settings(&mut engine, copied), resized);
        launch(&mut engine, scene.root, copied, LaunchMode::Cut);
        scene.check(
            &engine,
            &mut fixture,
            adapter.id,
            ExpectedPlacement {
                dimensions: [16, 4],
                position: [-1.0, 3.0],
                scale: [1.5, 1.5],
                blend: BlendMode::Screen,
            },
            "spatial-copy-live",
        );
        drop(fixture);
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        assert_eq!(scene.settings(&mut loaded, copied), resized);
        launch(&mut loaded, scene.root, copied, LaunchMode::Cut);
        let mut fixture = Fixture::new(adapter.id);
        scene.check(
            &loaded,
            &mut fixture,
            adapter.id,
            ExpectedPlacement {
                dimensions: [16, 4],
                position: [-1.0, 3.0],
                scale: [1.5, 1.5],
                blend: BlendMode::Screen,
            },
            "spatial-reload-live",
        );
        matrix(
            &mut loaded,
            scene.root.media,
            MatrixEdit::ConfigureCell {
                layer: scene.layer,
                column: scene.column,
                settings: CellSettings {
                    enabled: false,
                    ..resized
                },
            },
        );
        fixture.publish_editor(
            &loaded,
            mapgyver::preview::protocol::PreviewSource::MatrixCell {
                media: scene.root.media,
                cell: scene.cell,
            },
        );
        let mut expected = image([0.0; 4]);
        expected.draw(
            &Image::solid(16, 4, [1.0, 0.0, 0.0, 0.5]),
            &InstanceDefinition {
                transform: Transform2D {
                    position: [-1.0, 3.0],
                    scale: [1.5, 1.5],
                    pivot: [0.0; 2],
                    ..Default::default()
                },
                opacity: 0.5,
                ..Default::default()
            },
        );
        fixture.pixels(
            adapter.id,
            &expected,
            "spatial-inactive-original-editor-after-copy-reload",
        );
        let MediaEditResult::MatrixPlayback { snapshot } =
            matrix(&mut loaded, scene.root.media, MatrixEdit::InspectPlayback)
        else {
            panic!()
        };
        assert_eq!(snapshot.desired_column, Some(copied));
        drop(fixture);
        println!(
            "M12/7 live/nested/source-resize/copy/reload/inactive-editor pixels passed on {}",
            adapter.description
        );
    }
}
