use super::*;
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::Node,
};

fn request(editor: ScreenEditorKind, selected: golden_core::node::NodeUuid) -> ScreenEditorQuery {
    ScreenEditorQuery {
        editor,
        selected: Some(selected),
        document: None,
        locked: None,
        retained: None,
        surface: None,
    }
}
fn read(engine: &mut AppEngine, query: ScreenEditorQuery) -> ScreenEditorPage {
    let ScreenEditResult::Editor { page } = apply(engine, ScreenEdit::InspectEditor { query }) else {
        panic!()
    };
    page
}

#[test]
fn editor_targets_follow_descendants_retain_unrelated_selection_and_honor_independent_locks() {
    let mut engine = fixture();
    let (a, surface_a) = create(&mut engine);
    let (b, _) = create(&mut engine);
    let unrelated = super::drop_sources::source(&mut engine).media.0;
    let id = engine.node_id_by_uuid(surface_a.surface.0).unwrap();
    let parameter_folder = engine.nodes.get(id).unwrap().node_data().first_child.unwrap();
    let selected = engine.nodes.get(parameter_folder).unwrap().node_data().meta.uuid;
    let first = read(&mut engine, request(ScreenEditorKind::Screen, selected));
    assert_eq!(first.screen.unwrap().id, a);
    assert_eq!(first.surface.unwrap().reference, surface_a);
    let mut query = ScreenEditorQuery {
        document: Some(first.document),
        retained: first.target,
        ..request(ScreenEditorKind::Screen, unrelated)
    };
    assert_eq!(read(&mut engine, query).screen.unwrap().id, a);
    query.selected = Some(b.0);
    assert_eq!(read(&mut engine, query).screen.unwrap().id, b);
    query.locked = first.target;
    assert_eq!(read(&mut engine, query).screen.unwrap().id, a);
    let source = read(&mut engine, request(ScreenEditorKind::Surface, selected));
    let mut source_query = ScreenEditorQuery {
        document: Some(source.document),
        retained: source.target,
        ..request(ScreenEditorKind::Surface, b.0)
    };
    assert_eq!(read(&mut engine, source_query).surface.unwrap().reference, surface_a);
    source_query.locked = first.target;
    assert!(!invoke(&mut engine, ScreenEdit::InspectEditor { query: source_query }).success);
    query.document = None;
    assert!(!invoke(&mut engine, ScreenEdit::InspectEditor { query }).success);
}

#[test]
fn locked_missing_targets_recover_through_undo_and_load_without_crossing_projects() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    for editor in [ScreenEditorKind::Screen, ScreenEditorKind::Surface] {
        let initial = read(&mut engine, request(editor, reference.surface.0));
        let mut query = ScreenEditorQuery {
            document: Some(initial.document),
            locked: initial.target,
            retained: initial.target,
            surface: Some(reference.surface),
            ..request(editor, reference.surface.0)
        };
        apply(&mut engine, ScreenEdit::RemoveSurface { reference });
        let missing = read(&mut engine, query);
        assert!(missing.surface.is_none());
        assert_eq!(missing.selected_surface, Some(reference.surface));
        assert_eq!(missing.screen.is_some(), editor == ScreenEditorKind::Screen);
        assert!(engine.undo().unwrap());
        assert_eq!(read(&mut engine, query).surface.unwrap().reference, reference);
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&saved).unwrap();
        loaded.apply_edits().unwrap();
        assert_eq!(read(&mut loaded, query).surface.unwrap().reference, reference);
        let mut other = fixture();
        assert!(!read(&mut other, query).document_matches);
        let (new, surface) = create(&mut other);
        query.selected = Some(surface.surface.0);
        assert!(!read(&mut other, query).document_matches);
        query.locked = None;
        assert_eq!(read(&mut other, query).screen.unwrap().id, new);
    }
    apply(&mut engine, ScreenEdit::RemoveScreen { screen });
    assert!(
        read(&mut engine, request(ScreenEditorKind::Screen, screen.0))
            .screen
            .is_none()
    );
}

#[test]
fn large_screen_pages_and_source_inspections_keep_selected_definitions_independent() {
    let mut engine = fixture();
    let (screen, mut selected) = create(&mut engine);
    for _ in 0..130 {
        let ScreenEditResult::SurfaceCreated { reference } =
            apply(&mut engine, ScreenEdit::AddSurface { screen, label: None })
        else {
            panic!()
        };
        selected = reference;
    }
    let history = engine.ui_history_state();
    let page = read(&mut engine, request(ScreenEditorKind::Screen, selected.surface.0));
    assert_eq!(page.screen.unwrap().surfaces.len(), 128);
    assert!(page.has_more_surfaces);
    assert_eq!(page.surface.unwrap().reference, selected);
    let source = read(&mut engine, request(ScreenEditorKind::Surface, selected.surface.0));
    assert!(source.screen.unwrap().surfaces.is_empty());
    assert_eq!(source.surface.unwrap().reference, selected);
    assert_eq!(engine.ui_history_state(), history);
    let media = super::drop_sources::source(&mut engine);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let ScreenEditResult::SurfaceChanged { snapshot } = apply(
        &mut engine,
        ScreenEdit::DropSourceAt {
            screen,
            point: Point { x: 0.5, y: 0.5 },
            source: media.media.0,
        },
    ) else {
        panic!()
    };
    assert_eq!(
        snapshot.reference, selected,
        "drop resolves beyond the bounded overlay page"
    );
    assert_eq!(snapshot.definition.source, Some(media));
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(
        !invoke(
            &mut engine,
            ScreenEdit::DropSourceAt {
                screen,
                point: Point { x: f32::NAN, y: 0.5 },
                source: media.media.0
            }
        )
        .success
    );
}
