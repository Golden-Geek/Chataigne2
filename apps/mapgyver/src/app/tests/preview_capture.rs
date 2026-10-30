use super::*;
use golden_core::app::prepare_engine_for_runtime;
use golden_core::application::ProductionRuntime;
use golden_core::ui_sync::UiProjectFileSpec;

#[test]
fn authoritative_capture_returns_owned_app_metadata_without_changing_history() {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    prepare_engine_for_runtime(&mut engine).unwrap();
    let runtime = ProductionRuntime::new(engine, UiProjectFileSpec::default()).unwrap();
    let before = runtime.history_state().unwrap();
    let capture = runtime
        .capture_app(|engine| crate::media::capture_channels(engine, &[], 8))
        .unwrap()
        .unwrap();
    assert!(capture.is_current());
    let after = runtime.history_state().unwrap();
    assert_eq!(before.current_history_state_id, after.current_history_state_id);
    assert_eq!(before.can_undo, after.can_undo);
}
