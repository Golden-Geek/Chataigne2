//! Media, Matrix and application operations preserve one committed contract for every caller.
use super::*;
use crate::media::channel;
use golden_core::app::ProjectNode;
use golden_video::Extent2D;

mod applications;
mod cell_transfer_scale;
mod cell_transfers;
mod composition;
mod matrix;
mod picture_file;

#[derive(Clone, Copy, Debug)]
enum Caller {
    Ui,
    QuickJs,
    HostedQuickJs,
    NamedQuickJs,
}
const CALLERS: [Caller; 4] = [Caller::Ui, Caller::QuickJs, Caller::HostedQuickJs, Caller::NamedQuickJs];

fn named_invocation(operation: &MediaEdit) -> String {
    let (method, args) = match operation {
        MediaEdit::Matrix {
            media,
            edit:
                crate::matrix::protocol::MatrixEdit::TransferCell {
                    cell,
                    layer,
                    column,
                    action,
                },
        } => (
            "transferMatrixCell",
            serde_json::json!([media, cell, layer, column, action]),
        ),
        MediaEdit::ConfigurePictureFile { reference, file } => (
            "configurePictureFile",
            serde_json::json!([reference.media, reference.channel, file]),
        ),
        MediaEdit::InspectPictureFile { reference } => (
            "inspectPictureFile",
            serde_json::json!([reference.media, reference.channel]),
        ),
        MediaEdit::ReloadPicture { reference } => {
            ("reloadPicture", serde_json::json!([reference.media, reference.channel]))
        }
        MediaEdit::Applications { edit } => match edit {
            crate::applications::protocol::ApplicationEdit::SetFile {
                application,
                executable,
            } => (
                "setApplicationFile",
                serde_json::json!([application, executable.as_deref().unwrap_or("")]),
            ),
            crate::applications::protocol::ApplicationEdit::Configure {
                application,
                definition,
            } => (
                "configureApplication",
                serde_json::json!([application, serde_json::to_string(definition).unwrap()]),
            ),
            crate::applications::protocol::ApplicationEdit::Inspect { application } => {
                ("inspectApplication", serde_json::json!([application]))
            }
            crate::applications::protocol::ApplicationEdit::InspectRuntime {} => {
                ("inspectApplicationRuntime", serde_json::json!([]))
            }
            crate::applications::protocol::ApplicationEdit::InspectCallbacks {} => {
                ("inspectApplicationCallbacks", serde_json::json!([]))
            }
            _ => {
                return format!(
                    "local.editMedia('parity', {:?});",
                    serde_json::to_string(operation).unwrap()
                );
            }
        },
        MediaEdit::Matrix {
            media,
            edit: crate::matrix::protocol::MatrixEdit::LaunchColumn { column, mode },
        } => ("launchMatrixColumn", serde_json::json!([media, column, mode])),
        MediaEdit::Matrix {
            media,
            edit: crate::matrix::protocol::MatrixEdit::LaunchCell { layer, column, mode },
        } => ("launchMatrixCell", serde_json::json!([media, layer, column, mode])),
        MediaEdit::Matrix {
            media,
            edit: crate::matrix::protocol::MatrixEdit::ClearPlayback,
        } => ("unloadMatrix", serde_json::json!([media])),
        MediaEdit::Matrix {
            media,
            edit:
                crate::matrix::protocol::MatrixEdit::DropSource {
                    layer,
                    column,
                    source,
                    copy,
                },
        } => (
            "dropMatrixMedia",
            serde_json::json!([media, layer, column, source, copy]),
        ),
        MediaEdit::Composition {
            reference,
            edit:
                crate::media::composition::protocol::CompositionEdit::DropSource {
                    source,
                    instance,
                    position,
                    copy,
                },
        } => (
            "dropCompositionMedia",
            serde_json::json!([
                reference.media,
                reference.channel,
                source,
                instance.map(|id| id.0.0.to_string()).unwrap_or_default(),
                copy,
                position.x,
                position.y
            ]),
        ),
        MediaEdit::Composition {
            reference,
            edit:
                crate::media::composition::protocol::CompositionEdit::CreateMedia {
                    media_kind,
                    label,
                    position,
                },
        } => (
            "createCompositionMedia",
            serde_json::json!([
                reference.media,
                reference.channel,
                media_kind,
                label.as_deref().unwrap_or(""),
                position.x,
                position.y
            ]),
        ),
        MediaEdit::CreateMedia { media_kind, label } => (
            "createMedia",
            serde_json::json!([media_kind, label.as_deref().unwrap_or("")]),
        ),
        MediaEdit::AddChannel { media, label } => {
            ("addChannel", serde_json::json!([media, label.as_deref().unwrap_or("")]))
        }
        MediaEdit::ConfigureMediaFitMode { media, mode } => ("configureMediaFitMode", serde_json::json!([media, mode])),
        MediaEdit::ConfigureMediaSenderSelection { media, mode } => {
            ("configureMediaSenderSelection", serde_json::json!([media, mode]))
        }
        MediaEdit::ConfigureMediaResolution { media, size } => {
            let (enabled, width, height) = match size {
                SizePolicy::Automatic => (false, 0, 0),
                SizePolicy::Fixed { extent } => (true, extent.width(), extent.height()),
            };
            (
                "configureMediaResolution",
                serde_json::json!([media, enabled, width, height]),
            )
        }
        MediaEdit::ConfigureChannel { reference, source } => match source {
            ChannelSource::SolidColor { rgba } => (
                "configureSolidColor",
                serde_json::json!([reference.media, reference.channel, rgba[0], rgba[1], rgba[2], rgba[3]]),
            ),
            ChannelSource::Picture { file } => (
                "configurePicture",
                serde_json::json!([
                    reference.media,
                    reference.channel,
                    file.as_ref()
                        .map(crate::media::assets::locator_path)
                        .unwrap_or_default()
                ]),
            ),
            ChannelSource::Spout { sender } => (
                "configureSharedTexture",
                serde_json::json!([reference.media, reference.channel, sender.as_deref().unwrap_or("")]),
            ),
            _ => {
                return format!(
                    "local.editMedia('parity', {:?});",
                    serde_json::to_string(operation).unwrap()
                );
            }
        },
        MediaEdit::InspectChannel { reference } => (
            "inspectChannel",
            serde_json::json!([reference.media, reference.channel]),
        ),
        MediaEdit::ImportPicture {
            locator,
            target: None,
            label: None,
        } => {
            let (path, absolute) = match locator {
                AssetLocator::ProjectRelative { path } => (path.as_str(), false),
                AssetLocator::Absolute { path } => (path.as_str(), true),
            };
            ("importPicture", serde_json::json!([path, absolute]))
        }
        _ => {
            return format!(
                "local.editMedia('parity', {:?});",
                serde_json::to_string(operation).unwrap()
            );
        }
    };
    let args = args
        .as_array()
        .unwrap()
        .iter()
        .map(|arg| serde_json::to_string(arg).unwrap())
        .collect::<Vec<_>>()
        .join(",");
    format!("local.{method}('parity',{args});")
}

fn relative(path: &str) -> AssetLocator {
    AssetLocator::ProjectRelative {
        path: path.to_owned().try_into().unwrap(),
    }
}
fn inspect(engine: &AppEngine) -> MediaManagerSnapshot {
    crate::media::catalog::inspect(engine).unwrap()
}

type ApplicationState = (
    crate::applications::protocol::ApplicationId,
    String,
    crate::applications::protocol::ApplicationEnablement,
    crate::applications::protocol::ApplicationDefinition,
);

fn authored(
    engine: &AppEngine,
) -> (
    MediaManagerSnapshot,
    Vec<crate::media::composition::protocol::CompositionSnapshot>,
    Vec<crate::matrix::protocol::MatrixSnapshot>,
    Vec<ApplicationState>,
) {
    let catalog = inspect(engine);
    let references = catalog
        .media
        .iter()
        .flat_map(|media| &media.channels)
        .filter(|channel| matches!(channel.definition.source, ChannelSource::Composition { .. }))
        .map(|channel| channel.reference)
        .collect::<Vec<_>>();
    let compositions = references
        .into_iter()
        // An immutable query must not settle queued script operations before history is measured.
        .map(|reference| crate::media::composition::inspect(engine, reference).unwrap())
        .collect();
    let matrices = catalog
        .media
        .iter()
        .filter(|media| media.media_kind == MediaKind::Matrix)
        .map(|media| crate::matrix::inspect(engine, media.id).unwrap())
        .collect();
    let mut applications = engine
        .nodes
        .iter()
        .filter_map(|(_, node)| match node {
            AppNode::Application(node) => {
                let snapshot = node.snapshot(engine).unwrap();
                // Runtime revisions and recreated node instances are not persistent authoring.
                Some((snapshot.id, snapshot.label, snapshot.enablement, snapshot.definition))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    applications.sort_by_key(|(application, ..)| application.0.0);
    (catalog, compositions, matrices, applications)
}

/// The observer is installed before measuring history. Only the operation itself earns a step.
fn run(engine: &mut AppEngine, caller: Caller, operation: MediaEdit) -> Option<serde_json::Value> {
    run_measured(engine, caller, operation).0
}

fn run_measured(
    engine: &mut AppEngine,
    caller: Caller,
    operation: MediaEdit,
) -> (Option<serde_json::Value>, std::time::Duration) {
    let request = serde_json::to_string(&operation).unwrap();
    match caller {
        Caller::Ui => {}
        Caller::QuickJs | Caller::HostedQuickJs | Caller::NamedQuickJs => {
            let invoke = if matches!(caller, Caller::QuickJs) {
                format!("gc.invokeAppOperation('parity', {MEDIA_NAMESPACE:?}, {MEDIA_OPERATION:?}, {request});")
            } else if matches!(caller, Caller::NamedQuickJs) {
                format!("local.listen({{level:1}}); {}", named_invocation(&operation))
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
    }
    let before = authored(engine);
    let history = engine.undo_len();
    let started = std::time::Instant::now();
    let result = if matches!(caller, Caller::Ui) {
        let ack = invoke(engine, operation);
        ack.success.then(|| ack.result.unwrap().value)
    } else {
        service_script_app_operations(engine).unwrap();
        let responses = replies(engine);
        assert_eq!(responses.len(), 1, "{caller:?} must emit one committed callback");
        assert_eq!(responses[0]["request_id"], "parity");
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
    // A wrapper callback identifies its actual Parameter; the Parameter's deferred action
    // is a separate bounded service turn and must run before history replay can cancel it.
    service_script_app_operations(engine).unwrap();
    let elapsed = started.elapsed();
    let after = authored(engine);
    if after != before {
        assert!(result.is_some());
        assert_eq!(engine.undo_len(), history + 1, "{caller:?}");
        assert!(engine.undo().unwrap());
        assert_eq!(authored(engine), before, "{caller:?} undo");
        assert!(engine.redo().unwrap());
        assert_eq!(authored(engine), after, "{caller:?} redo");
    } else {
        assert_eq!(
            engine.undo_len(),
            history,
            "inspection/rejection must not author history"
        );
    }
    service_script_app_operations(engine).unwrap();
    assert_eq!(authored(engine), after, "history replay must not requeue the script");
    if !matches!(caller, Caller::Ui) {
        assert_eq!(replies(engine).len(), 1);
    }
    // JavaScript JSON numbers do not retain serde's integer/float representation. Decode the
    // actual public DTO before comparison, preserving typed values rather than lexical numbers.
    (
        result.map(|value| serde_json::to_value(serde_json::from_value::<MediaEditResult>(value).unwrap()).unwrap()),
        elapsed,
    )
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
                ChannelSource::Picture { file: selected } => *selected = Some(relative("first.png")),
                ChannelSource::Spout { sender } => {
                    *sender = Some(format!("Source {index}"));
                }
                _ => {}
            }
            apply(
                &mut engine,
                MediaEdit::ConfigureChannel {
                    reference,
                    source: definition.source,
                },
            );
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
    golden_core::app::prepare_engine_for_runtime(&mut engine).unwrap();
    engine
}

#[test]
fn source_mutations_inspection_and_atomic_rejection_match_ui_and_script_callers() {
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
        MediaEdit::InspectChannel {
            reference: ChannelReference {
                media: baseline.references[0][0].media,
                channel: ChannelId(golden_core::node::NodeUuid::nil()),
            },
        },
        MediaEdit::InspectChannel {
            reference: ChannelReference {
                media: baseline.references[0][0].media,
                channel: baseline.references[1][0].channel,
            },
        },
    ];
    for [first, second] in baseline.references {
        let mut definition = channel(&initial, first).unwrap().definition;
        match &mut definition.source {
            ChannelSource::SolidColor { rgba } => *rgba = [0.75, 0.25, 0.5, 0.25],
            ChannelSource::Picture { file } => *file = None,
            ChannelSource::Spout { sender, .. } => *sender = Some("Exact replacement".into()),
            _ => unreachable!(),
        }
        cases.extend([
            MediaEdit::ConfigureMediaFitMode {
                media: first.media,
                mode: MediaFitMode::Fill,
            },
            MediaEdit::ConfigureMediaResolution {
                media: first.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(17, 9).unwrap(),
                },
            },
            MediaEdit::ConfigureMediaResolution {
                media: first.media,
                size: SizePolicy::Automatic,
            },
            MediaEdit::ConfigureMediaResolution {
                media: first.media,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(16385, 1).unwrap(),
                },
            },
            MediaEdit::ConfigureChannel {
                reference: first,
                source: definition.source,
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
            MediaEdit::MoveChannel {
                reference: first,
                direction: ChannelMoveDirection::Earlier,
            },
            MediaEdit::MoveChannel {
                reference: first,
                direction: ChannelMoveDirection::Later,
            },
            MediaEdit::RemoveChannel { reference: first },
            MediaEdit::RemoveMedia { media: first.media },
            MediaEdit::InspectChannel { reference: first },
        ]);
    }
    cases.push(MediaEdit::ConfigureMediaSenderSelection {
        media: baseline.references[2][0].media,
        mode: SenderSelectionMode::Auto,
    });
    cases.push(MediaEdit::ConfigureChannel {
        reference: baseline.references[0][0],
        source: ChannelDefinition::for_media(MediaKind::Picture).source,
    });
    cases.push(MediaEdit::ConfigureChannel {
        reference: baseline.references[1][0],
        source: ChannelSource::Picture {
            file: Some(relative("first.png")),
        },
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
            if let MediaEdit::InspectChannel { reference } = &operation {
                if reference.channel.0 == golden_core::node::NodeUuid::nil() {
                    assert!(result.as_ref().unwrap()["snapshot"].is_null(), "{caller:?}");
                } else if reference.media == baseline.references[0][0].media
                    && reference.channel == baseline.references[1][0].channel
                {
                    assert!(result.is_none(), "{caller:?}: existing wrong-owner channel must reject");
                }
            }
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
            locator: AssetLocator::Absolute {
                path: std::env::temp_dir()
                    .join("mapgyver-source-api-picture.png")
                    .to_string_lossy()
                    .into_owned()
                    .try_into()
                    .unwrap(),
            },
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
                    let mut expected = ChannelDefinition::for_media(original.media_kind);
                    expected.size = original.channels[0].definition.size;
                    assert_eq!(channel(&engine, reference).unwrap().definition, expected);
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
                (MediaEdit::ImportPicture { locator, target, .. }, MediaEditResult::PictureImported { reference }) => {
                    let selected = channel(&engine, reference).unwrap();
                    assert_eq!(
                        selected.definition.source,
                        ChannelSource::Picture {
                            file: Some(locator.clone())
                        }
                    );
                    assert_eq!(after.assets, before.assets);
                    if let Some(target) = target {
                        assert_eq!(reference, *target);
                        let previous = channel(&loaded(&baseline.json), *target).unwrap();
                        assert_eq!(selected.definition.size, previous.definition.size);
                        assert_eq!(selected.definition.fit_mode, previous.definition.fit_mode);
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
            SizePolicy::Fixed {
                extent: Extent2D::new(7, 3).unwrap()
            }
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
