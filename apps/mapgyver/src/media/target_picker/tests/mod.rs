use super::*;
use crate::media::{
    protocol::*,
    tests::{apply, create, fixture, invoke},
};
use golden_core::{
    app::{ProjectLifecycle, from_sparse_project_json, to_sparse_project_json_pretty},
    edit::NodeTree,
};

fn reference(engine: &mut AppEngine, target: Option<MediaId>) -> (MediaId, NodeUuid, NodeUuid) {
    let MediaEditResult::ReferenceCreated { media } = apply(engine, MediaEdit::CreateReference { target, label: None })
    else {
        panic!()
    };
    let AppNode::ReferenceMedia(node) = engine.nodes.get(engine.node_id_by_uuid(media.0).unwrap()).unwrap() else {
        panic!()
    };
    (
        media,
        engine.nodes.get(node.target_parameter()).unwrap().node_data().meta.uuid,
        engine
            .nodes
            .get(node.channel_parameter())
            .unwrap()
            .node_data()
            .meta
            .uuid,
    )
}
fn query(parameter: NodeUuid) -> TargetPickerQuery {
    TargetPickerQuery {
        parameter,
        folder: None,
        cursor: None,
        backwards: false,
        limit: 64,
    }
}
fn choose(engine: &mut AppEngine, parameter: NodeUuid, target: Option<NodeUuid>) {
    apply(engine, MediaEdit::SelectTargetPickerChoice { parameter, target });
}

#[test]
fn removed_parameter_reads_are_absent_while_selection_writes_still_reject() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let (media, parameter, _) = reference(&mut engine, Some(source.media));
    apply(&mut engine, MediaEdit::RemoveMedia { media });
    let history = engine.ui_history_state();
    let operation = MediaEdit::InspectTargetPicker {
        query: query(parameter),
    };
    let result = AppNode::query_app_operation(
        &engine,
        MEDIA_NAMESPACE,
        MEDIA_OPERATION,
        &serde_json::to_value(&operation).unwrap(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result, serde_json::json!({ "kind": "TargetPicker", "page": null }));
    let ack = invoke(&mut engine, operation);
    assert!(ack.success);
    assert_eq!(ack.result.unwrap().value, result);
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::SelectTargetPickerChoice {
                parameter,
                target: Some(source.media.0),
            }
        )
        .success
    );
    assert_eq!(engine.ui_history_state(), history);
    assert!(engine.undo().unwrap());
    assert!(read(&engine, query(parameter)).unwrap().is_some());
}

#[test]
fn media_browse_is_catalog_only_and_current_nested_binding_does_not_open_its_hierarchy() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let MediaEditResult::FolderCreated { folder } = apply(
        &mut engine,
        MediaEdit::CreateFolder {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::Created { reference: inside } = apply(
        &mut engine,
        MediaEdit::CreateMediaInFolder {
            folder,
            media_kind: MediaKind::Picture,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::InstancesCreated { instances } = apply(
        &mut engine,
        MediaEdit::Composition {
            reference: composition,
            edit: crate::media::composition::protocol::CompositionEdit::DropSource {
                source: source.media.0,
                instance: None,
                position: golden_video::plan::Point { x: 1., y: 1. },
                copy: false,
            },
        },
    ) else {
        panic!()
    };
    let nested = MediaId(instances[0].instance.0);
    let (_, parameter, channel) = reference(&mut engine, Some(nested));
    let page = inspect(&engine, query(parameter)).unwrap();
    assert!(page.current.unwrap().present);
    assert!(page.entries.iter().any(|entry| entry.id == folder && entry.folder));
    assert!(!page.entries.iter().any(|entry| entry.id == inside.media.0));
    for excluded in [
        nested.0,
        source.channel.0,
        engine.nodes.get(engine.root).unwrap().node_data().meta.uuid,
    ] {
        assert!(!page.entries.iter().any(|entry| entry.id == excluded));
        let mut invalid = query(parameter);
        invalid.folder = Some(excluded);
        assert!(inspect(&engine, invalid).is_err());
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::SelectTargetPickerChoice {
                    parameter,
                    target: Some(excluded)
                }
            )
            .success
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    let mut child = query(parameter);
    child.folder = Some(folder);
    assert_eq!(
        inspect(&engine, child)
            .unwrap()
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        [inside.media.0]
    );
    let channels = inspect(&engine, query(channel)).unwrap();
    assert_eq!(
        channels.entries.len(),
        1,
        "explicit nested binding keeps its own channel available"
    );
}

#[test]
fn picker_choices_use_atomic_parameter_defaults_reject_foreign_outputs_and_keep_history_reload() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let foreign = create(&mut engine, MediaKind::SolidColor);
    let (media, parameter, channel) = reference(&mut engine, None);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    choose(&mut engine, parameter, Some(first.media.0));
    assert_eq!(engine.undo_len(), history + 1);
    assert_eq!(
        inspect(&engine, query(channel)).unwrap().current.unwrap().id,
        first.channel.0
    );
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    choose(&mut engine, channel, Some(second.channel.0));
    choose(&mut engine, parameter, Some(first.media.0));
    assert_eq!(
        inspect(&engine, query(channel)).unwrap().current.unwrap().id,
        second.channel.0
    );
    for (parameter, target) in [
        (parameter, media.0),
        (channel, foreign.channel.0),
        (channel, first.media.0),
    ] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::SelectTargetPickerChoice {
                    parameter,
                    target: Some(target)
                }
            )
            .success
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    apply(&mut engine, MediaEdit::RemoveChannel { reference: second });
    let missing = inspect(&engine, query(channel)).unwrap().current.unwrap();
    assert_eq!(missing.id, second.channel.0);
    assert!(!missing.present);
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::SelectTargetPickerChoice {
                parameter: channel,
                target: Some(second.channel.0)
            }
        )
        .success
    );
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(
        inspect(&loaded, query(channel)).unwrap().current.unwrap().id,
        second.channel.0
    );
    assert!(engine.undo().unwrap());
    assert!(inspect(&engine, query(channel)).unwrap().current.unwrap().present);
    choose(&mut engine, parameter, None);
    assert!(inspect(&engine, query(channel)).unwrap().current.is_none());
    assert!(inspect(&engine, query(channel)).unwrap().entries.is_empty());
}

#[test]
fn pages_bound_scanning_and_payload_and_query_transport_never_authors_history() {
    let mut engine = fixture();
    let (_, parameter, _) = reference(&mut engine, None);
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let folder = crate::media::MediaFolderNode::new("Many choices".into());
    let id = folder.node_data().meta.uuid;
    let mut tree = NodeTree::new(folder);
    for index in 0..300 {
        tree.push_child(NodeTree::new(crate::media::MediaFolderNode::new(format!(
            "Folder {index}"
        ))));
    }
    engine.edits.push(Edit::AddNodeTree {
        parent: manager,
        prev_sibling: None,
        tree,
    });
    engine.apply_edits().unwrap();
    let mut request = query(parameter);
    request.folder = Some(id);
    request.limit = 32;
    let first = inspect(&engine, request.clone()).unwrap();
    assert_eq!(first.entries.len(), 32);
    request.cursor = first.after;
    let second = inspect(&engine, request.clone()).unwrap();
    assert_eq!(second.entries.len(), 32);
    request.cursor = second.before;
    request.backwards = true;
    assert_eq!(
        inspect(&engine, request.clone())
            .unwrap()
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        first.entries.iter().map(|entry| entry.id).collect::<Vec<_>>()
    );
    request.limit = 65;
    assert!(inspect(&engine, request.clone()).is_err());
    request.limit = 0;
    request.cursor = None;
    request.backwards = false;
    assert!(inspect(&engine, request.clone()).unwrap().entries.is_empty());
    let edit = MediaEdit::InspectTargetPicker { query: request };
    let history = engine.ui_history_state();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let response = AppNode::query_app_operation(
        &engine,
        MEDIA_NAMESPACE,
        MEDIA_OPERATION,
        &serde_json::to_value(&edit).unwrap(),
    )
    .unwrap()
    .unwrap();
    let ack = invoke(&mut engine, edit);
    assert!(ack.success);
    assert_eq!(ack.result.unwrap().value, response);
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn surface_channels_follow_the_media_parameter_even_when_the_binding_is_empty() {
    let mut engine = fixture();
    let [media, channel] = engine
        .nodes
        .iter()
        .find_map(|(_, node)| {
            let AppNode::Surface(surface) = node else {
                return None;
            };
            Some(
                surface
                    .source_parameter_values(None)
                    .map(|(id, _)| engine.nodes.get(id).unwrap().node_data().meta.uuid),
            )
        })
        .unwrap();
    assert!(inspect(&engine, query(channel)).unwrap().entries.is_empty());
    assert_eq!(inspect(&engine, query(channel)).unwrap().empty_label, "None");
    let source = create(&mut engine, MediaKind::SolidColor);
    apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: source.media,
            label: None,
        },
    );
    choose(&mut engine, media, Some(source.media.0));
    assert_eq!(
        inspect(&engine, query(channel)).unwrap().current.unwrap().id,
        source.channel.0
    );
    choose(&mut engine, channel, None);
    let page = inspect(&engine, query(channel)).unwrap();
    assert!(page.current.is_none());
    assert_eq!(
        page.entries.len(),
        2,
        "empty Channel must not erase the selected Media's choices"
    );
    let matrix = create(&mut engine, MediaKind::Matrix);
    choose(&mut engine, media, Some(matrix.media.0));
    let choices = inspect(&engine, query(channel)).unwrap();
    assert_eq!(choices.entries.len(), 1, "only Matrix Final is exposed by default");
    let hidden = crate::matrix::inspect(&engine, matrix.media).unwrap().items[0].channel;
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::SelectTargetPickerChoice {
                parameter: channel,
                target: Some(hidden.channel.0)
            }
        )
        .success
    );
    let mut invalid = query(channel);
    invalid.cursor = Some(source.channel.0);
    assert!(inspect(&engine, invalid).is_err());
}

#[test]
fn long_excluded_runs_yield_a_cursor_after_at_most_256_siblings() {
    let mut engine = fixture();
    let (reference, parameter, _) = reference(&mut engine, None);
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let mut ids = vec![];
    let mut items = (0..300)
        .map(|index| {
            let asset = crate::media::AssetNode::from_locator(format!("Excluded asset {index}"), None);
            ids.push(asset.node_data().meta.uuid);
            golden_core::edit::UserItemTreeInsertion::new(manager, NodeTree::new(asset))
        })
        .collect::<Vec<_>>();
    let last = crate::media::MediaFolderNode::new("Visible after excluded items".into());
    let uuid = last.node_data().meta.uuid;
    items.push(golden_core::edit::UserItemTreeInsertion::new(
        manager,
        NodeTree::new(last),
    ));
    engine.edits.push(Edit::AddUserItemTrees { items });
    engine.apply_edits().unwrap();
    let mut request = query(parameter);
    request.cursor = Some(reference.0);
    let first = inspect(&engine, request.clone()).unwrap();
    assert!(first.entries.is_empty());
    assert_eq!(first.after, Some(ids[255]));
    request.cursor = first.after;
    let next = inspect(&engine, request).unwrap();
    assert_eq!(next.entries.len(), 1);
    assert_eq!(next.entries[0].id, uuid);
    assert!(next.after.is_none());
}

#[test]
fn ten_thousand_catalog_media_keep_first_middle_last_pages_and_current_reads_bounded() {
    let mut engine = fixture();
    let (_, parameter, _) = reference(&mut engine, None);
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let folder = crate::media::MediaFolderNode::new("Large catalog".into());
    let folder_id = folder.node_data().meta.uuid;
    let mut tree = NodeTree::new(folder);
    let mut ids = Vec::with_capacity(10000);
    for index in 0..10000 {
        let node = crate::media::ReferenceNode::from_source(format!("Catalog media {index}"), None);
        ids.push(node.node_data().meta.uuid);
        tree.push_child(NodeTree::new(node).as_user_item());
    }
    engine.edits.push(Edit::AddUserItemTree {
        parent: manager,
        prev_sibling: None,
        tree,
    });
    engine.apply_edits().unwrap();
    let history = engine.ui_history_state();
    let mut request = query(parameter);
    request.folder = Some(folder_id);
    for (cursor, backwards, expected) in [
        (None, false, 0..64),
        (Some(ids[4999]), false, 5000..5064),
        (None, true, 9936..10000),
    ] {
        request.cursor = cursor;
        request.backwards = backwards;
        let page = inspect(&engine, request.clone()).unwrap();
        assert_eq!(
            page.entries.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            ids[expected]
        );
        assert!(serde_json::to_vec(&page).unwrap().len() < 16384);
    }
    request.limit = 0;
    assert!(inspect(&engine, request).unwrap().entries.is_empty());
    assert_eq!(engine.ui_history_state(), history);
}
