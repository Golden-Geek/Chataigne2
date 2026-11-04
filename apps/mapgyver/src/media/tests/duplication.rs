use super::*;
use crate::media::{catalog, channel, validate_project};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::Node,
};
use golden_video::Extent2D;

#[test]
fn source_media_and_variant_copies_preserve_shared_settings_assets_and_independent_history() {
    for kind in [MediaKind::SolidColor, MediaKind::Picture, MediaKind::SharedTexture] {
        for copy_media in [false, true] {
            let mut engine = fixture();
            let MediaEditResult::AssetCreated { asset } = apply(
                &mut engine,
                MediaEdit::ImportAsset {
                    locator: AssetLocator::ProjectRelative {
                        path: "shared-copy.png".to_owned().try_into().unwrap(),
                    },
                    label: None,
                },
            ) else {
                panic!()
            };
            let first = create(&mut engine, kind);
            let MediaEditResult::Created { reference: second } = apply(
                &mut engine,
                MediaEdit::AddChannel {
                    media: first.media,
                    label: Some("Independent peer".into()),
                },
            ) else {
                panic!()
            };
            for (index, reference) in [first, second].into_iter().enumerate() {
                let source = match kind {
                    MediaKind::SolidColor => ChannelSource::SolidColor {
                        rgba: [index as f32, 0.25, 0.75, 0.5],
                    },
                    MediaKind::Picture => ChannelSource::Picture { asset: Some(asset) },
                    MediaKind::SharedTexture => ChannelSource::Spout {
                        sender: Some(format!("Exact copy source {index}")),
                    },
                    _ => unreachable!(),
                };
                apply(&mut engine, MediaEdit::ConfigureChannel { reference, source });
            }
            apply(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: first.media,
                    size: SizePolicy::Fixed {
                        extent: Extent2D::new(7, 3).unwrap(),
                    },
                },
            );
            if kind != MediaKind::SolidColor {
                apply(
                    &mut engine,
                    MediaEdit::ConfigureMediaFitMode {
                        media: first.media,
                        mode: MediaFitMode::Fill,
                    },
                );
            }
            let before = catalog::inspect(&engine).unwrap();
            let owner = engine.node_id_by_uuid(first.media.0).unwrap();
            let source = engine
                .node_id_by_uuid(if copy_media { first.media.0 } else { first.channel.0 })
                .unwrap();
            let parent = if copy_media {
                engine.nodes.get(owner).unwrap().node_data().parent.unwrap()
            } else {
                owner
            };
            let history = engine.undo_len();
            let ack = apply_project_intent(
                &mut engine,
                UiEditIntent::DuplicateNode {
                    source,
                    new_parent: parent,
                    new_prev_sibling: Some(source),
                    initial_params: vec![],
                },
            );
            assert!(ack.success, "{kind:?}, media={copy_media}: {ack:?}");
            assert_eq!(engine.undo_len(), history + 1);
            let copied = catalog::inspect(&engine).unwrap();
            assert_eq!(copied.assets, before.assets, "source copies share the authored asset");
            let copy = if copy_media {
                let copy = copied.media.iter().find(|media| media.id != first.media).unwrap();
                assert_eq!(copy.channels.len(), 2);
                assert_eq!(copy.media_kind, kind);
                for (original, duplicate) in before.media[0].channels.iter().zip(&copy.channels) {
                    assert_ne!(original.reference.media, duplicate.reference.media);
                    assert_ne!(original.reference.channel, duplicate.reference.channel);
                    assert_eq!(original.label, duplicate.label);
                    assert_eq!(original.definition, duplicate.definition);
                }
                assert_eq!(copied.media[0], before.media[0]);
                copy.channels[0].reference
            } else {
                assert_eq!(copied.media.len(), 1);
                let channels = &copied.media[0].channels;
                assert_eq!(channels.len(), 3);
                assert_eq!(channels[0], before.media[0].channels[0]);
                assert_eq!(channels[2], before.media[0].channels[1]);
                assert_eq!(channels[1].reference.media, first.media);
                assert_ne!(channels[1].reference.channel, first.channel);
                assert_eq!(channels[1].definition, before.media[0].channels[0].definition);
                channels[1].reference
            };
            let replacement = match kind {
                MediaKind::SolidColor => ChannelSource::SolidColor { rgba: [0.5; 4] },
                MediaKind::Picture => ChannelSource::Picture { asset: None },
                MediaKind::SharedTexture => ChannelSource::Spout { sender: None },
                _ => unreachable!(),
            };
            apply(
                &mut engine,
                MediaEdit::ConfigureChannel {
                    reference: copy,
                    source: replacement,
                },
            );
            for original in &before.media[0].channels {
                assert_eq!(channel(&engine, original.reference).unwrap(), *original);
            }
            if copy_media {
                let edited_source = catalog::inspect(&engine).unwrap();
                apply(
                    &mut engine,
                    MediaEdit::ConfigureMediaResolution {
                        media: copy.media,
                        size: SizePolicy::Automatic,
                    },
                );
                let resized = catalog::inspect(&engine).unwrap();
                assert_eq!(resized.media[0], before.media[0]);
                assert!(
                    resized.media[1]
                        .channels
                        .iter()
                        .all(|channel| { channel.definition.size == SizePolicy::Automatic })
                );
                assert!(engine.undo().unwrap());
                assert_eq!(catalog::inspect(&engine).unwrap(), edited_source);
            }
            assert!(engine.undo().unwrap());
            assert_eq!(catalog::inspect(&engine).unwrap(), copied);
            assert!(engine.undo().unwrap());
            assert_eq!(catalog::inspect(&engine).unwrap(), before);
            assert!(engine.redo().unwrap());
            assert_eq!(catalog::inspect(&engine).unwrap(), copied);
            let json = to_sparse_project_json_pretty(&engine).unwrap();
            let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
            loaded.apply_edits().unwrap();
            validate_project(&loaded).unwrap();
            assert_eq!(catalog::inspect(&loaded).unwrap(), copied);
        }
    }
}
