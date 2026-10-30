//! Commands and manager methods share the bounded atomic app-operation service.

use super::protocol::{SCREEN_NAMESPACE, SCREEN_OPERATION, ScreenEdit};
use golden_core::{
    events::CustomEvent,
    node::{NodeId, NodeScriptDescriptor},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
    script::{APP_OPERATION_RESULT_TOPIC, ScriptAppOperationRequest, ScriptNodeConfig, ScriptSource},
};
use std::sync::Arc;

pub const MAX_REQUEST_JSON_BYTES: usize = 64 * 1024;

pub(super) fn template() -> ScriptNodeConfig {
    ScriptNodeConfig {
        source: ScriptSource::Inline {
            text: [
                include_str!("script_templates/snippets/functions.js"),
                include_str!("script_templates/snippets/callbacks.js"),
            ]
            .join("\n"),
        },
    }
}

pub(super) fn decode(request: &str) -> Result<ScreenEdit, String> {
    if request.len() > MAX_REQUEST_JSON_BYTES {
        return Err("screen request exceeds 64 KiB of JSON".into());
    }
    serde_json::from_str(request).map_err(|error| format!("invalid ScreenEdit: {error}"))
}

pub(super) fn enqueue(
    ctx: &mut ProcessCtx,
    source: NodeId,
    owner: &Arc<()>,
    request_id: &str,
    edit: ScreenEdit,
) -> Result<(), String> {
    let reservation = ScriptAppOperationRequest::reserve(request_id, SCREEN_NAMESPACE, SCREEN_OPERATION)?;
    let payload = serde_json::to_value(edit).map_err(|error| error.to_string())?;
    let request = reservation.finish(payload).map_err(|error| error.to_string())?;
    ctx.enqueue_app_operation(source, Arc::downgrade(owner), request);
    Ok(())
}

pub(super) fn manager_descriptor() -> NodeScriptDescriptor {
    let mut descriptor = NodeScriptDescriptor::default();
    descriptor.add_methods(["editScreens", "inspectScreens"]);
    descriptor
}

pub(super) fn manager_method(
    ctx: &mut ProcessCtx,
    source: NodeId,
    owner: &Arc<()>,
    method: &str,
    args: &[ParamValue],
) -> Result<bool, String> {
    let count = match method {
        "editScreens" => 2,
        "inspectScreens" => 1,
        _ => return Ok(false),
    };
    if args.len() != count {
        return Err(format!("{method} expects {count} string argument(s)"));
    }
    let ParamValue::Str(request_id) = &args[0] else {
        return Err("screen request id must be a string".into());
    };
    let edit = if method == "inspectScreens" {
        ScreenEdit::InspectManager
    } else {
        let ParamValue::Str(request) = &args[1] else {
            return Err("screen request must be a JSON string".into());
        };
        decode(request)?
    };
    enqueue(ctx, source, owner, request_id, edit)?;
    Ok(true)
}

pub(super) fn forward_result(ctx: &mut ProcessCtx, source: NodeId, mut event: CustomEvent) {
    if event.topic != APP_OPERATION_RESULT_TOPIC || event.origin != Some(source) {
        return;
    }
    event.topic = "mapgyver.screens.result".into();
    ctx.emit_custom_event(event);
}
