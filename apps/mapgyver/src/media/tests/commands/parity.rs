//! Source authoring must preserve the same committed contract for every public caller.
use super::*;
use crate::media::channel;
use golden_core::app::ProjectNode;
use golden_video::{AlphaMode, ColorEncoding, Extent2D, PixelFormat, TextureDescriptor};

#[derive(Clone, Copy, Debug)]
enum Caller {
    Ui,
    QuickJs,
    HostedQuickJs,
    Command,
}
const CALLERS: [Caller; 4] = [Caller::Ui, Caller::QuickJs, Caller::HostedQuickJs, Caller::Command];

fn relative(path: &str) -> AssetLocator {
    AssetLocator::ProjectRelative {
        path: path.to_owned().try_into().unwrap(),
    }
}
fn inspect(engine: &AppEngine) -> MediaManagerSnapshot {
    crate::media::catalog::inspect(engine).unwrap()
}

/// The observer is installed before measuring history. Only the operation itself earns a step.
fn run(engine: &mut AppEngine, caller: Caller, operation: MediaEdit) -> Option<serde_json::Value> {
    let request = serde_json::to_string(&operation).unwrap();
    let mut execute = None;
    match caller {
        Caller::Ui => {}
        Caller::QuickJs | Caller::HostedQuickJs => {
            let invoke = if matches!(caller, Caller::QuickJs) {
                format!("gc.invokeAppOperation('parity', {MEDIA_NAMESPACE:?}, {MEDIA_OPERATION:?}, {request});")
            } else {
                format!("local.listen({{level:1}}); local.editMedia('parity', {request:?});")
            };
            script(
                engine,
                format!(
                    "function init(){{{invoke}}} function onAppOperationResult(reply){{gc.emit('media-method-reply',reply);}}"
                ),
            );
        }
        Caller::Command => {
            script(engine, "function init(){local.listen({level:1});} function onAppOperationResult(reply){gc.emit('media-method-reply',reply);}".into());
            let node = command(engine, "Media Operation");
            let parameter = engine
                .process_tree_snapshot()
                .find_child_by_decl_id(node, "request")
                .unwrap();
            let ack = apply_project_intent(
                engine,
                UiEditIntent::SetParam {
                    node: parameter,
                    value: ParamValue::Str(request),
                    behaviour: ParameterEventBehaviour::Append,
                },
            );
            assert!(ack.success, "{ack:?}");
            execute = Some(
                engine
                    .process_tree_snapshot()
                    .find_child_by_decl_id(node, "execute")
                    .unwrap(),
            );
        }
    }
    let before = inspect(engine);
    let history = engine.undo_len();
    let result = if matches!(caller, Caller::Ui) {
        let ack = invoke(engine, operation);
        ack.success.then(|| ack.result.unwrap().value)
    } else {
        if let Some(trigger) = execute {
            let ack = apply_project_intent(
                engine,
                UiEditIntent::SetParam {
                    node: trigger,
                    value: ParamValue::Trigger(),
                    behaviour: ParameterEventBehaviour::Append,
                },
            );
            assert!(ack.success, "{ack:?}");
        }
        service_script_app_operations(engine).unwrap();
        let responses = replies(engine);
        assert_eq!(responses.len(), 1, "{caller:?} must emit one committed callback");
        assert_eq!(
            responses[0]["request_id"],
            if matches!(caller, Caller::Command) {
                "media-command"
            } else {
                "parity"
            }
        );
        if responses[0]["status"] == "applied" {
            Some(responses[0]["value"].clone())
        } else {
            assert_eq!(responses[0]["status"], "rejected");
            assert!(responses[0].get("value").is_none());
            assert!(
                responses[0]["message"]
                    .as_str()
                    .is_some_and(|message| !message.is_empty())
            );
            None
        }
    };
    let after = inspect(engine);
    if after != before {
        assert!(result.is_some());
        assert_eq!(engine.undo_len(), history + 1, "{caller:?}");
        assert!(engine.undo().unwrap());
        assert_eq!(inspect(engine), before, "{caller:?} undo");
        assert!(engine.redo().unwrap());
        assert_eq!(inspect(engine), after, "{caller:?} redo");
    } else {
        assert_eq!(
            engine.undo_len(),
            history,
            "inspection/rejection must not author history"
        );
    }
    service_script_app_operations(engine).unwrap();
    assert_eq!(
        inspect(engine),
        after,
        "history replay must not requeue the command/script"
    );
    if !matches!(caller, Caller::Ui) {
        assert_eq!(replies(engine).len(), 1);
    }
    // JavaScript JSON numbers do not retain serde's integer/float representation. Decode the
    // actual public DTO before comparison, preserving typed values rather than lexical numbers.
    result.map(|value| serde_json::to_value(serde_json::from_value::<MediaEditResult>(value).unwrap()).unwrap())
}

struct Baseline {
    json: String,
    references: [[ChannelReference; 2]; 3],
    asset: AssetId,
}
fn baseline() -> Baseline {
    let mut engine = fixture();
    let MediaEditResult::AssetCreated { asset } = apply(
        &mut engine,
        MediaEdit::ImportAsset {
            locator: relative("first.png"),
            label: None,
        },
    ) else {
        panic!()
    };
    let references = [MediaKind::SolidColor, MediaKind::Picture, MediaKind::SharedTexture].map(|kind| {
        let first = create(&mut engine, kind);
        let MediaEditResult::Created { reference: second } = apply(
            &mut engine,
            MediaEdit::AddChannel {
                media: first.media,
                label: Some("Second".into()),
            },
        ) else {
            panic!()
        };
        for (index, reference) in [first, second].into_iter().enumerate() {
            let mut definition = channel(&engine, reference).unwrap().definition;
            match &mut definition.source {
                ChannelSource::Picture { asset: selected } => *selected = Some(asset),
                ChannelSource::Spout { sender, maximum } => {
                    *sender = Some(format!("Source {index}"));
                    *maximum = TextureDescriptor {
                        extent: Extent2D::new(4, 4).unwrap(),
                        format: PixelFormat::Bgra8Unorm,
                        color_encoding: ColorEncoding::Srgb,
                        alpha_mode: AlphaMode::Straight,
                    };
                }
                _ => {}
            }
            if index == 1 {
                definition.size = SizePolicy::Fixed {
                    extent: Extent2D::new(7, 3).unwrap(),
                };
            }
            apply(&mut engine, MediaEdit::ConfigureChannel { reference, definition });
        }
        [first, second]
    });
    Baseline {
        json: to_sparse_project_json_pretty(&engine).unwrap(),
        references,
        asset,
    }
}
fn loaded(json: &str) -> AppEngine {
    let mut engine = from_sparse_project_json::<AppNode>(json).unwrap();
    engine.apply_edits().unwrap();
    engine
}

#[test]
fn source_mutations_inspection_and_atomic_rejection_match_ui_quickjs_hosted_and_command_callers() {
    let baseline = baseline();
    let initial = loaded(&baseline.json);
    let mut cases = vec![
        MediaEdit::InspectManager,
        MediaEdit::InspectAsset { asset: baseline.asset },
        MediaEdit::ReloadAsset { asset: baseline.asset },
        MediaEdit::ReloadAsset {
            asset: AssetId::from_uuid(baseline.references[0][0].channel.0.0),
        },
        MediaEdit::RelinkAsset {
            asset: baseline.asset,
            locator: relative("replacement.png"),
        },
        MediaEdit::RemoveAsset { asset: baseline.asset },
    ];
    for [first, second] in baseline.references {
        let mut definition = channel(&initial, first).unwrap().definition;
        definition.size = SizePolicy::Fixed {
            extent: Extent2D::new(17, 9).unwrap(),
        };
        definition.filter = golden_video::plan::SampleFilter::Nearest;
        match &mut definition.source {
            ChannelSource::SolidColor { rgba } => *rgba = [0.75, 0.25, 0.5, 0.25],
            ChannelSource::Picture { asset } => *asset = None,
            ChannelSource::Spout { sender, .. } => *sender = Some("Exact replacement".into()),
            _ => unreachable!(),
        }
        cases.extend([
            MediaEdit::ConfigureChannel {
                reference: first,
                definition,
            },
            MediaEdit::RenameMedia {
                media: first.media,
                label: "Renamed source".into(),
            },
            MediaEdit::RenameChannel {
                reference: first,
                label: "Renamed output".into(),
            },
            MediaEdit::ReorderChannel {
                reference: first,
                after: Some(second.channel),
            },
            MediaEdit::RemoveChannel { reference: first },
            MediaEdit::RemoveMedia { media: first.media },
            MediaEdit::InspectChannel { reference: first },
        ]);
    }
    cases.push(MediaEdit::ConfigureChannel {
        reference: baseline.references[0][0],
        definition: ChannelDefinition::for_media(MediaKind::Picture),
    });
    cases.push(MediaEdit::RenameChannel {
        reference: ChannelReference {
            media: baseline.references[0][0].media,
            channel: baseline.references[1][0].channel,
        },
        label: "Wrong owner".into(),
    });
    for operation in cases {
        let mut canonical = None;
        for caller in CALLERS {
            let mut engine = loaded(&baseline.json);
            let result = run(&mut engine, caller, operation.clone());
            let actual = (result, inspect(&engine));
            if let Some(expected) = &canonical {
                assert_eq!(&actual, expected, "{caller:?}: {operation:?}");
            } else {
                canonical = Some(actual);
            }
        }
    }
}

#[test]
fn source_creation_returns_committed_defaults_and_identity_for_every_caller() {
    for kind in [MediaKind::SolidColor, MediaKind::Picture, MediaKind::SharedTexture] {
        for caller in CALLERS {
            let mut engine = fixture();
            let value = run(
                &mut engine,
                caller,
                MediaEdit::CreateMedia {
                    media_kind: kind,
                    label: None,
                },
            )
            .unwrap();
            let MediaEditResult::Created { reference } = serde_json::from_value(value).unwrap() else {
                panic!()
            };
            let catalog = inspect(&engine);
            assert_eq!(catalog.media.len(), 1);
            assert_eq!(catalog.media[0].media_kind, kind);
            assert_eq!(catalog.media[0].channels[0].reference, reference);
            assert_eq!(
                channel(&engine, reference).unwrap().definition,
                ChannelDefinition::for_media(kind)
            );
        }
    }
}

#[test]
fn allocating_channel_and_picture_intents_return_owned_ids_without_changing_other_sources() {
    let baseline = baseline();
    let mut cases = baseline
        .references
        .map(|[first, _]| MediaEdit::AddChannel {
            media: first.media,
            label: Some("Allocated output".into()),
        })
        .to_vec();
    cases.extend([
        MediaEdit::ImportAsset {
            locator: relative("imported.png"),
            label: None,
        },
        MediaEdit::ImportPicture {
            locator: relative("new-picture.png"),
            target: None,
            label: None,
        },
        MediaEdit::ImportPicture {
            locator: relative("assigned-picture.png"),
            target: Some(baseline.references[1][1]),
            label: None,
        },
    ]);
    for operation in cases {
        for caller in CALLERS {
            let mut engine = loaded(&baseline.json);
            let before = inspect(&engine);
            let value = run(&mut engine, caller, operation.clone()).unwrap();
            let result: MediaEditResult = serde_json::from_value(value).unwrap();
            let after = inspect(&engine);
            match (&operation, result) {
                (MediaEdit::AddChannel { media, .. }, MediaEditResult::Created { reference }) => {
                    assert_eq!(reference.media, *media);
                    let original = before.media.iter().find(|entry| entry.id == *media).unwrap();
                    assert!(!original.channels.iter().any(|entry| entry.reference == reference));
                    assert_eq!(
                        channel(&engine, reference).unwrap().definition,
                        ChannelDefinition::for_media(original.media_kind)
                    );
                    assert_eq!(
                        after
                            .media
                            .iter()
                            .find(|entry| entry.id == *media)
                            .unwrap()
                            .channels
                            .len(),
                        3
                    );
                    assert_eq!(after.assets, before.assets);
                }
                (MediaEdit::ImportAsset { locator, .. }, MediaEditResult::AssetCreated { asset }) => {
                    assert_eq!(after.media, before.media);
                    assert_eq!(crate::media::asset(&engine, asset).unwrap().locator, *locator);
                    assert_eq!(after.assets.len(), before.assets.len() + 1);
                }
                (
                    MediaEdit::ImportPicture { locator, target, .. },
                    MediaEditResult::PictureImported { reference, asset },
                ) => {
                    assert_eq!(crate::media::asset(&engine, asset).unwrap().locator, *locator);
                    let selected = channel(&engine, reference).unwrap();
                    assert_eq!(
                        selected.definition.source,
                        ChannelSource::Picture { asset: Some(asset) }
                    );
                    assert_eq!(after.assets.len(), before.assets.len() + 1);
                    if let Some(target) = target {
                        assert_eq!(reference, *target);
                        let previous = channel(&loaded(&baseline.json), *target).unwrap();
                        assert_eq!(selected.definition.size, previous.definition.size);
                        assert_eq!(selected.definition.filter, previous.definition.filter);
                    } else {
                        assert_eq!(after.media.len(), before.media.len() + 1);
                        assert!(!before.media.iter().any(|entry| entry.id == reference.media));
                        assert_eq!(selected.definition.size, SizePolicy::Automatic);
                    }
                }
                _ => panic!("unexpected allocation reply: {caller:?} {operation:?}"),
            }
            for media in &before.media {
                for previous in &media.channels {
                    if matches!(&operation, MediaEdit::ImportPicture { target: Some(target), .. } if target == &previous.reference)
                    {
                        continue;
                    }
                    assert_eq!(
                        channel(&engine, previous.reference).unwrap(),
                        *previous,
                        "independent source changed"
                    );
                }
            }
        }
    }
}

#[test]
fn all_source_definitions_and_shared_asset_references_survive_project_and_history_roundtrip() {
    let baseline = baseline();
    let mut engine = loaded(&baseline.json);
    let before = inspect(&engine);
    for [first, _] in baseline.references {
        let saved = channel(&engine, first).unwrap();
        apply(&mut engine, MediaEdit::RemoveChannel { reference: first });
        assert!(engine.undo().unwrap());
        assert_eq!(channel(&engine, first).unwrap(), saved);
    }
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let restored = loaded(&json);
    crate::media::validate_project(&restored).unwrap();
    assert_eq!(inspect(&restored), before);
    let full = engine.to_project_file_with(Node::project_encode_data).unwrap();
    let mut full_restored = AppEngine::from_project_file_with(full, AppNode::project_decode_node).unwrap();
    full_restored.apply_edits().unwrap();
    crate::media::validate_project(&full_restored).unwrap();
    assert_eq!(inspect(&full_restored), before);
    for [first, second] in baseline.references {
        assert_eq!(
            channel(&restored, first).unwrap().definition.size,
            SizePolicy::Automatic
        );
        assert_eq!(
            channel(&restored, second).unwrap().definition.size,
            SizePolicy::Fixed {
                extent: Extent2D::new(7, 3).unwrap()
            }
        );
    }
    for forbidden in [
        "shared_handle",
        "pool_generation",
        "fingerprint",
        "pixels",
        "retained_bytes",
    ] {
        assert!(!json.contains(forbidden), "runtime resource serialized: {forbidden}");
    }
}
