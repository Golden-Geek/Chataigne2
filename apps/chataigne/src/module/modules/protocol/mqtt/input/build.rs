use super::*;
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPreparer};
use golden_io::PayloadPool;
use std::sync::Arc;

pub(super) struct Draft {
    source: Option<Source>,
    raw: RetainedPayload<MqttReceivedPublish>,
    values: Vec<ReceivedValueMessage>,
    callback: NativeCallbackDraft,
    error: Option<String>,
    prepared: Option<Arc<Prepared>>,
    weight: usize,
}

impl Draft {
    pub(super) fn new(raw: RetainedPayload<MqttReceivedPublish>, source: Source, log: bool) -> Result<Self, String> {
        if raw.payload.len() > MAX_PAYLOAD || raw.topic.len() > 4096 {
            return Err("MQTT input exceeds 64 KiB payload / 4096 topic bytes; message was not admitted".into());
        }
        let (values, error) = match decode::values(&raw, source.mode.payload) {
            Ok(values) => (values, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let payload = super::super::payload::mqtt_payload_script_arg(&raw.payload);
        let callback = NativeCallbackDraft::new(
            super::super::MQTT_MESSAGE_RECEIVED_CALLBACK,
            vec![
                serde_json::json!(raw.topic),
                payload.clone(),
                serde_json::json!({ "topic": raw.topic, "payload": payload, "bytes": crate::app::module::script_api::bytes_arg(&raw.payload), "qos": raw.qos.variant(), "retain": raw.retain }),
            ],
        )?;
        let weight = size_of::<Prepared>()
            + 2 * size_of::<usize>()
            + source.lifetime.retained_heap_bytes()
            + 4 * (values.capacity() * size_of::<ReceivedValueMessage>()
                + values.iter().map(ReceivedValueMessage::retained_weight).sum::<usize>())
            + error.as_ref().map_or(0, String::capacity);
        if weight + callback.retained_bytes() > MAX_PREPARED_BYTES {
            return Err("MQTT prepared input exceeds 16 MiB; message was not admitted".into());
        }
        if log {
            golden_core::log!(origin = source.module; format!("Received MQTT {} on {}", super::super::payload::format_mqtt_payload_for_log(&raw.payload[..raw.payload.len().min(256)]), raw.topic));
        }
        Ok(Self {
            source: Some(source),
            raw,
            values,
            callback,
            error,
            prepared: None,
            weight,
        })
    }
    pub(super) fn retain(
        &mut self,
        values: &PayloadPool,
        callbacks: &NativeCallbackPreparer,
    ) -> Option<RetainedPayload<Prepared>> {
        if self.prepared.is_none() {
            let callback = callbacks.try_retain(&self.callback)?;
            self.prepared = Some(Arc::new(Prepared {
                source: self.source.take().unwrap(),
                values: std::mem::take(&mut self.values),
                callback,
                error: self.error.take(),
                _raw: self.raw.clone(),
            }));
        }
        values.try_retain_arc(self.prepared.as_ref()?.clone(), self.weight).ok()
    }
    #[cfg(test)]
    pub(super) fn prepared_identity(&self) -> Option<*const Prepared> {
        self.prepared.as_ref().map(Arc::as_ptr)
    }
}
