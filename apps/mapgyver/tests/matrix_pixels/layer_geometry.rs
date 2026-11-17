use super::*;
use golden_core::app::create_new_project_engine;
use golden_core::node::Node;

fn alpha(fixture: &Fixture, adapter: AdapterId) -> u8 {
    let budget = golden_io::PayloadPool::new("layer-stop-readback", 3, 1024 * 1024).unwrap();
    let mut probe = golden_video::native::SpoutProbe::new(fixture.sender(0), adapter, budget).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(image) = probe.capture_pixels().unwrap() {
            return image.pixels()[4 * 32 + 5][3];
        }
        assert!(Instant::now() < deadline, "Layer Stop native alpha readback timeout");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[ignore = "explicit native Layer position/canvas and Undo qualification"]
fn native_layer_position_canvas_and_undo_preserve_source_pixel_size_on_each_adapter() {
    let (done, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            completion.recv_timeout(Duration::from_secs(90)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Layer native qualification/retirement watchdog expired");
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
        let red = color(&mut engine, [1.0, 0.0, 0.0, 1.0]);
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: red.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(8, 4).unwrap(),
                },
            },
        );
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
                source: red.media.0,
                copy: false,
            },
        );
        let settings = LayerSettings {
            position: [4.0, 3.0],
            size: Extent2D::new(8, 4).unwrap(),
            ..Default::default()
        };
        matrix(&mut engine, root.media, MatrixEdit::ConfigureLayer { layer, settings });
        launch(&mut engine, root, columns[0], LaunchMode::Cut);
        let expected = |width, height| {
            let mut result = image([0.0; 4]);
            result.draw(
                &Image::solid(width, height, [1.0, 0.0, 0.0, 1.0]),
                &InstanceDefinition {
                    transform: Transform2D {
                        position: [4.0, 3.0],
                        pivot: [0.0; 2],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            );
            result
        };
        let mut fixture = Fixture::new(adapter.id);
        fixture.publish_channels(&engine, &[root, root], root, None);
        let large = expected(8, 4);
        fixture.pixels_channels(adapter.id, &[&large, &large], &large, "layer-position-canvas");
        matrix(
            &mut engine,
            root.media,
            MatrixEdit::ConfigureLayer {
                layer,
                settings: LayerSettings {
                    size: Extent2D::new(4, 2).unwrap(),
                    ..settings
                },
            },
        );
        fixture.publish_channels(&engine, &[root, root], root, None);
        let small = expected(4, 2);
        fixture.pixels_channels(adapter.id, &[&small, &small], &small, "layer-canvas-resize");
        assert!(engine.undo().unwrap());
        fixture.publish_channels(&engine, &[root, root], root, None);
        fixture.pixels_channels(adapter.id, &[&large, &large], &large, "layer-canvas-undo");
        matrix(
            &mut engine,
            root.media,
            MatrixEdit::ConfigureColumn {
                column: columns[0],
                definition: ColumnDefinition {
                    transition: Some(TransitionDefinition {
                        duration_ms: 4000,
                        curve: TransitionCurve::Linear,
                    }),
                },
            },
        );
        let owner = engine.node_id_by_uuid(layer.0).unwrap();
        let folder = engine
            .nodes
            .iter()
            .find(|(_, node)| node.node_data().parent == Some(owner) && node.node_data().meta.decl_id.0 == "parameters")
            .unwrap()
            .0;
        let stop = engine
            .nodes
            .iter()
            .find(|(_, node)| {
                node.node_data().parent == Some(folder) && node.node_data().meta.decl_id.0 == "parameters/stop"
            })
            .unwrap()
            .0;
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: stop,
                value: golden_core::parameter::ParamValue::Trigger(),
                behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
        golden_core::application::service_script_app_operations(&mut engine).unwrap();
        fixture.publish_channels(&engine, &[root, root], root, None);
        let first = alpha(&fixture, adapter.id);
        std::thread::sleep(Duration::from_millis(300));
        let second = alpha(&fixture, adapter.id);
        assert!(first > 0 && first < 255, "expected a partial outgoing alpha: {first}");
        assert!(
            second < first.saturating_sub(5),
            "native fade must advance: {first} -> {second}"
        );
        std::thread::sleep(Duration::from_millis(4000));
        fixture.publish_channels(&engine, &[root, root], root, None);
        let blank = image([0.0; 4]);
        fixture.pixels_channels(adapter.id, &[&blank, &blank], &blank, "layer-stop-completed");
        println!(
            "Layer Stop native alpha {first}->{second}->0 on {}",
            adapter.description
        );
        drop(fixture);
        println!(
            "Layer position/canvas/resize/Undo native pixels passed on {}",
            adapter.description
        );
    }
    let _ = done.send(());
}
