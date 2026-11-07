use super::*;
use crate::screens::protocol::{SCREEN_NAMESPACE, SCREEN_OPERATION, ScreenEdit, ScreenTarget, SurfaceDefinition};
use golden_core::{
    app::{ProjectLifecycle, prepare_engine_for_runtime},
    application::apply_project_intent,
    ui_sync::UiEditIntent,
};

fn fresh() -> super::super::AppEngine {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    prepare_engine_for_runtime(&mut engine).unwrap();
    engine
}

fn edit(engine: &mut super::super::AppEngine, operation: ScreenEdit) {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: SCREEN_NAMESPACE.into(),
            operation: SCREEN_OPERATION.into(),
            payload: serde_json::to_value(operation).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
}

#[test]
fn fresh_project_has_one_complete_typed_screen_without_authored_history() {
    for _ in 0..3 {
        let mut engine = fresh();
        let snapshot = crate::screens::inspect(&engine).unwrap();
        assert_eq!(snapshot.screens.len(), 1);
        let screen = &snapshot.screens[0];
        assert_eq!(screen.label, "Screen");
        assert!(screen.definition.enabled);
        assert_eq!(screen.surfaces.len(), 1);
        assert_eq!(screen.surfaces[0].reference.screen, screen.id);
        assert_eq!(screen.surfaces[0].definition, SurfaceDefinition::default());
        assert!(
            matches!(&screen.definition.target, Some(ScreenTarget::Spout {sender, extent})
            if sender == "MapGyver Screen" && extent.width() == 1920 && extent.height() == 1080)
        );
        assert_eq!(engine.undo_len(), 0);
        assert!(!engine.undo().unwrap());
        let row = crate::screens::inspect(&engine).unwrap();
        assert_eq!(row, snapshot);
        let json = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(json.contains("mapgyver.screen"));
        assert!(json.contains("mapgyver.surface"));
    }
}

#[test]
fn loaded_zero_one_and_multiple_screen_projects_keep_their_exact_authored_contents() {
    for count in 0..=3 {
        let mut engine = fresh();
        let initial = crate::screens::inspect(&engine).unwrap().screens[0].id;
        if count == 0 {
            edit(&mut engine, ScreenEdit::RemoveScreen { screen: initial });
        }
        for index in 1..count {
            edit(
                &mut engine,
                ScreenEdit::CreateScreen {
                    label: Some(format!("Saved {index}")),
                },
            );
        }
        let expected = crate::screens::inspect(&engine).unwrap();
        assert_eq!(expected.screens.len(), count);
        let json = to_sparse_project_json_pretty(&engine).unwrap();
        for _ in 0..2 {
            let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
            AppNode::prepare_project_candidate(&mut loaded).unwrap();
            prepare_engine_for_runtime(&mut loaded).unwrap();
            assert_eq!(crate::screens::inspect(&loaded).unwrap(), expected);
            assert_eq!(loaded.undo_len(), 0);
        }
    }
}

#[test]
fn initial_screen_reserves_its_label_and_sender_for_ordinary_creation_and_history() {
    let mut engine = fresh();
    let initial = crate::screens::inspect(&engine).unwrap();
    edit(&mut engine, ScreenEdit::CreateScreen { label: None });
    let snapshot = crate::screens::inspect(&engine).unwrap();
    assert_eq!(snapshot.screens.len(), 2);
    assert_eq!(snapshot.screens[1].label, "Screen 2");
    assert!(matches!(&snapshot.screens[1].definition.target,
        Some(ScreenTarget::Spout {sender, ..}) if sender == "MapGyver Screen 2"));
    assert_eq!(engine.undo_len(), 1);
    assert!(engine.undo().unwrap());
    assert_eq!(crate::screens::inspect(&engine).unwrap(), initial);
    assert!(engine.redo().unwrap());
    assert_eq!(crate::screens::inspect(&engine).unwrap(), snapshot);
}

#[test]
fn initial_screen_and_surface_parameters_are_bound_and_editable_after_runtime_preparation() {
    let mut engine = fresh();
    let snapshot = crate::screens::inspect(&engine).unwrap();
    let screen = &snapshot.screens[0];
    let surface = &screen.surfaces[0];
    let mut definition = surface.definition.clone();
    definition.opacity = 0.5;
    edit(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: surface.reference,
            definition: definition.clone(),
        },
    );
    let mut output = screen.definition.clone();
    output.enabled = false;
    edit(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen: screen.id,
            definition: output.clone(),
        },
    );
    let configured = crate::screens::inspect(&engine).unwrap();
    assert_eq!(configured.screens[0].definition, output);
    assert_eq!(configured.screens[0].surfaces[0].definition, definition);
    assert_eq!(engine.undo_len(), 2);
    assert!(engine.undo().unwrap());
    assert!(engine.undo().unwrap());
    assert_eq!(crate::screens::inspect(&engine).unwrap(), snapshot);
}
