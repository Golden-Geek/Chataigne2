use golden_core::{
    node::{NodeData, NodeId, NodeScriptDescriptor},
    process_ctx::ProcessCtx,
};
use serde_json::Value as JsonValue;

const STREAM_TEXT_RECEIVED_CALLBACK: &str = "textReceived";
const STREAM_DATA_RECEIVED_CALLBACK: &str = "dataReceived";
const STREAM_DATA_RECEIVE_CALLBACK: &str = "dataReceive";
const STREAM_CLIENT_CONNECTED_CALLBACK: &str = "clientConnected";
const STREAM_CLIENT_DISCONNECTED_CALLBACK: &str = "clientDisconnected";

pub(crate) const STREAMING_SCRIPT_METHODS: &[&str] = &[
    "sendText",
    "sendString",
    "sendBytes",
    "sendData",
    "sendHex",
    "sendHexString",
];

pub(crate) fn descriptor_for_node(node_data: &NodeData, node_type: &str) -> NodeScriptDescriptor {
    crate::app::module::script_api::descriptor_for_node(node_data, node_type, STREAMING_SCRIPT_METHODS)
}

pub(crate) fn emit_stream_bytes_callbacks(
    ctx: &mut ProcessCtx,
    module_id: NodeId,
    bytes: &[u8],
    source: Option<&str>,
    text_hint: bool,
) {
    let source_arg = source
        .map(|source| JsonValue::String(source.to_string()))
        .unwrap_or(JsonValue::Null);
    let data_args = vec![
        crate::app::module::script_api::bytes_arg(bytes),
        source_arg.clone(),
    ];
    crate::app::module::script_api::emit_script_callback(
        ctx,
        module_id,
        STREAM_DATA_RECEIVED_CALLBACK,
        data_args.clone(),
    );
    crate::app::module::script_api::emit_script_callback(ctx, module_id, STREAM_DATA_RECEIVE_CALLBACK, data_args);

    if text_hint {
        if let Ok(text) = std::str::from_utf8(bytes) {
            crate::app::module::script_api::emit_script_callback(
                ctx,
                module_id,
                STREAM_TEXT_RECEIVED_CALLBACK,
                vec![JsonValue::String(text.to_string()), source_arg],
            );
        }
    }
}

pub(crate) fn emit_client_connected(ctx: &mut ProcessCtx, module_id: NodeId, client_id: &str, info: String) {
    crate::app::module::script_api::emit_script_callback(
        ctx,
        module_id,
        STREAM_CLIENT_CONNECTED_CALLBACK,
        vec![serde_json::json!(client_id), serde_json::json!(info)],
    );
}

pub(crate) fn emit_client_disconnected(
    ctx: &mut ProcessCtx,
    module_id: NodeId,
    client_id: &str,
    reason: Option<&str>,
) {
    crate::app::module::script_api::emit_script_callback(
        ctx,
        module_id,
        STREAM_CLIENT_DISCONNECTED_CALLBACK,
        vec![serde_json::json!(client_id), serde_json::json!(reason)],
    );
}
