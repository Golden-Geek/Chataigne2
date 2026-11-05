use super::matrix::{matrix_edit, prepare};
use super::*;
use crate::matrix::protocol::*;
use golden_video::plan::{Point, RenderOperation};

#[test]
fn cell_placement_maps_fit_fill_stretch_and_natural_source_resize_without_extra_passes() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { items, .. } = matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddLayers { parent: None, count: 1 },
    ) else {
        panic!()
    };
    let MatrixItemId::Layer { id: layer } = items[0] else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { columns, .. } = matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddColumns { parent: None, count: 1 },
    ) else {
        panic!()
    };
    for (media, width, height) in [(source.media, 8, 4), (matrix.media, 32, 24)] {
        edit(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(width, height).unwrap(),
                },
            },
        );
    }
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureLayer {
            layer,
            settings: LayerSettings {
                size: Extent2D::new(32, 24).unwrap(),
                ..Default::default()
            },
        },
    );
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureItem {
            item: items[0],
            settings: CompositeSettings {
                expose_channel: true,
                ..Default::default()
            },
        },
    );
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::DropSource {
            layer,
            column: columns[0],
            source: source.media.0,
            copy: false,
        },
    );
    matrix_edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    let snapshot = crate::matrix::inspect(&engine, matrix.media).unwrap();
    let output = snapshot.items[0].channel;
    let mut pass_count = None;
    for (mode, expected, outside) in [
        (MediaFitMode::Fit, [0.0625, 0.125], [4.5, 4.5]),
        (MediaFitMode::Fill, [0.28125, 0.3125], [3.5, 6.5]),
        (MediaFitMode::Stretch, [0.0625, 0.3125], [3.5, 6.5]),
    ] {
        matrix_edit(
            &mut engine,
            matrix.media,
            MatrixEdit::ConfigureCell {
                layer,
                column: columns[0],
                settings: CellSettings {
                    position: [4.0, 4.0],
                    size: Some(Extent2D::new(8, 8).unwrap()),
                    fit_mode: mode,
                    ..Default::default()
                },
            },
        );
        let frame = prepare(&engine, &[output]);
        let count = frame.submission.plan.passes.len();
        assert_eq!(
            *pass_count.get_or_insert(count),
            count,
            "fit policy does not add an intermediate pass"
        );
        assert_eq!(
            frame
                .channels
                .iter()
                .find(|channel| channel.reference == source)
                .unwrap()
                .resolution
                .extent,
            Some(Extent2D::new(8, 4).unwrap())
        );
        let pass = frame
            .submission
            .plan
            .passes
            .iter()
            .find(|pass| pass.output == frame.entries[&output].target)
            .unwrap();
        let RenderOperation::Composite { layers } = &pass.operation else {
            panic!()
        };
        let mapping = layers[0].mapping.as_ref().unwrap();
        let mapped = mapping
            .source_coordinate(Point {
                x: 4.5 / 32.0,
                y: 6.5 / 24.0,
            })
            .unwrap();
        for (actual, expected) in mapped.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 0.00001, "{mode:?}: {actual} != {expected}");
        }
        assert_eq!(
            mapping.source_coordinate(Point {
                x: outside[0] / 32.0,
                y: outside[1] / 24.0
            }),
            None
        );
    }
    // Source dimensions can change independently of a fixed placement, or drive natural sizing.
    edit(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: source.media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(16, 4).unwrap(),
            },
        },
    );
    for size in [Some(Extent2D::new(8, 8).unwrap()), None] {
        matrix_edit(
            &mut engine,
            matrix.media,
            MatrixEdit::ConfigureCell {
                layer,
                column: columns[0],
                settings: CellSettings {
                    position: [4.0, 4.0],
                    size,
                    ..Default::default()
                },
            },
        );
        let frame = prepare(&engine, &[output]);
        let pass = frame
            .submission
            .plan
            .passes
            .iter()
            .find(|pass| pass.output == frame.entries[&output].target)
            .unwrap();
        let RenderOperation::Composite { layers } = &pass.operation else {
            panic!()
        };
        let mapping = layers[0].mapping.as_ref().unwrap();
        let point = if size.is_some() { [8.0, 8.0] } else { [12.0, 6.0] };
        let mapped = mapping
            .source_coordinate(Point {
                x: point[0] / 32.0,
                y: point[1] / 24.0,
            })
            .unwrap();
        assert!(mapped.into_iter().all(|value| (value - 0.5).abs() < 0.00001));
    }
}
