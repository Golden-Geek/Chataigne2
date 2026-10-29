use super::*;
use crate::app::AppNode;
use crate::media::{channel, validate_project};
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};
use golden_core::node::Node;
use golden_video::Extent2D;

#[test]
fn backend_defaults_and_multi_channel_edits_use_one_history_step_each() {
    let mut engine = fixture();
    let before = engine.undo_len();
    let first = create(&mut engine, MediaKind::SolidColor);
    assert_eq!(engine.undo_len(), before + 1);
    assert_eq!(
        channel(&engine, first).unwrap().definition,
        ChannelDefinition::for_media(MediaKind::SolidColor)
    );
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    assert_eq!(first.media, second.media);
    assert_ne!(first.channel, second.channel);
    let original = channel(&engine, second).unwrap();
    let mut definition = original.definition.clone();
    definition.size = SizePolicy::Fixed {
        extent: Extent2D::new(320, 240).unwrap(),
    };
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: second,
            definition: definition.clone(),
        },
    );
    assert_eq!(channel(&engine, second).unwrap().definition, definition);
    assert_eq!(channel(&engine, first).unwrap().definition.size, SizePolicy::Automatic);
    assert!(engine.undo().unwrap());
    assert_eq!(channel(&engine, second).unwrap(), original);
    assert!(engine.redo().unwrap());
    assert_eq!(channel(&engine, second).unwrap().definition, definition);
}

#[test]
fn rename_reorder_remove_undo_and_project_reload_preserve_exact_references() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::SharedTexture);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    apply(
        &mut engine,
        MediaEdit::RenameMedia {
            media: first.media,
            label: "Renamed".into(),
        },
    );
    apply(
        &mut engine,
        MediaEdit::RenameChannel {
            reference: first,
            label: "Input A".into(),
        },
    );
    apply(
        &mut engine,
        MediaEdit::ReorderChannel {
            reference: first,
            after: Some(second.channel),
        },
    );
    let saved = channel(&engine, first).unwrap();
    apply(&mut engine, MediaEdit::RemoveChannel { reference: first });
    assert!(channel(&engine, first).is_err());
    assert!(channel(&engine, second).is_ok());
    assert!(engine.undo().unwrap());
    assert_eq!(channel(&engine, first).unwrap(), saved);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    validate_project(&loaded).unwrap();
    assert_eq!(channel(&loaded, first).unwrap(), saved);
    let parent = loaded.node_id_by_uuid(first.media.0).unwrap();
    let last = loaded.nodes.get(parent).unwrap().node_data().last_child.unwrap();
    assert_eq!(loaded.nodes.get(last).unwrap().node_data().meta.uuid, first.channel.0);
    apply(&mut loaded, MediaEdit::RemoveMedia { media: first.media });
    assert!(channel(&loaded, first).is_err());
    assert!(loaded.undo().unwrap());
    assert_eq!(channel(&loaded, first).unwrap(), saved);
}

#[test]
fn invalid_source_kind_wrong_owner_and_last_channel_removal_are_atomic() {
    let mut engine = fixture();
    let solid = create(&mut engine, MediaKind::SolidColor);
    let spout = create(&mut engine, MediaKind::SharedTexture);
    let original = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let invalid = [
        MediaEdit::ConfigureChannel {
            reference: solid,
            definition: ChannelDefinition::for_media(MediaKind::SharedTexture),
        },
        MediaEdit::InspectChannel {
            reference: ChannelReference {
                media: solid.media,
                channel: spout.channel,
            },
        },
        MediaEdit::ReorderChannel {
            reference: solid,
            after: Some(spout.channel),
        },
        MediaEdit::RemoveChannel { reference: solid },
        MediaEdit::RenameChannel {
            reference: solid,
            label: "  ".into(),
        },
    ];
    for edit in invalid {
        assert!(!invoke(&mut engine, edit).success);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), original);
    }
}

#[test]
fn source_configuration_is_authored_but_observation_and_resources_are_not() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::SharedTexture);
    let mut definition = channel(&engine, reference).unwrap().definition;
    let ChannelSource::Spout { sender, .. } = &mut definition.source else {
        panic!()
    };
    *sender = Some("Exact Sender".into());
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference,
            definition: definition.clone(),
        },
    );
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(json.contains("Exact Sender"));
    for forbidden in ["observed_descriptor", "shared_handle", "device_generation", "sdk_calls"] {
        assert!(!json.contains(forbidden));
    }
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(channel(&loaded, reference).unwrap().definition, definition);
}

#[test]
fn production_actor_creates_and_saves_the_same_committed_contract() {
    use golden_core::app::prepare_engine_for_runtime;
    use golden_core::application::{ProductionRuntime, ProjectSaveRequest};
    use golden_core::ui_sync::UiProjectFileSpec;
    let mut engine = fixture();
    prepare_engine_for_runtime(&mut engine).unwrap();
    let runtime = ProductionRuntime::new(engine, UiProjectFileSpec::default()).unwrap();
    let applied = runtime
        .apply_ui_transaction(
            golden_core::ui_sync::UiEditIntent::InvokeAppOperation {
                namespace: MEDIA_NAMESPACE.into(),
                operation: MEDIA_OPERATION.into(),
                payload: serde_json::to_value(MediaEdit::CreateMedia {
                    media_kind: MediaKind::SolidColor,
                    label: None,
                })
                .unwrap(),
            },
            None,
        )
        .unwrap();
    assert!(applied.acknowledgement.success, "{:?}", applied.acknowledgement);
    let MediaEditResult::Created { reference } =
        serde_json::from_value(applied.acknowledgement.result.unwrap().value).unwrap()
    else {
        panic!()
    };
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("media.mapgyver");
    runtime
        .save_project(ProjectSaveRequest {
            path: path.to_string_lossy().into_owned(),
            ui_state: None,
        })
        .unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&std::fs::read_to_string(path).unwrap()).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(
        channel(&loaded, reference).unwrap().definition,
        ChannelDefinition::for_media(MediaKind::SolidColor)
    );
}
