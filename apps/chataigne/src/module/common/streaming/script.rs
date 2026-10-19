use golden_core::node::{NodeData, NodeScriptDescriptor};
use serde_json::Value as JsonValue;

const STREAM_TEXT_RECEIVED_CALLBACK: &str = "textReceived";
const STREAM_DATA_RECEIVED_CALLBACK: &str = "dataReceived";
const STREAM_DATA_RECEIVE_CALLBACK: &str = "dataReceive";

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

pub(crate) fn prepare_stream_bytes_callbacks(
    bytes: &[u8],
    source: Option<&str>,
    text_hint: bool,
) -> Result<Vec<crate::app::module::script_api::NativeCallbackPayload>, String> {
    stream_bytes_callback_args(bytes, source, text_hint)
        .into_iter()
        .map(|(callback, args)| crate::app::module::script_api::NativeCallbackPayload::prepare(callback, args))
        .collect()
}

fn stream_bytes_callback_args(
    bytes: &[u8],
    source: Option<&str>,
    text_hint: bool,
) -> Vec<(&'static str, Vec<JsonValue>)> {
    let source_arg = source
        .map(|source| JsonValue::String(source.to_string()))
        .unwrap_or(JsonValue::Null);
    let args = vec![crate::app::module::script_api::bytes_arg(bytes), source_arg.clone()];
    let mut callbacks = vec![
        (STREAM_DATA_RECEIVED_CALLBACK, args.clone()),
        (STREAM_DATA_RECEIVE_CALLBACK, args),
    ];
    if text_hint {
        if let Ok(text) = std::str::from_utf8(bytes) {
            callbacks.push((
                STREAM_TEXT_RECEIVED_CALLBACK,
                vec![JsonValue::String(text.to_string()), source_arg],
            ));
        }
    }
    callbacks
}
