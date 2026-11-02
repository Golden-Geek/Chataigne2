use super::*;
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};

fn output(engine: &AppEngine, screen: ScreenId) -> ScreenEdit {
    let snapshot = crate::screens::inspect(engine).unwrap();
    ScreenEdit::DisableOutput {
        project: crate::media::authoring::project_instance(engine).unwrap(),
        screen,
        target: snapshot
            .screens
            .iter()
            .find(|item| item.id == screen)
            .unwrap()
            .definition
            .target
            .clone()
            .unwrap(),
    }
}

#[test]
fn output_escape_disables_only_its_screen_through_metadata_with_exact_history() {
    let mut engine = fixture();
    let (screen, _) = create(&mut engine);
    let (peer, _) = create(&mut engine);
    let before = crate::screens::inspect(&engine).unwrap();
    let captured = crate::screens::rendering::capture_outputs(&engine, 128, 7, &[]).unwrap();
    let history = engine.undo_len();
    let request = output(&engine, screen);
    assert!(invoke(&mut engine, request.clone()).success);
    let after = crate::screens::inspect(&engine).unwrap();
    assert!(
        !after
            .screens
            .iter()
            .find(|item| item.id == screen)
            .unwrap()
            .definition
            .enabled
    );
    assert!(
        after
            .screens
            .iter()
            .find(|item| item.id == peer)
            .unwrap()
            .definition
            .enabled
    );
    assert_eq!(after.screens[0].surfaces, before.screens[0].surfaces);
    assert_eq!(after.screens[0].definition.target, before.screens[0].definition.target);
    assert!(
        captured.guards.iter().any(|guard| !guard.is_current()),
        "disable must wake output recapture"
    );
    let outputs = crate::screens::rendering::capture_outputs(&engine, 128, 7, &[])
        .unwrap()
        .outputs;
    assert_eq!(
        outputs.iter().map(|output| output.screen).collect::<Vec<_>>(),
        vec![peer]
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(invoke(&mut engine, request).success);
    assert_eq!(engine.undo_len(), history + 1, "repeat disable is a no-op");
    assert!(engine.undo().unwrap());
    assert_eq!(crate::screens::inspect(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(crate::screens::inspect(&engine).unwrap(), after);
}

#[test]
fn retired_project_and_retargeted_output_requests_cannot_disable_a_current_screen() {
    let mut engine = fixture();
    let (screen, _) = create(&mut engine);
    let request = output(&engine, screen);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    let history = loaded.undo_len();
    assert!(!invoke(&mut loaded, request.clone()).success);
    assert_eq!(loaded.undo_len(), history);
    let before = crate::screens::inspect(&engine).unwrap();
    let mut definition = before.screens[0].definition.clone();
    definition.target = Some(ScreenTarget::Rectangle {
        bounds: PhysicalRectangle {
            left: 10,
            top: 20,
            extent: golden_video::Extent2D::new(320, 180).unwrap(),
        },
    });
    apply(&mut engine, ScreenEdit::ConfigureScreen { screen, definition });
    let current = crate::screens::inspect(&engine).unwrap();
    let history = engine.undo_len();
    assert!(!invoke(&mut engine, request).success);
    assert_eq!(crate::screens::inspect(&engine).unwrap(), current);
    assert_eq!(engine.undo_len(), history);
}
