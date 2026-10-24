use super::*;

/// Applies a project-aware intent synchronously, including app-owned operations.
///
/// Headless/native callers receive the same authoritative result and rejection as
/// the production application facade. The caller owns exclusive engine access.
pub fn apply_project_intent<T: ProjectLifecycle>(engine: &mut Engine<T>, intent: UiEditIntent) -> UiAck {
    apply_ui_intent_to_engine(engine, intent, None)
}

pub(super) fn apply<T: ProjectLifecycle>(
    engine: &mut Engine<T>,
    namespace: String,
    operation: String,
    payload: serde_json::Value,
    client: Option<&str>,
    before: Option<EngineTime>,
) -> UiAck {
    if [&namespace, &operation]
        .iter()
        .any(|name| name.trim().is_empty() || name.len() > 128)
    {
        return rejected_ack(
            engine,
            "invalid_app_operation",
            "app namespace and operation require 1..128 bytes".into(),
        );
    }
    let label = format!("{namespace}: {operation}");
    let outcome = engine.apply_implicit_ui_edit_session(&label, "__app-operation", client, |engine| {
        let reject = |engine: &Engine<T>, message| crate::engine::EngineEditError::NodeMutationRejected {
            edit_index: 0,
            operation: "InvokeAppOperation",
            node: engine.root,
            node_type: "application".into(),
            message,
        };
        let result = T::apply_app_operation(engine, &namespace, &operation, payload)
            .map_err(|message| reject(engine, message))?;
        engine.settle_edit_notifications(16)?;
        T::settle_pending_operations(engine).map_err(|message| reject(engine, message))?;
        Ok(result)
    });
    match outcome {
        Ok(result) => {
            let mut ack = applied_ack_since(engine, before);
            ack.result = Some(crate::ui_sync::UiAppOperationResult { value: result });
            ack
        }
        Err(error) => rejected_ack(engine, "app_operation_failed", error.to_string()),
    }
}
