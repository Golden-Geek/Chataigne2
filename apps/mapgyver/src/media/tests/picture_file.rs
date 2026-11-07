use super::*;
use crate::{app::AppNode, media::channel};
use golden_core::{
    app::{ProjectLifecycle, from_sparse_project_json, to_sparse_project_json_pretty},
    node::{Node, NodeUuid},
    parameter::ParamValue,
};

fn binding(engine: &mut AppEngine, reference: ChannelReference) -> PictureFileBinding {
    let MediaEditResult::PictureFile { binding: Some(binding) } =
        apply(engine, MediaEdit::InspectPictureFile { reference })
    else {
        panic!()
    };
    binding
}
fn topology(engine: &AppEngine) -> Vec<(NodeUuid, Option<NodeUuid>)> {
    let mut nodes = engine
        .nodes
        .iter()
        .map(|(_, node)| {
            let data = node.node_data();
            (
                data.meta.uuid,
                data.parent
                    .and_then(|id| engine.nodes.get(id))
                    .map(|node| node.node_data().meta.uuid),
            )
        })
        .collect::<Vec<_>>();
    nodes.sort_by_key(|(uuid, _)| uuid.0);
    nodes
}

fn persisted_topology(json: &str) -> Vec<(String, Option<String>)> {
    let project: serde_json::Value = serde_json::from_str(json).unwrap();
    let mut pending = vec![(&project["root"], None)];
    let mut result = Vec::new();
    while let Some((node, parent)) = pending.pop() {
        let uuid = node["uuid"].as_str().unwrap().to_owned();
        result.push((uuid.clone(), parent));
        if let Some(children) = node["children"].as_array() {
            pending.extend(children.iter().map(|child| (child, Some(uuid.clone()))));
        }
    }
    result.sort();
    result
}

#[test]
fn picture_file_is_an_owned_parameter_and_every_edit_preserves_document_topology_and_exact_history() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Picture);
    let address = binding(&mut engine, reference).parameter;
    let shape = topology(&engine);
    for file in ["images/source.png", "images/relinked.png", ""] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        apply(
            &mut engine,
            MediaEdit::ConfigurePictureFile {
                reference,
                file: file.into(),
            },
        );
        assert_eq!(engine.undo_len(), history + 1);
        assert_eq!(topology(&engine), shape);
        assert_eq!(binding(&mut engine, reference).parameter, address);
        assert_eq!(
            engine
                .nodes
                .get(address.node)
                .unwrap()
                .engine_param_snapshot()
                .unwrap()
                .value,
            ParamValue::File(file.into())
        );
        let after = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
    }
    let history = engine.undo_len();
    apply(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference,
            file: String::new(),
        },
    );
    assert_eq!(engine.undo_len(), history);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(binding(&mut loaded, reference).parameter.uuid, address.uuid);
    assert_eq!(loaded.nodes.len(), shape.len());
    // Golden rematerializes default declared children and places Parameters
    // before user items on load. Authored identities/parents must still match.
    assert_eq!(
        persisted_topology(&to_sparse_project_json_pretty(&loaded).unwrap()),
        persisted_topology(&saved)
    );
    assert_eq!(
        crate::media::catalog::inspect(&loaded),
        crate::media::catalog::inspect(&engine)
    );
    assert!(crate::media::catalog::inspect(&loaded).unwrap().assets.is_empty());
}

#[test]
fn picture_file_parameters_are_independent_and_generic_edits_share_the_same_authority_and_validation() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::Picture);
    let second = create(&mut engine, MediaKind::Picture);
    apply(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference: first,
            file: "source.png".into(),
        },
    );
    let source = channel(&engine, first).unwrap().definition.source;
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: second,
            source: source.clone(),
        },
    );
    let first_file = binding(&mut engine, first).parameter;
    let second_file = binding(&mut engine, second).parameter;
    assert_ne!(first_file.uuid, second_file.uuid);
    let shape = topology(&engine);
    let history = engine.undo_len();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: second_file.node,
                value: ParamValue::File("independent.png".into()),
                behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(channel(&engine, first).unwrap().definition.source, source);
    assert_eq!(
        channel(&engine, second).unwrap().definition.source,
        ChannelSource::Picture {
            file: Some(crate::media::assets::parse_file("independent.png").unwrap().unwrap())
        }
    );
    assert_eq!(topology(&engine), shape);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for intent in [
        UiEditIntent::SetParam {
            node: second_file.node,
            value: ParamValue::File("../escape.png".into()),
            behaviour: golden_core::parameter::ParameterEventBehaviour::Append,
        },
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(MediaEdit::ConfigurePictureFile {
                reference: second,
                file: "../escape.png".into(),
            })
            .unwrap(),
        },
    ] {
        assert!(!apply_project_intent(&mut engine, intent).success);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    let solid = create(&mut engine, MediaKind::SolidColor);
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::ConfigurePictureFile {
                reference: solid,
                file: "source.png".into()
            }
        )
        .success
    );
    assert!(serde_json::from_value::<ChannelSource>(serde_json::json!({"kind":"Picture","asset":null})).is_err());
    let old = to_sparse_project_json_pretty(&engine)
        .unwrap()
        .replace("mapgyver.media.channel.v2", "mapgyver.media.channel");
    assert!(from_sparse_project_json::<AppNode>(&old).is_err());
}

#[test]
fn file_binding_query_is_immutable_nullable_after_deletion_and_shares_the_public_result() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Picture);
    for removed in [false, true] {
        if removed {
            apply(&mut engine, MediaEdit::RemoveMedia { media: reference.media });
        }
        let edit = MediaEdit::InspectPictureFile { reference };
        let payload = serde_json::to_value(&edit).unwrap();
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.ui_history_state();
        let query = AppNode::query_app_operation(&engine, MEDIA_NAMESPACE, MEDIA_OPERATION, &payload)
            .unwrap()
            .unwrap();
        let reply = invoke(&mut engine, edit);
        assert!(reply.success);
        assert_eq!(query, reply.result.unwrap().value);
        assert_eq!(query["binding"].is_null(), removed);
        assert_eq!(engine.ui_history_state(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}

#[test]
fn reload_fences_only_the_selected_picture_preserves_redo_and_does_not_author_or_persist_runtime_generations() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::Picture);
    let peer = create(&mut engine, MediaKind::Picture);
    apply(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference: first,
            file: "source.png".into(),
        },
    );
    assert!(engine.undo().unwrap());
    let history = engine.ui_history_state();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let shape = topology(&engine);
    let stale = crate::media::capture_channels(&engine, &[first], 16).unwrap();
    let healthy = crate::media::capture_channels(&engine, &[peer], 16).unwrap();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let notifier = golden_io::WorkerNotifier::new(move || {
        let _ = send.try_send(());
    });
    let subscriptions = stale
        .revision_guards()
        .map(|guard| guard.subscribe(notifier.clone()))
        .collect::<Vec<_>>();
    apply(&mut engine, MediaEdit::ReloadPicture { reference: first });
    assert!(!stale.is_current());
    assert!(healthy.is_current());
    assert_eq!(
        receive.try_recv(),
        Ok(()),
        "Reload wakes the ordinary preview worker without polling"
    );
    drop(subscriptions);
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(topology(&engine), shape);
    let id = super::super::authoring::channel_id(&engine, first).unwrap();
    let AppNode::MediaChannel(channel) = engine.nodes.get(id).unwrap() else {
        panic!()
    };
    assert_eq!(channel.picture_reload_generation(), 1);
    assert!(engine.redo().unwrap());
    assert_eq!(channel_file(&engine, first), Some("source.png".into()));
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    let id = super::super::authoring::channel_id(&loaded, first).unwrap();
    let AppNode::MediaChannel(channel) = loaded.nodes.get(id).unwrap() else {
        panic!()
    };
    assert_eq!(
        channel.picture_reload_generation(),
        0,
        "load must not replay a runtime Reload"
    );
    assert_eq!(channel_file(&loaded, first), Some("source.png".into()));
}

fn channel_file(engine: &AppEngine, reference: ChannelReference) -> Option<String> {
    match channel(engine, reference).unwrap().definition.source {
        ChannelSource::Picture { file } => file
            .as_ref()
            .map(|locator| super::super::assets::locator_path(locator).to_owned()),
        _ => panic!(),
    }
}
