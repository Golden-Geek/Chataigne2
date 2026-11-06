use super::*;
use crate::{
    app::AppNode,
    media::{channel, sender_catalog::Source},
};
use golden_core::{
    app::{from_sparse_project_json, prepare_engine_for_runtime, to_sparse_project_json_pretty},
    application::service_script_app_operations,
    edit::Edit,
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    script::ScriptMethodCall,
};

fn publish(engine: &mut AppEngine, names: &[&str]) {
    Source::capture(engine)
        .unwrap()
        .publish(&names.iter().map(|s| (*s).into()).collect::<Vec<_>>())
        .unwrap();
    engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
}

fn with_catalog(names: &[&str]) -> AppEngine {
    let mut engine = fixture();
    prepare_engine_for_runtime(&mut engine).unwrap();
    publish(&mut engine, names);
    engine
}

fn reference(engine: &AppEngine, media: MediaId) -> ChannelReference {
    let owner = engine.node_id_by_uuid(media.0).unwrap();
    let channel = engine
        .nodes
        .iter()
        .find_map(|(_, node)| match node {
            AppNode::MediaChannel(channel) if channel.node_data().parent == Some(owner) => {
                Some(channel.node_data().meta.uuid)
            }
            _ => None,
        })
        .unwrap();
    ChannelReference {
        media,
        channel: ChannelId(channel),
    }
}

fn check(engine: &AppEngine, reference: ChannelReference, expected: Option<&str>) {
    let media = engine
        .nodes
        .get(engine.node_id_by_uuid(reference.media.0).unwrap())
        .unwrap();
    let AppNode::Media(media) = media else { panic!() };
    assert_eq!(media.sender_selection(), Some(SenderSelectionMode::Auto));
    assert_eq!(
        channel(engine, reference).unwrap().definition.source,
        ChannelSource::Spout {
            sender: expected.map(str::to_owned)
        }
    );
    let node = engine
        .nodes
        .get(engine.node_id_by_uuid(reference.channel.0).unwrap())
        .unwrap();
    let AppNode::MediaChannel(node) = node else { panic!() };
    let state = engine
        .nodes
        .get(node.sender_parameter(SenderSelectionMode::Auto))
        .unwrap()
        .engine_param_snapshot()
        .unwrap();
    let options = state.constraints.enum_options.as_slice();
    assert_eq!(options[0].label, "None");
    assert_eq!(options[0].value, ParamValue::Enum(String::new()));
    assert_eq!(options.get(1).map(|entry| entry.variant_id.as_str()), expected);
    assert_eq!(state.value, ParamValue::Enum(expected.unwrap_or_default().into()));
}

fn generic(engine: &mut AppEngine, parent: NodeId) -> ChannelReference {
    let before = engine.undo_len();
    let ack = apply_project_intent(
        engine,
        UiEditIntent::CreateUserItem {
            parent,
            node_type: "mapgyver.media.shared_texture".into(),
            label: Some("Generic sender".into()),
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(engine.undo_len(), before + 1);
    let media = engine
        .nodes
        .iter()
        .find_map(|(_, node)| {
            (node.node_data().parent == Some(parent) && node.node_data().meta.label == "Generic sender")
                .then_some(MediaId(node.node_data().meta.uuid))
        })
        .unwrap();
    reference(engine, media)
}

#[test]
fn first_media_uses_one_snapshot_and_one_history_entry_for_zero_one_or_many_senders() {
    for names in [vec![], vec!["Only sender"], vec!["Zulu first", "Alpha second"]] {
        let mut engine = with_catalog(&names);
        let history = engine.undo_len();
        let reference = create(&mut engine, MediaKind::SharedTexture);
        check(&engine, reference, names.first().copied());
        assert_eq!(engine.undo_len(), history + 1);
        publish(&mut engine, &["Replacement"]);
        assert!(engine.undo().unwrap());
        assert!(channel(&engine, reference).is_err());
        assert!(engine.redo().unwrap());
        assert_eq!(
            channel(&engine, reference).unwrap().definition.source,
            ChannelSource::Spout {
                sender: names.first().map(|s| (*s).into())
            }
        );
    }
}

#[test]
fn manager_folder_and_owned_creation_paths_share_first_sender_and_immediate_enum_metadata() {
    use crate::matrix::protocol::{MatrixEdit, MatrixItemId};
    use crate::media::composition::protocol::{CompositionEdit, Point};
    for names in [vec![], vec!["Only"], vec!["Second alphabetically", "A later entry"]] {
        let mut engine = with_catalog(&names);
        let expected = names.first().copied();
        let manager = crate::media::authoring::manager(&engine).unwrap();
        let created = generic(&mut engine, manager);
        check(&engine, created, expected);
        let MediaEditResult::FolderCreated { folder } = apply(
            &mut engine,
            MediaEdit::CreateFolder {
                parent: None,
                label: None,
            },
        ) else {
            panic!()
        };
        let parent = engine.node_id_by_uuid(folder).unwrap();
        let created = generic(&mut engine, parent);
        check(&engine, created, expected);
        let MediaEditResult::Created { reference: created } = apply(
            &mut engine,
            MediaEdit::CreateMediaInFolder {
                folder,
                media_kind: MediaKind::SharedTexture,
                label: None,
            },
        ) else {
            panic!()
        };
        check(&engine, created, expected);
        let composition = create(&mut engine, MediaKind::Composition);
        let MediaEditResult::InstancesCreated { instances } = apply(
            &mut engine,
            MediaEdit::Composition {
                reference: composition,
                edit: CompositionEdit::CreateMedia {
                    media_kind: MediaKind::SharedTexture,
                    label: None,
                    position: Point { x: 1., y: 2. },
                },
            },
        ) else {
            panic!()
        };
        check(&engine, reference(&engine, MediaId(instances[0].instance.0)), expected);
        let matrix = create(&mut engine, MediaKind::Matrix);
        let MediaEditResult::Matrix { snapshot } = apply(
            &mut engine,
            MediaEdit::Matrix {
                media: matrix.media,
                edit: MatrixEdit::Inspect,
            },
        ) else {
            panic!()
        };
        let layer = snapshot
            .items
            .iter()
            .find_map(|item| match item.id {
                MatrixItemId::Layer { id } => Some(id),
                _ => None,
            })
            .unwrap();
        let MediaEditResult::MatrixMediaCreated { media, .. } = apply(
            &mut engine,
            MediaEdit::Matrix {
                media: matrix.media,
                edit: MatrixEdit::CreateCellMedia {
                    layer,
                    column: snapshot.columns[0].id,
                    media_kind: MediaKind::SharedTexture,
                },
            },
        ) else {
            panic!()
        };
        check(&engine, reference(&engine, media), expected);
    }
}

#[test]
fn named_script_and_command_creation_receive_backend_defaults() {
    for names in [vec![], vec!["Only"], vec!["Z first", "A second"]] {
        let mut engine = with_catalog(&names);
        let manager = crate::media::authoring::manager(&engine).unwrap();
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: manager,
            call: ScriptMethodCall::prepare(
                "createMedia",
                vec![
                    ParamValue::Str("defaults-script".into()),
                    ParamValue::Str("SharedTexture".into()),
                    ParamValue::Str("Script sender".into()),
                ],
            )
            .unwrap(),
        });
        engine.apply_edits().unwrap();
        engine.apply_edits().unwrap();
        service_script_app_operations(&mut engine).unwrap();
        let snapshot = crate::media::catalog::inspect(&engine).unwrap();
        let scripted = snapshot
            .media
            .iter()
            .find(|media| media.label == "Script sender")
            .unwrap()
            .channels[0]
            .reference;
        check(&engine, scripted, names.first().copied());
        let MediaEditResult::CommandCreated { command } = apply(
            &mut engine,
            MediaEdit::CreateCommand {
                command_kind: MediaCommandKind::CreateMedia,
                label: None,
            },
        ) else {
            panic!()
        };
        let owner = engine.node_id_by_uuid(command).unwrap();
        let folder = engine.nodes.get(owner).unwrap().node_data().first_child.unwrap();
        let parameter = engine
            .nodes
            .iter()
            .find_map(|(id, node)| {
                (node.node_data().parent == Some(folder) && node.node_data().meta.decl_id.0 == "parameters/media_kind")
                    .then_some(id)
            })
            .unwrap();
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::SetParam {
                node: parameter,
                value: ParamValue::Enum("SharedTexture".into()),
                behaviour: ParameterEventBehaviour::Append,
            },
        );
        assert!(ack.success, "{ack:?}");
        let MediaEditResult::Created { reference: created } = apply(&mut engine, MediaEdit::ExecuteCommand { command })
        else {
            panic!()
        };
        check(&engine, created, names.first().copied());
    }
}

#[test]
fn explicit_none_exact_choices_and_manual_sparse_projects_are_never_reseeded() {
    let mut engine = with_catalog(&["First", "Second"]);
    let first = create(&mut engine, MediaKind::SharedTexture);
    let second = create(&mut engine, MediaKind::SharedTexture);
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: first,
            source: ChannelSource::Spout { sender: None },
        },
    );
    apply(
        &mut engine,
        MediaEdit::ConfigureMediaSenderSelection {
            media: second.media,
            mode: SenderSelectionMode::Manual,
        },
    );
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: second,
            source: ChannelSource::Spout {
                sender: Some("Exact absent".into()),
            },
        },
    );
    let history = engine.undo_len();
    for names in [vec![], vec!["Different"], vec!["Exact absent", "First"]] {
        publish(&mut engine, &names);
        assert_eq!(
            channel(&engine, first).unwrap().definition.source,
            ChannelSource::Spout { sender: None }
        );
        assert_eq!(
            channel(&engine, second).unwrap().definition.source,
            ChannelSource::Spout {
                sender: Some("Exact absent".into())
            }
        );
        assert_eq!(engine.undo_len(), history);
    }
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    prepare_engine_for_runtime(&mut loaded).unwrap();
    publish(&mut loaded, &["Other after reload"]);
    assert_eq!(
        channel(&loaded, first).unwrap().definition.source,
        ChannelSource::Spout { sender: None }
    );
    let AppNode::Media(media) = loaded
        .nodes
        .get(loaded.node_id_by_uuid(second.media.0).unwrap())
        .unwrap()
    else {
        panic!()
    };
    assert_eq!(media.sender_selection(), Some(SenderSelectionMode::Manual));
    assert_eq!(
        channel(&loaded, second).unwrap().definition.source,
        ChannelSource::Spout {
            sender: Some("Exact absent".into())
        }
    );
    for original in [first, second] {
        let before = crate::media::catalog::inspect(&loaded).unwrap();
        let source = loaded.node_id_by_uuid(original.media.0).unwrap();
        let parent = loaded.nodes.get(source).unwrap().node_data().parent.unwrap();
        let ack = apply_project_intent(
            &mut loaded,
            UiEditIntent::DuplicateNode {
                source,
                new_parent: parent,
                new_prev_sibling: Some(source),
                initial_params: vec![],
            },
        );
        assert!(ack.success, "{ack:?}");
        let after = crate::media::catalog::inspect(&loaded).unwrap();
        let copy = after
            .media
            .iter()
            .find(|media| !before.media.iter().any(|old| old.id == media.id))
            .unwrap();
        let old = before.media.iter().find(|media| media.id == original.media).unwrap();
        assert_eq!(copy.sender_selection, old.sender_selection);
        assert_eq!(copy.channels[0].definition.source, old.channels[0].definition.source);
    }
    apply(
        &mut loaded,
        MediaEdit::ConfigureMediaSenderSelection {
            media: second.media,
            mode: SenderSelectionMode::Auto,
        },
    );
    assert_eq!(
        channel(&loaded, second).unwrap().definition.source,
        ChannelSource::Spout {
            sender: Some("Exact absent".into())
        }
    );
}
