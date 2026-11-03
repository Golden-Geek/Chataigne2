use super::*;
use crate::app::AppNode;
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::{Node, NodeUuid},
};

fn inspect(engine: &mut AppEngine, query: MediaPreviewQuery) -> MediaPreviewPage {
    let MediaEditResult::MediaPreview { page } = apply(engine, MediaEdit::InspectMediaPreview { query }) else {
        panic!()
    };
    page
}
#[test]
fn selection_resolves_descendants_defaults_and_off_page_channels_without_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: source.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let id = engine.node_id_by_uuid(second.channel.0).unwrap();
    let folder = engine.nodes.get(id).unwrap().node_data().first_child.unwrap();
    let selected = engine.nodes.get(folder).unwrap().node_data().meta.uuid;
    let history = engine.ui_history_state();
    let mut query = MediaPreviewQuery {
        selected: Some(selected),
        document: None,
        channel: None,
        start: 0,
        count: 1,
    };
    let page = inspect(&mut engine, query);
    assert_eq!(page.media, Some(source.media));
    assert_eq!(page.selected.unwrap().reference, second);
    assert_eq!(page.channels.unwrap().channels[0].reference, source);
    query.channel = Some(source.channel);
    assert_eq!(inspect(&mut engine, query).selected.unwrap().reference, source);
    query.channel = Some(ChannelId(NodeUuid(uuid::Uuid::new_v4())));
    assert_eq!(inspect(&mut engine, query).selected.unwrap().reference, second);
    assert_eq!(engine.ui_history_state(), history);
}
#[test]
fn locks_are_document_scoped_and_references_recover_through_history_and_reload() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::ReferenceCreated { media } = apply(
        &mut engine,
        MediaEdit::CreateReference {
            target: Some(source.media),
            label: None,
        },
    ) else {
        panic!()
    };
    let mut query = MediaPreviewQuery {
        selected: Some(media.0),
        document: None,
        channel: Some(source.channel),
        start: 0,
        count: 4,
    };
    let first = inspect(&mut engine, query);
    query.document = Some(first.document);
    assert_eq!(first.selected.unwrap().reference.media, media);
    apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
    assert!(inspect(&mut engine, query).selected.is_none());
    assert!(engine.undo().unwrap());
    assert!(inspect(&mut engine, query).selected.is_some());
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert!(inspect(&mut loaded, query).document_matches);
    let mut other = fixture();
    let mismatch = inspect(&mut other, query);
    assert!(!mismatch.document_matches);
    assert!(mismatch.media.is_none() && mismatch.selected.is_none());
}
