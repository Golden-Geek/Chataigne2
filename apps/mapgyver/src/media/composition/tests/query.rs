use super::*;
use golden_core::{
    application::apply_project_intent,
    node::{Node, NodeUuid},
    ui_sync::UiEditIntent,
};

fn read(engine: &mut AppEngine, query: CompositionEditorQuery) -> CompositionEditorPage {
    let MediaEditResult::CompositionEditor { page } = apply(engine, MediaEdit::InspectCompositionEditor { query })
    else {
        panic!()
    };
    page
}
fn query(selected: NodeUuid) -> CompositionEditorQuery {
    CompositionEditorQuery {
        selected: Some(selected),
        document: None,
        locked: None,
        instance: None,
        start: 0,
        count: 4,
    }
}

#[test]
fn bounded_pages_resolve_owned_descendants_and_keep_off_page_selection_independent() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let children = add(&mut engine, owner, vec![source; 300]);
    let id = engine.node_id_by_uuid(children[299].instance.0).unwrap();
    let placement = engine.nodes.get(id).unwrap().node_data().first_child.unwrap();
    let selected = engine.nodes.get(placement).unwrap().node_data().meta.uuid;
    let history = engine.ui_history_state();
    let mut request = query(selected);
    let page = read(&mut engine, request);
    assert_eq!(page.channel.unwrap().reference, owner);
    assert_eq!(page.total, 300);
    let composition = page.composition.unwrap();
    assert_eq!(composition.instances.len(), 4);
    assert_eq!(composition.instances[0].reference, children[0]);
    assert_eq!(page.selected.unwrap().reference, children[299]);
    request.start = 298;
    let page = read(&mut engine, request);
    assert_eq!(page.composition.unwrap().instances.len(), 2);
    assert_eq!(page.selected.unwrap().reference, children[299]);
    request.start = u32::MAX;
    assert!(read(&mut engine, request).composition.unwrap().instances.is_empty());
    assert_eq!(engine.ui_history_state(), history);
}

#[test]
fn locks_are_project_scoped_and_follow_selection_never_uses_an_unrelated_composition() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let other = create(&mut engine, MediaKind::Composition);
    let children = add(&mut engine, owner, vec![source; 2]);
    let first = read(&mut engine, query(owner.media.0));
    let mut locked = CompositionEditorQuery {
        document: Some(first.document),
        locked: Some(owner),
        instance: Some(children[1].instance),
        ..query(other.media.0)
    };
    assert_eq!(read(&mut engine, locked).channel.unwrap().reference, owner);
    assert_eq!(read(&mut engine, locked).selected.unwrap().reference, children[1]);
    assert_eq!(
        read(&mut engine, query(other.channel.0)).channel.unwrap().reference,
        other
    );
    assert!(read(&mut engine, query(source.media.0)).composition.is_none());
    apply(&mut engine, MediaEdit::RemoveMedia { media: owner.media });
    assert!(read(&mut engine, locked).composition.is_none());
    assert!(engine.undo().unwrap());
    assert_eq!(read(&mut engine, locked).selected.unwrap().reference, children[1]);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert!(read(&mut loaded, locked).document_matches);
    let mut unrelated = fixture();
    let mismatch = read(&mut unrelated, locked);
    assert!(!mismatch.document_matches);
    assert!(mismatch.composition.is_none() && mismatch.selected.is_none());
    locked.document = None;
    assert!(!invoke(&mut engine, MediaEdit::InspectCompositionEditor { query: locked }).success);
}

#[test]
fn immutable_editor_reads_preserve_held_edits_redo_and_rejected_query_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    add(&mut engine, owner, vec![source]);
    assert!(engine.undo().unwrap());
    let begin = apply_project_intent(
        &mut engine,
        UiEditIntent::BeginEdit {
            client_edit_id: "read-fence".into(),
            label: None,
        },
    );
    assert!(begin.success);
    let history = engine.ui_history_state();
    let page = read(&mut engine, query(owner.channel.0));
    assert_eq!(page.total, 0);
    for count in [0, 129, u16::MAX] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::InspectCompositionEditor {
                    query: CompositionEditorQuery {
                        count,
                        ..query(owner.media.0)
                    }
                }
            )
            .success
        );
    }
    assert_eq!(engine.ui_history_state(), history);
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::EndEdit {
                client_edit_id: "read-fence".into()
            }
        )
        .success
    );
    assert!(engine.redo().unwrap());
    assert_eq!(read(&mut engine, query(owner.channel.0)).total, 1);
}
