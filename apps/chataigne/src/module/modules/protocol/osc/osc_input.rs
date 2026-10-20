//! Worker-prepared incoming values and callbacks, with canonical background disposal.
use super::{osc_message::OscDecodedMessage, osc_runtime::admission::payload_weight, OscValuePayload};
use crate::app::module::{
    common::received_values::{ReceivedValueMessage, ReceivedValuePayload},
    script_api::{NativeCallbackDraft, NativeCallbackPayload, NativeCallbackPreparer},
};
use golden_core::node::NodeId;
use golden_io::{PayloadPool, RetainedPayload};
use std::{
    mem::size_of,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

#[cfg(test)]
mod tests;

const MAX_EVENT_BYTES: usize = 2 * 1024 * 1024;
pub(super) const INPUT_BATCH_MESSAGES: usize = 16;

#[derive(Clone)]
pub(crate) struct InputPlan {
    module: NodeId,
    log: Arc<AtomicBool>,
}
impl InputPlan {
    pub(super) fn new(module: NodeId, log: bool) -> Self {
        Self {
            module,
            log: Arc::new(AtomicBool::new(log)),
        }
    }
    pub(super) fn set_log(&self, enabled: bool) {
        self.log.store(enabled, Ordering::Release);
    }
}

#[derive(Clone)]
pub(super) enum InputMode {
    Prepared(InputPlan),
    #[cfg(test)]
    Decoded,
}

#[derive(Debug)]
pub(crate) struct PreparedInput {
    pub module: NodeId,
    pub values: Vec<ReceivedValueMessage>,
    pub callbacks: Vec<NativeCallbackPayload>,
    pub log: Option<String>,
    pub weight: usize,
}

pub(super) struct InputPreparer {
    plan: InputPlan,
    values: PayloadPool,
    callbacks: NativeCallbackPreparer,
}

pub(super) struct InputDraft {
    module: NodeId,
    values: Vec<ReceivedValueMessage>,
    callbacks: Vec<NativeCallbackDraft>,
    retained_callbacks: Vec<NativeCallbackPayload>,
    log: Option<String>,
    own_weight: usize,
    prepared: Option<Arc<PreparedInput>>,
}

impl InputPreparer {
    pub(super) fn new(plan: InputPlan) -> Result<Self, String> {
        Ok(Self {
            plan,
            values: input_pool()?,
            callbacks: NativeCallbackPreparer::new()?,
        })
    }
    #[cfg(test)]
    pub(super) fn draft(&self, message: OscDecodedMessage) -> Result<InputDraft, String> {
        self.draft_batch(vec![message])
    }
    pub(super) fn draft_batch(&self, messages: Vec<OscDecodedMessage>) -> Result<InputDraft, String> {
        InputDraft::new(&self.plan, messages)
    }
    pub(super) fn retain(&self, draft: &mut InputDraft) -> Option<RetainedPayload<PreparedInput>> {
        if draft.prepared.is_none() {
            while let Some(callback) = draft.callbacks.get(draft.retained_callbacks.len()) {
                draft.retained_callbacks.push(self.callbacks.try_retain(callback)?);
            }
            draft.prepared = Some(Arc::new(PreparedInput {
                module: draft.module,
                values: std::mem::take(&mut draft.values),
                callbacks: std::mem::take(&mut draft.retained_callbacks),
                log: draft.log.take(),
                weight: draft.own_weight
                    + draft
                        .callbacks
                        .iter()
                        .map(NativeCallbackDraft::retained_bytes)
                        .sum::<usize>(),
            }));
        }
        self.values
            .try_retain_arc(draft.prepared.as_ref()?.clone(), draft.own_weight)
            .ok()
    }
}

impl InputDraft {
    fn new(plan: &InputPlan, messages: Vec<OscDecodedMessage>) -> Result<Self, String> {
        if messages.is_empty() || messages.len() > INPUT_BATCH_MESSAGES {
            return Err("OSC input batch must contain 1-16 messages".into());
        }
        let log = plan.log.load(Ordering::Acquire).then(|| {
            format!(
                "Received OSC {} ({} messages in prepared batch)",
                messages[0].address.chars().take(256).collect::<String>(),
                messages.len(),
            )
        });
        let mut values = Vec::with_capacity(messages.len());
        let mut callbacks = Vec::with_capacity(messages.len());
        for message in messages {
            let (value, callback) = prepare_message(message)?;
            values.extend(value);
            callbacks.push(callback);
        }
        let retained_callbacks = Vec::with_capacity(callbacks.len());
        let own_weight = size_of::<PreparedInput>()
            + 2 * size_of::<usize>()
            + values.capacity() * size_of::<ReceivedValueMessage>()
            + values.iter().map(ReceivedValueMessage::retained_weight).sum::<usize>()
            + retained_callbacks.capacity() * size_of::<NativeCallbackPayload>()
            + log.as_ref().map_or(0, String::capacity);
        if own_weight.saturating_add(callbacks.iter().map(NativeCallbackDraft::retained_bytes).sum::<usize>())
            > MAX_EVENT_BYTES - 256
        {
            return Err("OSC incoming preparation exceeds 2 MiB; batch was not admitted".into());
        }
        Ok(Self {
            module: plan.module,
            values,
            callbacks,
            retained_callbacks,
            log,
            own_weight,
            prepared: None,
        })
    }
}

fn prepare_message(message: OscDecodedMessage) -> Result<(Option<ReceivedValueMessage>, NativeCallbackDraft), String> {
    payload_weight(&message.address, &message.payload)?;
    if !message.address.starts_with('/') || message.address.contains('\0') {
        return Err("OSC incoming address must start with '/' and contain no NUL".into());
    }
    let mut path = Vec::new();
    for segment in message
        .address
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if path.len() == 64 {
            return Err("OSC incoming path exceeds 64 segments".into());
        }
        path.push(segment.to_string());
    }
    let payload_json = match &message.payload {
        OscValuePayload::Single(value) => value.to_script_json(),
        OscValuePayload::Multi(values) | OscValuePayload::Arguments(values) => {
            crate::app::module::script_api::param_values_arg(values)
        }
    };
    let callback = NativeCallbackDraft::new(
        "messageReceived",
        vec![
            serde_json::json!(message.address),
            payload_json.clone(),
            serde_json::json!({ "address": message.address, "payload": payload_json }),
        ],
    )?;
    let payload = match message.payload {
        OscValuePayload::Single(value) => Some(ReceivedValuePayload::Single(value)),
        OscValuePayload::Multi(values) => Some(ReceivedValuePayload::Multi(values)),
        // This variant belongs to outgoing authoring, never to the OSC decoder.
        OscValuePayload::Arguments(_) => None,
    };
    let value = payload.map(|payload| ReceivedValueMessage {
        path_segments: path,
        payload,
        source_description: format!("OSC address '{}'", message.address),
    });
    Ok((value, callback))
}

fn input_pool() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut pool = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if pool.is_none() {
        *pool = Some(
            PayloadPool::new("osc-input-disposer", 1024, 32 * 1024 * 1024)
                .map_err(|error| format!("failed to start OSC input disposer: {error}"))?,
        );
    }
    Ok(pool.as_ref().expect("installed input disposer").clone())
}
