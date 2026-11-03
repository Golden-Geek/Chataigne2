use super::*;
use crate::app::AppNode;
use golden_core::app::{ProjectLifecycle, to_sparse_project_json_pretty};
#[test]
fn inspections_preserve_active_gestures_redo_and_events_for_every_public_caller() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    apply(
        &mut engine,
        ScreenEdit::RenameScreen {
            screen,
            label: "Undone".into(),
        },
    );
    assert!(engine.undo().unwrap());
    for active in [false, true] {
        if active {
            super::gestures::session(&mut engine, true);
        }
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.ui_history_state();
        let cursor = engine.ui_event_log().last().map(|event| event.time);
        for edit in [
            ScreenEdit::InspectManager,
            ScreenEdit::InspectScreen { screen },
            ScreenEdit::InspectSurface { reference },
            ScreenEdit::InspectRow { node: screen.0 },
            ScreenEdit::InspectRow {
                node: reference.surface.0,
            },
            ScreenEdit::InspectRow {
                node: golden_core::model::NodeUuid(uuid::Uuid::new_v4()),
            },
        ] {
            let payload = serde_json::to_value(&edit).unwrap();
            let expected = AppNode::query_app_operation(&engine, SCREEN_NAMESPACE, SCREEN_OPERATION, &payload).unwrap();
            let reply = invoke(&mut engine, edit);
            match expected {
                Ok(value) => {
                    assert!(reply.success);
                    assert_eq!(reply.result.unwrap().value, value);
                }
                Err(_) => assert!(!reply.success),
            }
            assert_eq!(reply.history, history);
            assert_eq!((reply.earliest_event_time, reply.latest_event_time), (None, None));
            assert_eq!(engine.ui_event_log().last().map(|event| event.time), cursor);
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        }
        if active {
            super::gestures::session(&mut engine, false);
        }
    }
    assert!(engine.redo().unwrap());
    assert_eq!(super::super::inspect(&engine).unwrap().screens[0].label, "Undone");
    let payload = serde_json::to_value(ScreenEdit::RemoveScreen { screen }).unwrap();
    assert!(AppNode::query_app_operation(&engine, SCREEN_NAMESPACE, SCREEN_OPERATION, &payload).is_none());
}
