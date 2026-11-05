use super::*;
use golden_core::app::create_new_project_engine;
use mapgyver::preview::protocol::PreviewSource;

#[test]
#[ignore = "explicit inactive Cell editor preview pixels on each hardware adapter"]
fn native_cell_editor_previews_inactive_placement_without_playback_on_each_adapter() {
    let adapters = golden_video::native::adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!adapters.is_empty());
    for adapter in adapters {
        let mut engine = create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let root = create(&mut engine, MediaKind::Matrix);
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
        matrix(
            &mut engine,
            root.media,
            MatrixEdit::ConfigureItem {
                item: items[0],
                settings: CompositeSettings {
                    enabled: false,
                    opacity: 0.2,
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
        let MediaEditResult::MatrixMediaCreated { cell, media } = matrix(
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
        let source = PreviewSource::MatrixCell {
            media: root.media,
            cell,
        };
        let mut fixture = Fixture::new(adapter.id);
        for (mode, size, position, scale, label) in [
            (
                MediaFitMode::Fit,
                Some(Extent2D::new(8, 8).unwrap()),
                [3.0, 4.0],
                [1.0, 1.0],
                "inactive-editor-fit",
            ),
            (
                MediaFitMode::Fill,
                Some(Extent2D::new(8, 8).unwrap()),
                [-1.0, 2.0],
                [2.0, 2.0],
                "inactive-editor-fill",
            ),
            (
                MediaFitMode::Stretch,
                Some(Extent2D::new(8, 8).unwrap()),
                [3.0, 2.0],
                [1.0, 2.0],
                "inactive-editor-stretch",
            ),
            (
                MediaFitMode::Fit,
                None,
                [3.0, 2.0],
                [1.0, 1.0],
                "inactive-editor-natural",
            ),
        ] {
            matrix(
                &mut engine,
                root.media,
                MatrixEdit::ConfigureCell {
                    layer,
                    column: columns[0],
                    settings: CellSettings {
                        position: [3.0, 2.0],
                        size,
                        fit_mode: mode,
                        opacity: 0.5,
                        enabled: false,
                    },
                },
            );
            let mut expected = image([0.0; 4]);
            expected.draw_clipped(
                &Image::solid(8, 4, [1.0; 4]),
                &InstanceDefinition {
                    transform: Transform2D {
                        position,
                        scale,
                        pivot: [0.0; 2],
                        ..Default::default()
                    },
                    opacity: 0.5,
                    ..Default::default()
                },
                [3.0, 2.0, 11.0, if size.is_some() { 10.0 } else { 6.0 }],
            );
            fixture.publish_editor(&engine, source);
            fixture.pixels(adapter.id, &expected, label);
        }
        let MediaEditResult::MatrixPlayback { snapshot } = matrix(&mut engine, root.media, MatrixEdit::InspectPlayback)
        else {
            panic!()
        };
        assert_eq!(snapshot.desired_column, None);
        assert!(snapshot.layers.is_empty());
        drop(fixture);
        println!(
            "Inactive Cell editor preview pixels passed on {} without playback",
            adapter.description
        );
    }
}
