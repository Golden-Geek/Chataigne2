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
        retained: None,
        instance: None,
        start: 0,
        count: 4,
    }
}

#[test]
fn explicit_owner_selection_clears_retained_children_without_mutating_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let children = add(&mut engine, owner, vec![source; 6]);
    let document = read(&mut engine, query(owner.media.0)).document;
    let history = engine.ui_history_state();
    for selected in [owner.media.0, owner.channel.0] {
        let page = read(
            &mut engine,
            CompositionEditorQuery {
                document: Some(document),
                locked: Some(owner),
                retained: Some(owner),
                instance: Some(children[5].instance),
                ..query(selected)
            },
        );
        assert_eq!(page.total, 6);
        assert!(page.selected.is_none());
        assert_eq!(
            page.parameters.len(),
            4,
            "no stale off-page child bindings after deselection"
        );
    }
    let mut request = CompositionEditorQuery {
        document: Some(document),
        locked: Some(owner),
        instance: Some(children[5].instance),
        ..query(source.media.0)
    };
    assert_eq!(read(&mut engine, request).selected.unwrap().reference, children[5]);
    request.selected = Some(children[0].instance.0);
    assert_eq!(read(&mut engine, request).selected.unwrap().reference, children[0]);
    request.selected = None;
    request.instance = None;
    assert!(
        read(&mut engine, request).selected.is_none(),
        "no implicit first-child selection"
    );
    assert_eq!(engine.ui_history_state(), history);
}

#[test]
fn nested_composition_owner_clears_its_child_even_when_it_is_an_outer_instance() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let inner = create(&mut engine, MediaKind::Composition);
    add(&mut engine, inner, vec![source]);
    let outer = create(&mut engine, MediaKind::Composition);
    let MediaEditResult::InstancesCreated { .. } = edit(
        &mut engine,
        outer,
        CompositionEdit::DropSource {
            source: inner.media.0,
            instance: None,
            copy: true,
            position: Point { x: 0.0, y: 0.0 },
        },
    ) else {
        panic!()
    };
    let reference = inspect(&mut engine, outer).instances[0].definition.source.unwrap();
    let document = read(&mut engine, query(outer.media.0)).document;
    let page = read(
        &mut engine,
        CompositionEditorQuery {
            document: Some(document),
            locked: Some(reference),
            ..query(source.media.0)
        },
    );
    let retained = page.composition.unwrap().instances[0].reference.instance;
    let page = read(
        &mut engine,
        CompositionEditorQuery {
            document: Some(document),
            locked: Some(reference),
            instance: Some(retained),
            ..query(reference.media.0)
        },
    );
    assert_eq!(page.channel.unwrap().reference, reference);
    assert_eq!(page.total, 1);
    assert!(page.selected.is_none());
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
    assert_eq!(page.at, engine.time);
    assert_eq!(page.parameters.len(), 5);
    for binding in &page.parameters {
        let child = engine.node_id_by_uuid(binding.instance.0).unwrap();
        let parameter = super::parameters::instance_parameter(&engine, child, "position");
        assert_eq!(
            binding.position,
            engine.nodes.get(parameter).unwrap().node_data().meta.uuid
        );
    }
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
fn retained_content_follows_relevant_selection_and_never_crosses_documents() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let other = create(&mut engine, MediaKind::Composition);
    let first = read(&mut engine, query(owner.media.0));
    let mut request = CompositionEditorQuery {
        document: Some(first.document),
        retained: Some(owner),
        ..query(source.media.0)
    };
    assert_eq!(read(&mut engine, request).channel.unwrap().reference, owner);
    request.selected = Some(other.channel.0);
    assert_eq!(read(&mut engine, request).channel.unwrap().reference, other);
    request.locked = Some(owner);
    assert_eq!(read(&mut engine, request).channel.unwrap().reference, owner);
    request.locked = None;
    request.selected = Some(source.media.0);
    apply(&mut engine, MediaEdit::RemoveMedia { media: owner.media });
    assert!(read(&mut engine, request).channel.is_none());
    assert!(engine.undo().unwrap());
    assert_eq!(read(&mut engine, request).channel.unwrap().reference, owner);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(read(&mut loaded, request).channel.unwrap().reference, owner);
    let mut unrelated = fixture();
    assert!(!read(&mut unrelated, request).document_matches);
    let fresh = create(&mut unrelated, MediaKind::Composition);
    request.selected = Some(fresh.media.0);
    let page = read(&mut unrelated, request);
    assert!(page.document_matches);
    assert_eq!(page.channel.unwrap().reference, fresh);
    request.locked = Some(owner);
    assert!(!read(&mut unrelated, request).document_matches);
    request.document = None;
    assert!(!invoke(&mut engine, MediaEdit::InspectCompositionEditor { query: request }).success);
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
