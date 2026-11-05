use super::*;
use golden_core::app::create_new_project_engine;

#[test]
#[ignore = "explicit native typed Cell placement/opacity/disabled/Undo qualification"]
fn native_cell_position_opacity_enabled_and_undo_preserve_child_size_on_each_adapter() {
    let (done, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            completion.recv_timeout(Duration::from_secs(90)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Cell native qualification/retirement watchdog expired");
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
        let mut engine = create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let sources = [
            color(&mut engine, [1.0, 0.0, 0.0, 1.0]),
            color(&mut engine, [0.0, 1.0, 0.0, 1.0]),
        ];
        for source in sources {
            edit(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: source.media,
                    size: SizePolicy::Fixed {
                        extent: Extent2D::new(8, 4).unwrap(),
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
        let MediaEditResult::MatrixCreated { columns, .. } = matrix(
            &mut engine,
            root.media,
            MatrixEdit::AddColumns { parent: None, count: 2 },
        ) else {
            panic!()
        };
        matrix(
            &mut engine,
            root.media,
            MatrixEdit::ConfigureItem {
                item: items[0],
                settings: CompositeSettings {
                    opacity: 0.5,
                    ..Default::default()
                },
            },
        );
        for (column, source) in columns.iter().zip(sources) {
            matrix(
                &mut engine,
                root.media,
                MatrixEdit::DropSource {
                    layer,
                    column: *column,
                    source: source.media.0,
                    copy: false,
                },
            );
        }
        let original = CellSettings {
            position: [3.0, 2.0],
            opacity: 0.5,
            ..Default::default()
        };
        matrix(
            &mut engine,
            root.media,
            MatrixEdit::ConfigureCell {
                layer,
                column: columns[0],
                settings: original,
            },
        );
        launch(&mut engine, root, columns[0], LaunchMode::Cut);
        let expected = |rgba, settings: CellSettings| {
            let mut result = image([0.0; 4]);
            result.draw(
                &Image::solid(8, 4, rgba),
                &InstanceDefinition {
                    transform: Transform2D {
                        position: settings.position,
                        pivot: [0.0; 2],
                        ..Default::default()
                    },
                    opacity: settings.opacity * 0.5,
                    ..Default::default()
                },
            );
            result
        };
        let mut fixture = Fixture::new(adapter.id);
        let mut verify = |engine: &AppEngine, rgba, settings, label| {
            fixture.publish_channels(engine, &[root, root], root, None);
            let expected = expected(rgba, settings);
            fixture.pixels_channels(adapter.id, &[&expected, &expected], &expected, label);
        };
        verify(
            &engine,
            [1.0, 0.0, 0.0, 1.0],
            original,
            "cell-native-size-position-opacity",
        );
        let changed = CellSettings {
            position: [7.0, 5.0],
            opacity: 0.75,
            ..Default::default()
        };
        matrix(
            &mut engine,
            root.media,
            MatrixEdit::ConfigureCell {
                layer,
                column: columns[0],
                settings: changed,
            },
        );
        verify(&engine, [1.0, 0.0, 0.0, 1.0], changed, "cell-live-position-opacity");
        assert!(engine.undo().unwrap());
        verify(&engine, [1.0, 0.0, 0.0, 1.0], original, "cell-settings-undo");
        let second = CellSettings {
            position: [1.0, 1.0],
            opacity: 0.5,
            enabled: false,
        };
        matrix(
            &mut engine,
            root.media,
            MatrixEdit::ConfigureCell {
                layer,
                column: columns[1],
                settings: second,
            },
        );
        launch(&mut engine, root, columns[1], LaunchMode::Cut);
        verify(
            &engine,
            [1.0, 0.0, 0.0, 1.0],
            original,
            "cell-disabled-destination-holds",
        );
        let second = CellSettings {
            enabled: true,
            ..second
        };
        matrix(
            &mut engine,
            root.media,
            MatrixEdit::ConfigureCell {
                layer,
                column: columns[1],
                settings: second,
            },
        );
        launch(&mut engine, root, columns[1], LaunchMode::Cut);
        verify(
            &engine,
            [0.0, 1.0, 0.0, 1.0],
            second,
            "cell-enabled-destination-launches",
        );
        drop(fixture);
        println!(
            "Cell position/opacity/enabled/Undo native pixels passed on {}",
            adapter.description
        );
    }
    let _ = done.send(());
}
