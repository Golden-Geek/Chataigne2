use super::*;
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};
use golden_video::Extent2D;

fn inspect(engine: &mut AppEngine) -> MediaManagerSnapshot {
    let MediaEditResult::Manager { snapshot } = apply(engine, MediaEdit::InspectManager) else {
        panic!("manager result")
    };
    snapshot
}

#[test]
fn catalog_inspection_is_read_only_and_retains_channel_order_across_history_and_reload() {
    let mut engine = fixture();
    assert!(inspect(&mut engine).media.is_empty());
    let first = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!("created result")
    };
    let mut definition = crate::media::channel(&engine, second).unwrap().definition;
    definition.size = SizePolicy::Fixed {
        extent: Extent2D::new(640, 360).unwrap(),
    };
    definition.source = ChannelSource::SolidColor {
        rgba: [0.2, 0.4, 0.6, 0.5],
    };
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: second,
            definition: definition.clone(),
        },
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let snapshot = inspect(&mut engine);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(snapshot.media.len(), 1);
    assert_eq!(snapshot.media[0].channels[0].reference, first);
    assert_eq!(snapshot.media[0].channels[1].reference, second);
    assert_eq!(snapshot.media[0].channels[1].definition, definition);
    apply(
        &mut engine,
        MediaEdit::ReorderChannel {
            reference: second,
            after: None,
        },
    );
    assert_eq!(inspect(&mut engine).media[0].channels[0].reference, second);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine), snapshot);
    assert!(engine.redo().unwrap());
    let reordered = inspect(&mut engine);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(inspect(&mut loaded), reordered);
}

#[test]
fn catalog_lists_media_kinds_without_exposing_assets_as_media() {
    let mut engine = fixture();
    for kind in [MediaKind::SolidColor, MediaKind::Picture, MediaKind::SharedTexture] {
        create(&mut engine, kind);
    }
    apply(
        &mut engine,
        MediaEdit::ImportAsset {
            locator: AssetLocator::ProjectRelative {
                path: "image.png".to_owned().try_into().unwrap(),
            },
            label: None,
        },
    );
    let snapshot = inspect(&mut engine);
    assert_eq!(snapshot.media.len(), 3);
    assert_eq!(snapshot.media[2].media_kind, MediaKind::SharedTexture);
    assert!(snapshot.media.iter().all(|media| media.channels.len() == 1));
    assert_eq!(snapshot.assets.len(), 1);
    assert_eq!(snapshot.assets[0].label, "image.png");
    assert!(snapshot.assets[0].locator.is_some());
}
