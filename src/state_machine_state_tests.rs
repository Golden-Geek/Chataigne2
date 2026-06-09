use super::StateMachineState;

#[test]
fn state_defaults_are_ready_for_canvas_authoring() {
    let state = StateMachineState::new();

    assert!(state.active.get());
    assert_eq!(state.description.get_ref(), "");
    assert_eq!(state.x.get(), 0.0);
    assert_eq!(state.y.get(), 0.0);
    assert_eq!(state.width.get(), 13.0);
    assert_eq!(state.height.get(), 8.0);
}
