use super::*;
use golden_core::app::create_new_project_engine;
use mapgyver::media::composition::protocol::CompositionEdit;

#[test]
#[ignore = "explicit native typed Cell Fit/Fill/Stretch and natural placement qualification"]
fn native_cell_placement_fit_fill_stretch_crop_and_natural_size_on_each_adapter() {
    let hardware = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        let mut engine = create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let red = color(&mut engine, [1.0, 0.0, 0.0, 1.0]);
        let green = color(&mut engine, [0.0, 1.0, 0.0, 1.0]);
        let pattern = create(&mut engine, MediaKind::Composition);
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: pattern.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(8, 4).unwrap(),
                },
            },
        );
        edit(
            &mut engine,
            MediaEdit::Composition {
                reference: pattern,
                edit: CompositionEdit::AddInstances {
                    sources: vec![red, green, red],
                },
            },
        );
        let MediaEditResult::Composition { snapshot } = edit(
            &mut engine,
            MediaEdit::Composition {
                reference: pattern,
                edit: CompositionEdit::Inspect,
            },
        ) else {
            panic!()
        };
        for (instance, (x, width)) in snapshot.instances.into_iter().zip([(0.0, 2.0), (2.0, 4.0), (6.0, 2.0)]) {
            edit(
                &mut engine,
                MediaEdit::Composition {
                    reference: pattern,
                    edit: CompositionEdit::ConfigureInstance {
                        instance: instance.reference.instance,
                        definition: InstanceDefinition {
                            transform: Transform2D {
                                position: [x, 0.0],
                                scale: [width, 4.0],
                                pivot: [0.0; 2],
                                ..Default::default()
                            },
                            ..instance.definition
                        },
                    },
                },
            );
        }
        let root = create(&mut engine, MediaKind::Matrix);
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: root.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(32, 24).unwrap(),
                },
            },
        );
        let MediaEditResult::MatrixCreated { items, .. } = matrix(
            &mut engine,
            root.media,
            MatrixEdit::AddLayers { parent: None, count: 1 },
        ) else {
            panic!()
        };
        let MatrixItemId::Layer { id: layer } = items[0] else {
            panic!()
        };
        matrix(
            &mut engine,
            root.media,
            MatrixEdit::ConfigureLayer {
                layer,
                settings: LayerSettings {
                    size: Extent2D::new(32, 24).unwrap(),
                    ..Default::default()
                },
            },
        );
        let MediaEditResult::MatrixCreated { columns, .. } = matrix(
            &mut engine,
            root.media,
            MatrixEdit::AddColumns { parent: None, count: 1 },
        ) else {
            panic!()
        };
        matrix(
            &mut engine,
            root.media,
            MatrixEdit::DropSource {
                layer,
                column: columns[0],
                source: pattern.media.0,
                copy: false,
            },
        );
        launch(&mut engine, root, columns[0], LaunchMode::Cut);
        let mut source = Image::solid(8, 4, [1.0, 0.0, 0.0, 1.0]);
        source.draw(
            &Image::solid(4, 4, [0.0, 1.0, 0.0, 1.0]),
            &InstanceDefinition {
                transform: Transform2D {
                    position: [2.0, 0.0],
                    pivot: [0.0; 2],
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let mut fixture = Fixture::new(adapter.id);
        for (mode, size, position, scale, label) in [
            (
                MediaFitMode::Fit,
                Some(Extent2D::new(8, 8).unwrap()),
                [4.0, 6.0],
                [1.0, 1.0],
                "cell-fit-letterbox",
            ),
            (
                MediaFitMode::Fill,
                Some(Extent2D::new(8, 8).unwrap()),
                [0.0, 4.0],
                [2.0, 2.0],
                "cell-fill-centered-crop",
            ),
            (
                MediaFitMode::Stretch,
                Some(Extent2D::new(8, 8).unwrap()),
                [4.0, 4.0],
                [1.0, 2.0],
                "cell-stretch",
            ),
            (MediaFitMode::Fill, None, [4.0, 4.0], [1.0, 1.0], "cell-natural-default"),
        ] {
            matrix(
                &mut engine,
                root.media,
                MatrixEdit::ConfigureCell {
                    layer,
                    column: columns[0],
                    settings: CellSettings {
                        position: [4.0, 4.0],
                        size,
                        fit_mode: mode,
                        ..Default::default()
                    },
                },
            );
            let mut expected = image([0.0; 4]);
            expected.draw_clipped(
                &source,
                &InstanceDefinition {
                    transform: Transform2D {
                        position,
                        scale,
                        pivot: [0.0; 2],
                        ..Default::default()
                    },
                    ..Default::default()
                },
                [4.0, 4.0, 12.0, if size.is_some() { 12.0 } else { 8.0 }],
            );
            fixture.publish_channels(&engine, &[root, root], root, None);
            fixture.pixels(adapter.id, &expected, label);
        }
        assert!(engine.undo().unwrap());
        let mut expected = image([0.0; 4]);
        expected.draw(
            &source,
            &InstanceDefinition {
                transform: Transform2D {
                    position: [4.0, 4.0],
                    scale: [1.0, 2.0],
                    pivot: [0.0; 2],
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        fixture.publish_channels(&engine, &[root, root], root, None);
        fixture.pixels(adapter.id, &expected, "cell-placement-undo");
        drop(fixture);
        println!(
            "Cell Fit/Fill/Stretch/natural/Undo pixels passed on {}",
            adapter.description
        );
    }
}
