use crate::{app::AppNode, screens::protocol::*};
use golden_core::app::{
    ProjectLifecycle, create_new_project_engine, from_sparse_project_json, prepare_engine_for_runtime,
    to_sparse_project_json_pretty,
};

fn query(editor: ScreenEditorKind) -> ScreenEditorQuery {
    ScreenEditorQuery {
        editor,
        selected: None,
        document: None,
        locked: None,
        retained: None,
        surface: None,
    }
}

fn edit(engine: &mut crate::app::AppEngine, edit: ScreenEdit) -> ScreenEditResult {
    let ack = golden_core::application::apply_project_intent(
        engine,
        golden_core::ui_sync::UiEditIntent::InvokeAppOperation {
            namespace: SCREEN_NAMESPACE.into(),
            operation: SCREEN_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}

fn read(engine: &crate::app::AppEngine, query: ScreenEditorQuery) -> ScreenEditorPage {
    let result = AppNode::query_app_operation(
        engine,
        SCREEN_NAMESPACE,
        SCREEN_OPERATION,
        &serde_json::to_value(ScreenEdit::InspectEditor { query }).unwrap(),
    )
    .unwrap()
    .unwrap();
    let ScreenEditResult::Editor { page } = serde_json::from_value(result).unwrap() else {
        panic!("expected editor page")
    };
    *page
}

#[test]
fn fresh_and_repeated_new_resolve_actual_targets_without_selection_or_history() {
    let mut previous = [None, None];
    for _ in 0..3 {
        let mut engine = create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        prepare_engine_for_runtime(&mut engine).unwrap();
        let snapshot = crate::screens::inspect(&engine).unwrap();
        let initial = &snapshot.screens[0];
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        for (index, kind) in [ScreenEditorKind::Screen, ScreenEditorKind::Surface]
            .into_iter()
            .enumerate()
        {
            let page = read(&engine, query(kind));
            assert!(page.document_matches);
            assert_eq!(page.screen.unwrap().id, initial.id);
            let expected = match kind {
                ScreenEditorKind::Screen => ScreenEditorTarget::Screen { screen: initial.id },
                ScreenEditorKind::Surface => ScreenEditorTarget::Surface {
                    reference: initial.surfaces[0].reference,
                },
            };
            assert_eq!(page.target, Some(expected));
            if kind == ScreenEditorKind::Surface {
                assert_eq!(page.surface.unwrap().reference, initial.surfaces[0].reference);
            }
            if let Some((old_document, old_target)) = previous[index] {
                assert_ne!(Some(old_target), page.target);
                let stale = ScreenEditorQuery {
                    document: Some(old_document),
                    locked: Some(old_target),
                    retained: Some(old_target),
                    ..query(kind)
                };
                assert_eq!(read(&engine, stale).target, Some(expected));
            }
            previous[index] = Some((page.document, expected));
        }
        assert_eq!(engine.undo_len(), 0);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}

#[test]
fn loaded_projects_have_no_inferred_default_but_keep_saved_panel_targets() {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    for kind in [ScreenEditorKind::Screen, ScreenEditorKind::Surface] {
        let original = read(&engine, query(kind));
        assert!(read(&loaded, query(kind)).target.is_none());
        let retained = ScreenEditorQuery {
            document: Some(original.document),
            retained: original.target,
            ..query(kind)
        };
        assert_eq!(read(&loaded, retained).target, original.target);
        let locked = ScreenEditorQuery {
            locked: original.target,
            ..retained
        };
        assert_eq!(read(&loaded, locked).target, original.target);
        let foreign = ScreenEditorQuery {
            document: Some(golden_core::node::NodeUuid(uuid::Uuid::new_v4())),
            ..locked
        };
        assert!(!read(&loaded, foreign).document_matches);
    }
}

#[test]
fn explicit_follow_and_lock_precede_defaults_and_deleted_seeds_do_not_choose_other_objects() {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let ScreenEditResult::ScreenCreated { screen, surface } =
        edit(&mut engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    for kind in [ScreenEditorKind::Screen, ScreenEditorKind::Surface] {
        let initial = read(&engine, query(kind));
        let other = read(
            &engine,
            ScreenEditorQuery {
                selected: Some(surface.surface.0),
                ..query(kind)
            },
        );
        assert_eq!(other.screen.unwrap().id, screen);
        let retained = ScreenEditorQuery {
            document: Some(initial.document),
            retained: other.target,
            ..query(kind)
        };
        assert_eq!(read(&engine, retained).target, other.target);
        let locked = ScreenEditorQuery {
            selected: Some(surface.surface.0),
            locked: initial.target,
            ..retained
        };
        assert_eq!(read(&engine, locked).target, initial.target);
    }
    let initial = read(&engine, query(ScreenEditorKind::Screen)).screen.unwrap().id;
    edit(&mut engine, ScreenEdit::RemoveScreen { screen: initial });
    for kind in [ScreenEditorKind::Screen, ScreenEditorKind::Surface] {
        assert!(read(&engine, query(kind)).target.is_none());
    }
    assert!(engine.undo().unwrap());
    assert_eq!(
        read(&engine, query(ScreenEditorKind::Screen)).screen.unwrap().id,
        initial
    );
    edit(&mut engine, ScreenEdit::RemoveScreen { screen: initial });
    edit(&mut engine, ScreenEdit::RemoveScreen { screen });
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut empty = from_sparse_project_json::<AppNode>(&saved).unwrap();
    empty.apply_edits().unwrap();
    for kind in [ScreenEditorKind::Screen, ScreenEditorKind::Surface] {
        assert!(read(&empty, query(kind)).target.is_none());
    }
    assert!(crate::screens::inspect(&empty).unwrap().screens.is_empty());
}
