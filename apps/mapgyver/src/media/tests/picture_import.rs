use super::*;
use crate::media::{asset, channel, validate_project};
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};
use golden_video::{Extent2D, plan::SampleFilter};

fn relative(path: &str) -> AssetLocator {
    AssetLocator::ProjectRelative {
        path: path.to_owned().try_into().unwrap(),
    }
}

fn import(engine: &mut AppEngine, target: Option<ChannelReference>, path: &str) -> (ChannelReference, AssetId) {
    let MediaEditResult::PictureImported { reference, asset } = apply(
        engine,
        MediaEdit::ImportPicture {
            locator: relative(path),
            target,
            label: None,
        },
    ) else {
        panic!("Picture import result")
    };
    (reference, asset)
}

fn inspect(engine: &mut AppEngine) -> MediaManagerSnapshot {
    let MediaEditResult::Manager { snapshot } = apply(engine, MediaEdit::InspectManager) else {
        panic!()
    };
    snapshot
}

#[test]
fn import_creates_asset_media_and_channel_in_one_history_step_with_unique_labels_and_stable_ids() {
    let mut engine = fixture();
    let before = engine.undo_len();
    let MediaEditResult::PictureImported { reference, asset: id } = apply(
        &mut engine,
        MediaEdit::ImportPicture {
            locator: relative("same.png"),
            target: None,
            label: Some("same.png".into()),
        },
    ) else {
        panic!()
    };
    assert_eq!(engine.undo_len(), before + 1);
    let snapshot = inspect(&mut engine);
    assert_eq!(snapshot.media.len(), 1);
    assert_eq!(snapshot.assets.len(), 1);
    assert_ne!(snapshot.media[0].label, snapshot.assets[0].label);
    let mut expected = ChannelDefinition::for_media(MediaKind::Picture);
    expected.source = ChannelSource::Picture { asset: Some(id) };
    assert_eq!(channel(&engine, reference).unwrap().definition, expected);
    assert!(engine.undo().unwrap());
    assert!(channel(&engine, reference).is_err());
    assert!(asset(&engine, id).is_err());
    assert!(inspect(&mut engine).media.is_empty());
    assert!(inspect(&mut engine).assets.is_empty());
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine), snapshot);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    validate_project(&loaded).unwrap();
    assert_eq!(inspect(&mut loaded), snapshot);
}

#[test]
fn import_into_one_channel_preserves_output_settings_and_relink_keeps_shared_asset_identity() {
    let mut engine = fixture();
    let (first, shared) = import(&mut engine, None, "original.png");
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let first_definition = channel(&engine, first).unwrap().definition;
    let mut definition = first_definition.clone();
    definition.size = SizePolicy::Fixed {
        extent: Extent2D::new(320, 240).unwrap(),
    };
    definition.filter = SampleFilter::Nearest;
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: second,
            definition: definition.clone(),
        },
    );
    let before = engine.undo_len();
    let (assigned, replacement) = import(&mut engine, Some(second), "replacement.png");
    assert_eq!(assigned, second);
    assert_ne!(replacement, shared);
    assert_eq!(engine.undo_len(), before + 1);
    let actual = channel(&engine, second).unwrap().definition;
    assert_eq!(
        actual.source,
        ChannelSource::Picture {
            asset: Some(replacement)
        }
    );
    assert_eq!(actual.size, definition.size);
    assert_eq!(actual.filter, definition.filter);
    assert_eq!(channel(&engine, first).unwrap().definition, first_definition);
    assert!(engine.undo().unwrap());
    assert!(asset(&engine, replacement).is_err());
    assert_eq!(channel(&engine, second).unwrap().definition, definition);
    apply(
        &mut engine,
        MediaEdit::RelinkAsset {
            asset: shared,
            locator: relative("moved.png"),
        },
    );
    assert_eq!(asset(&engine, shared).unwrap().locator, relative("moved.png"));
    for reference in [first, second] {
        assert_eq!(
            channel(&engine, reference).unwrap().definition.source,
            ChannelSource::Picture { asset: Some(shared) }
        );
    }
    assert!(engine.undo().unwrap());
    assert_eq!(asset(&engine, shared).unwrap().locator, relative("original.png"));
}

#[test]
fn invalid_picture_import_does_not_leave_orphan_assets_or_partial_media() {
    let mut engine = fixture();
    let solid = create(&mut engine, MediaKind::SolidColor);
    let picture = create(&mut engine, MediaKind::Picture);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (target, label) in [
        (Some(solid), None),
        (
            Some(ChannelReference {
                media: picture.media,
                channel: solid.channel,
            }),
            None,
        ),
        (Some(picture), Some("unexpected rename".into())),
        (None, Some("  ".into())),
    ] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::ImportPicture {
                    locator: relative("new.png"),
                    target,
                    label
                }
            )
            .success
        );
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(inspect(&mut engine).assets.is_empty());
    }
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::json!({"kind":"ImportPicture","locator":{"kind":"project_relative","path":"../bad.png"},"target":null,"label":null}),
        },
    );
    assert!(!ack.success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn quickjs_picture_import_returns_the_same_committed_channel_asset_and_single_history_step() {
    use golden_core::application::service_script_app_operations;
    use golden_core::events::EventKind;
    use golden_core::script::{ScriptNode, ScriptNodeConfig, ScriptSource};
    let mut engine = fixture();
    let request = serde_json::to_string(&MediaEdit::ImportPicture {
        locator: relative("script.png"),
        target: None,
        label: None,
    })
    .unwrap();
    let text = format!(
        "function init() {{ gc.invokeAppOperation('picture', {MEDIA_NAMESPACE:?}, {MEDIA_OPERATION:?}, {request}); }}\nfunction onAppOperationResult(reply) {{ gc.emit('picture-reply', reply); }}"
    );
    let mut script = ScriptNode::new(
        "Picture Caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline { text },
        },
    );
    script.budgets.max_wall_time_us_per_load = 1_000_000;
    script.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(script.into(), None);
    engine.apply_edits().unwrap();
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let reply = engine
        .ui_event_log()
        .iter()
        .find_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "picture-reply" => Some((*event.payload).clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(reply["status"], "applied");
    let MediaEditResult::PictureImported { reference, asset: id } =
        serde_json::from_value(reply["value"].clone()).unwrap()
    else {
        panic!()
    };
    assert_eq!(
        channel(&engine, reference).unwrap().definition.source,
        ChannelSource::Picture { asset: Some(id) }
    );
    assert_eq!(asset(&engine, id).unwrap().locator, relative("script.png"));
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert!(channel(&engine, reference).is_err());
    assert!(asset(&engine, id).is_err());
}
