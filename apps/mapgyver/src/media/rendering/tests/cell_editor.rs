use super::matrix::matrix_edit;
use super::*;
use crate::{matrix::protocol::*, preview::protocol::PreviewSource};
use golden_video::plan::{Point, RenderOperation};

#[test]
fn inactive_cell_editor_preview_is_transient_placed_bounded_and_does_not_launch_or_capture_app_demand() {
    let mut engine = engine();
    let root = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::MatrixCreated { items, .. } = matrix_edit(
        &mut engine,
        root.media,
        MatrixEdit::AddLayers { parent: None, count: 1 },
    ) else {
        panic!()
    };
    let MatrixItemId::Layer { id: layer } = items[0] else {
        panic!()
    };
    let MediaEditResult::MatrixCreated { columns, .. } = matrix_edit(
        &mut engine,
        root.media,
        MatrixEdit::AddColumns { parent: None, count: 1 },
    ) else {
        panic!()
    };
    matrix_edit(
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
    matrix_edit(
        &mut engine,
        root.media,
        MatrixEdit::ConfigureItem {
            item: items[0],
            settings: CompositeSettings {
                opacity: 0.2,
                enabled: false,
                ..Default::default()
            },
        },
    );
    let MediaEditResult::MatrixMediaCreated { cell, media } = matrix_edit(
        &mut engine,
        root.media,
        MatrixEdit::CreateCellMedia {
            layer,
            column: columns[0],
            media_kind: MediaKind::SolidColor,
        },
    ) else {
        panic!()
    };
    edit(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(8, 4).unwrap(),
            },
        },
    );
    matrix_edit(
        &mut engine,
        root.media,
        MatrixEdit::ConfigureCell {
            layer,
            column: columns[0],
            settings: CellSettings {
                position: [4.0, 4.0],
                size: Some(Extent2D::new(8, 8).unwrap()),
                fit_mode: MediaFitMode::Fill,
                opacity: 0.5,
                enabled: false,
            },
        },
    );
    let source = PreviewSource::MatrixCell {
        media: root.media,
        cell,
    };
    let reference = source.reference().unwrap();
    let before = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let capture = crate::media::capture_editor_channels(&engine, &[], &[source, source], 16, None).unwrap();
    assert_eq!(capture.channels.len(), 2);
    assert!(capture.application_demand().is_empty());
    assert_eq!(
        golden_core::app::to_sparse_project_json_pretty(&engine).unwrap(),
        before
    );
    assert_eq!(engine.undo_len(), history);
    let owner = renderer();
    let routes = [ChannelRoute {
        output: OutputId::new(1).unwrap(),
        channel: reference,
    }];
    let frame = super::super::plan::build(
        &capture,
        super::super::plan::Targets {
            routes: &routes,
            preview: None,
            screens: &[],
        },
        super::super::plan::Previous {
            channels: &Default::default(),
            screens: &Default::default(),
        },
        &mut 3,
        config().plan_limits,
        super::super::plan::Sources {
            observations: &owner.source_observations(),
            pictures: None,
            #[cfg(feature = "video-files")]
            videos: None,
            calibration: &mut Default::default(),
        },
    )
    .unwrap();
    let rendered = frame
        .channels
        .iter()
        .find(|channel| channel.reference == reference)
        .unwrap();
    assert_eq!(rendered.resolution.extent, Some(Extent2D::new(32, 24).unwrap()));
    assert_eq!(
        rendered.resolution.availability,
        crate::media::resolution::ChannelAvailability::Ready
    );
    let pass = frame
        .submission
        .plan
        .passes
        .iter()
        .find(|pass| pass.output == rendered.target)
        .unwrap();
    let RenderOperation::Composite { layers } = &pass.operation else {
        panic!()
    };
    assert_eq!(
        layers[0].opacity.get(),
        0.5,
        "content editing preserves Cell opacity without applying whole-Layer compositing"
    );
    let mapped = layers[0]
        .mapping
        .as_ref()
        .unwrap()
        .source_coordinate(Point {
            x: 4.5 / 32.0,
            y: 6.5 / 24.0,
        })
        .unwrap();
    assert!((mapped[0] - 0.28125).abs() < 0.00001);
    assert!((mapped[1] - 0.3125).abs() < 0.00001);
    close(owner);
    assert!(crate::media::capture_editor_channels(&engine, &[], &[source], 1, None).is_err());
    matrix_edit(
        &mut engine,
        root.media,
        MatrixEdit::ConfigureCell {
            layer,
            column: columns[0],
            settings: CellSettings::default(),
        },
    );
    assert!(!capture.is_current());
    let updated = crate::media::capture_editor_channels(&engine, &[], &[source], 16, None).unwrap();
    assert_eq!(
        updated
            .channels
            .iter()
            .find(|channel| channel.reference == reference)
            .unwrap()
            .matrix
            .as_ref()
            .unwrap()
            .inputs[0]
            .placement
            .unwrap()
            .size,
        None
    );
    matrix_edit(&mut engine, root.media, MatrixEdit::RemoveColumn { column: columns[0] });
    assert!(!updated.is_current());
    let missing = crate::media::capture_editor_channels(&engine, &[], &[source], 16, None).unwrap();
    assert!(
        missing
            .channels
            .iter()
            .find(|channel| channel.reference == reference)
            .unwrap()
            .definition
            .is_none()
    );
    assert!(engine.undo().unwrap());
    assert!(
        crate::media::capture_editor_channels(&engine, &[], &[source], 16, None)
            .unwrap()
            .channels
            .iter()
            .find(|channel| channel.reference == reference)
            .unwrap()
            .definition
            .is_some()
    );
    let MediaEditResult::MatrixPlayback { snapshot } =
        matrix_edit(&mut engine, root.media, MatrixEdit::InspectPlayback)
    else {
        panic!()
    };
    assert_eq!(snapshot.desired_column, None);
}
