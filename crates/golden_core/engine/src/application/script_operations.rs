use super::*;
use crate::{
    edit::EditCompletionStatus,
    events::{CustomEvent, CustomEventRetention, Event, EventFrame},
    process_ctx::ExecutionPhase,
};
use golden_script::ScriptAppOperationRequest;

/// Services one bounded turn of script requests through the same atomic app operation route.
///
/// The production runtime calls this after ordinary edits/ticks. Exclusive headless engine
/// consumers may call it after applying callback edits. Requests emitted by result callbacks
/// wait for the next turn, and discarded/reloaded script runtimes cannot mutate the project.
pub fn service_script_app_operations<T: ProjectLifecycle>(
    engine: &mut Engine<T>,
) -> Result<(), crate::engine::EngineEditError> {
    if engine.has_atomic_journal() {
        return Ok(());
    }
    const MAXIMUM_PER_TURN: usize = 8;
    let count = engine.pending_script_app_operations.len().min(MAXIMUM_PER_TURN);
    for _ in 0..count {
        let pending = engine
            .pending_script_app_operations
            .pop_front()
            .expect("fixed request turn");
        if pending.owner.upgrade().is_none() || !engine.nodes.contains(pending.source) {
            continue;
        }
        let Some(fence) = pending.fence.get() else {
            engine.pending_script_app_operations.push_back(pending);
            continue;
        };
        let result = match fence.status() {
            EditCompletionStatus::Pending => {
                engine.pending_script_app_operations.push_back(pending);
                continue;
            }
            EditCompletionStatus::Failed => Err("The originating script callback edits did not commit".into()),
            EditCompletionStatus::Applied => app_operations::apply_atomic(
                engine,
                pending.request.namespace(),
                pending.request.operation(),
                pending.request.payload().clone(),
                Some("__script-app-operation"),
                ScriptAppOperationRequest::admit_result,
            )
            .map_err(|error| error.to_string()),
        };
        let payload = pending.request.into_callback(result);
        // A successful operation may delete or reload its requesting script. Seal the
        // response first so an abandoned result still has background disposal ownership.
        if pending.owner.upgrade().is_none() || !engine.nodes.contains(pending.source) {
            continue;
        }
        let event = CustomEvent::from_shared_payload(
            crate::script::app_operations::APP_OPERATION_RESULT_TOPIC,
            Some(pending.source),
            payload,
            CustomEventRetention::Transient,
        );
        let frame = EventFrame::from_shared(vec![std::sync::Arc::new(Event::custom(engine.time, event))]);
        engine.dispatch_precomputed_inbox(ExecutionPhase::EngineTick, vec![(pending.source, frame)])?;
        engine.settle_runtime_edit_notifications(16)?;
    }
    Ok(())
}
