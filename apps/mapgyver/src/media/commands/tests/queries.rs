use super::*;
use golden_core::app::ProjectLifecycle;

#[test]
fn typed_inspect_command_uses_the_immutable_query_path_with_identical_page_results() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let query = command(&mut engine, MediaCommandKind::InspectChannels);
    set(&mut engine, query, "media", reference(source.media.0));
    let history = engine.ui_history_state();
    let payload = serde_json::to_value(MediaEdit::ExecuteCommand { command: query }).unwrap();
    let expected = AppNode::query_app_operation(&engine, MEDIA_NAMESPACE, MEDIA_OPERATION, &payload)
        .unwrap()
        .unwrap();
    let actual = apply(&mut engine, MediaEdit::ExecuteCommand { command: query });
    assert_eq!(serde_json::to_value(actual).unwrap(), expected);
    assert_eq!(engine.ui_history_state(), history);
    let mutation = command(&mut engine, MediaCommandKind::CreateMedia);
    let payload = serde_json::to_value(MediaEdit::ExecuteCommand { command: mutation }).unwrap();
    assert!(AppNode::query_app_operation(&engine, MEDIA_NAMESPACE, MEDIA_OPERATION, &payload).is_none());
}
