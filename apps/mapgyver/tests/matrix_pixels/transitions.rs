use super::*;
use golden_io::PayloadPool;
use golden_video::native::SpoutProbe;

fn sample(name: &str, adapter: AdapterId, budget: &PayloadPool) -> [f64; 3] {
    // SpoutProbe is deliberately a single-image observation, not a streaming receiver.
    // Fresh probes also release all three receiving/staging/CPU credits before the next read.
    let mut probe = SpoutProbe::new(name, adapter, budget.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    let image = loop {
        if let Some(image) = probe.capture_pixels().unwrap() {
            break image;
        }
        assert!(Instant::now() < deadline, "transition native readback timeout");
        std::thread::sleep(Duration::from_millis(5));
    };
    let pixel = image.pixels()[0];
    assert!(
        image.pixels().iter().all(|p| *p == pixel),
        "uniform clip colors must stay spatially uniform"
    );
    assert!(pixel[3] >= 253, "opaque crossfade must retain coverage: {pixel:?}");
    let weights = std::array::from_fn(|c| {
        let v = f64::from(pixel[c]) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    });
    // Three linear RGBA8 producer levels plus the encoded readback can each round.
    assert!(
        (weights.iter().sum::<f64>() - 1.0).abs() <= 4.0 / 255.0,
        "crossfade loses/gains color weight: {weights:?}"
    );
    weights
}

fn sample_outputs(fixture: &Fixture, adapter: AdapterId, budget: &PayloadPool) -> [[f64; 3]; 3] {
    // Each receiver owns its device on its worker. Serial receiver/device startup must not
    // consume the other outputs' fade observation window; no native handle crosses workers.
    std::thread::scope(|scope| {
        let workers: [_; 3] = std::array::from_fn(|index| {
            let name = fixture.sender(index);
            scope.spawn(move || sample(name, adapter, budget))
        });
        workers.map(|worker| worker.join().unwrap())
    })
}

pub(super) fn check(adapter: AdapterId) {
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let sources = [[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0], [0.0, 0.0, 1.0, 1.0]].map(|c| color(&mut engine, c));
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
    matrix(
        &mut engine,
        root.media,
        MatrixEdit::Configure {
            settings: MatrixSettings::default(),
        },
    );
    let MediaEditResult::MatrixCreated { items, .. } = matrix(
        &mut engine,
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
    let MediaEditResult::MatrixCreated { items, .. } = matrix(
        &mut engine,
        root.media,
        MatrixEdit::AddLayers {
            parent: Some(id),
            count: 1,
        },
    ) else {
        panic!()
    };
    let item = items[0];
    let MatrixItemId::Layer { id: layer } = item else {
        panic!()
    };
    matrix(&mut engine, root.media, MatrixEdit::AddColumns { count: 3 });
    let snapshot = inspect(&mut engine, root.media);
    matrix(
        &mut engine,
        root.media,
        MatrixEdit::ConfigureLayer {
            layer,
            settings: LayerSettings {
                duration_ms: 6000,
                ..snapshot
                    .items
                    .iter()
                    .find(|item| item.id == MatrixItemId::Layer { id: layer })
                    .unwrap()
                    .layer
                    .unwrap()
            },
        },
    );
    let columns = snapshot.columns.iter().map(|c| c.id).collect::<Vec<_>>();
    matrix(
        &mut engine,
        root.media,
        MatrixEdit::SetCells {
            assignments: columns
                .iter()
                .zip(sources)
                .map(|(column, source)| CellAssignment {
                    layer,
                    column: *column,
                    source: Some(source),
                })
                .collect(),
        },
    );
    let routes = [
        snapshot.items.iter().find(|i| i.id == item).unwrap().channel,
        snapshot.items.iter().find(|i| i.id == group).unwrap().channel,
        root,
    ];
    let mut fixture = Fixture::with_outputs(adapter, 3);
    launch(&mut engine, root, columns[0], LaunchMode::Cut);
    fixture.publish_channels(&engine, &routes, root, Some(4));
    let red = image([1.0, 0.0, 0.0, 1.0]);
    fixture.pixels_channels(adapter, &[&red, &red, &red], &red, "transition-initial-cut");
    // Three independent consumers each reserve receiving, staging and CPU storage. This is
    // qualification readback storage, separate from the unchanged renderer admission limits.
    let budget = PayloadPool::new("matrix-transition-readback", 9, 1024 * 1024).unwrap();
    for index in 0..3 {
        assert_eq!(sample(fixture.sender(index), adapter, &budget), [1.0, 0.0, 0.0]);
    }
    launch(&mut engine, root, columns[1], LaunchMode::Configured);
    fixture.publish_channels(&engine, &routes, root, None);
    let deadline = Instant::now() + Duration::from_secs(6);
    let before = loop {
        let p = sample(fixture.sender(0), adapter, &budget);
        assert!(p[2] <= 1.0 / 255.0, "blue entered the first two-color fade");
        if p[0] > 0.15 && p[1] > 0.15 {
            break p;
        }
        assert!(
            Instant::now() < deadline && p[0] > 0.05,
            "native intermediate fade was not observed"
        );
        std::thread::sleep(Duration::from_millis(30));
    };
    launch(&mut engine, root, columns[2], LaunchMode::Configured);
    fixture.publish_channels(&engine, &routes, root, None);
    let mut previous: Option<[[f64; 3]; 3]> = None;
    let mut count = 0;
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut ratio: Option<f64> = None;
    while count < 3 {
        let weights = sample_outputs(&fixture, adapter, &budget);
        if weights.iter().all(|w| w.iter().all(|v| *v > 0.02) && w[2] < 0.85) {
            for w in &weights {
                let current = w[0] / (w[0] + w[1]);
                if let Some(r) = ratio {
                    assert!(
                        (current - r).abs() < 0.025,
                        "outgoing red/green mixture changed after interruption: {weights:?}"
                    );
                } else {
                    ratio = Some(current);
                }
            }
            if let Some(old) = &previous {
                for (now, then) in weights.iter().zip(old) {
                    assert!(
                        now[2] >= then[2] - 2.0 / 255.0
                            && now[0] <= then[0] + 2.0 / 255.0
                            && now[1] <= then[1] + 2.0 / 255.0
                    );
                }
            }
            eprintln!("Matrix interrupted native weights sample={count} layer/group/final={weights:?}");
            previous = Some(weights);
            count += 1;
        }
        // A finite readback may finish after its sampled frame. Expire only pending observations.
        assert!(
            count == 3 || Instant::now() < deadline,
            "three-color interruption mixture was not observed: samples={count} weights={weights:?}"
        );
        std::thread::sleep(Duration::from_millis(80));
    }
    assert!(budget.wait_for_idle(Duration::from_secs(1)));
    // Observe the configured fade's real endpoint; do not forge a frame time for pixel capture.
    let deadline = Instant::now() + Duration::from_secs(12);
    loop {
        fixture.publish_channels(&engine, &routes, root, None);
        if fixture.renderer().channels().len() == 4 {
            break;
        }
        assert!(Instant::now() < deadline, "outgoing channel demand did not retire");
        std::thread::sleep(Duration::from_millis(50));
    }
    let blue = image([0.0, 0.0, 1.0, 1.0]);
    fixture.pixels_channels(adapter, &[&blue, &blue, &blue], &blue, "transition-interrupt-final");
    bounded(&mut fixture);
    eprintln!(
        "Matrix interruption original_mix={before:?} outgoing_ratio={ratio:?} samples={count} retired_channels=4"
    );
}
