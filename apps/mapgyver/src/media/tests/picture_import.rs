use super::*;
use crate::media::{channel, validate_project};
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};
use golden_video::Extent2D;

fn relative(path: &str) -> AssetLocator {
    AssetLocator::ProjectRelative {
        path: path.to_owned().try_into().unwrap(),
    }
}
fn inspect(engine: &mut AppEngine) -> MediaManagerSnapshot {
    let MediaEditResult::Manager { snapshot } = apply(engine, MediaEdit::InspectManager) else {
        panic!()
    };
    snapshot
}
#[test]
fn import_creates_only_one_picture_tree_with_owned_file_and_atomic_history_and_reload() {
    let mut engine = fixture();
    let history = engine.undo_len();
    let MediaEditResult::PictureImported { reference } = apply(
        &mut engine,
        MediaEdit::ImportPicture {
            locator: relative("same.png"),
            target: None,
            label: Some("same.png".into()),
        },
    ) else {
        panic!()
    };
    assert_eq!(engine.undo_len(), history + 1);
    let snapshot = inspect(&mut engine);
    assert_eq!(snapshot.media.len(), 1);
    assert!(snapshot.assets.is_empty());
    assert_eq!(snapshot.media[0].label, "same.png");
    assert_eq!(
        channel(&engine, reference).unwrap().definition.source,
        ChannelSource::Picture {
            file: Some(relative("same.png"))
        }
    );
    assert!(engine.undo().unwrap());
    assert!(channel(&engine, reference).is_err());
    assert!(inspect(&mut engine).media.is_empty());
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine), snapshot);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    validate_project(&loaded).unwrap();
    assert_eq!(inspect(&mut loaded), snapshot);
}
#[test]
fn targeted_picture_import_preserves_topology_sizing_fit_and_independent_peer_file() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::Picture);
    let second = create(&mut engine, MediaKind::Picture);
    apply(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference: first,
            file: "original.png".into(),
        },
    );
    apply(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: second.media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(320, 240).unwrap(),
            },
        },
    );
    apply(
        &mut engine,
        MediaEdit::ConfigureMediaFitMode {
            media: second.media,
            mode: MediaFitMode::Fill,
        },
    );
    let before = channel(&engine, second).unwrap().definition;
    let count = engine.nodes.len();
    let history = engine.undo_len();
    let MediaEditResult::PictureImported { reference } = apply(
        &mut engine,
        MediaEdit::ImportPicture {
            locator: relative("replacement.png"),
            target: Some(second),
            label: None,
        },
    ) else {
        panic!()
    };
    assert_eq!(reference, second);
    assert_eq!(engine.nodes.len(), count);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(inspect(&mut engine).assets.is_empty());
    let after = channel(&engine, second).unwrap().definition;
    assert_eq!(after.size, before.size);
    assert_eq!(after.fit_mode, before.fit_mode);
    assert_eq!(
        after.source,
        ChannelSource::Picture {
            file: Some(relative("replacement.png"))
        }
    );
    assert_eq!(
        channel(&engine, first).unwrap().definition.source,
        ChannelSource::Picture {
            file: Some(relative("original.png"))
        }
    );
    assert!(engine.undo().unwrap());
    assert_eq!(channel(&engine, second).unwrap().definition, before);
    assert!(engine.redo().unwrap());
    assert_eq!(channel(&engine, second).unwrap().definition, after);
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
fn quickjs_picture_import_returns_the_same_committed_picture_and_single_history_step() {
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
    let MediaEditResult::PictureImported { reference } = serde_json::from_value(reply["value"].clone()).unwrap() else {
        panic!()
    };
    assert_eq!(
        channel(&engine, reference).unwrap().definition.source,
        ChannelSource::Picture {
            file: Some(relative("script.png"))
        }
    );
    assert!(inspect(&mut engine).assets.is_empty());
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert!(channel(&engine, reference).is_err());
}
