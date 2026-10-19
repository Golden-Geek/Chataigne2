use crate::app::module::common::streaming::{
    admission::{retain_payload, PreparedMessages},
    module_helpers::format_bytes_for_log,
    parser::{StreamingParseConfig, StreamingParser},
    script::prepare_stream_bytes_callbacks,
};
use crate::app::module::script_api::NativeCallbackPayload;
use golden_core::node::NodeId;
use golden_io::RetainedPayload;
use std::{
    mem::size_of,
    sync::{Arc, Mutex},
};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StreamInputConfig {
    pub module: NodeId,
    pub parse: StreamingParseConfig,
    pub processing_enabled: bool,
    pub log_incoming: bool,
}

#[derive(Clone)]
pub(crate) struct InputSnapshot {
    pub generation: u64,
    pub config: StreamInputConfig,
}

#[derive(Clone)]
pub(crate) struct InputPlan(Arc<Mutex<InputSnapshot>>);

impl InputPlan {
    pub(crate) fn new(config: StreamInputConfig) -> Self {
        Self(Arc::new(Mutex::new(InputSnapshot { generation: 0, config })))
    }
    pub(crate) fn snapshot(&self) -> InputSnapshot {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub(crate) fn update(&self, config: StreamInputConfig) {
        let mut current = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if current.config.parse != config.parse
            || current.config.processing_enabled != config.processing_enabled
            || current.config.module != config.module
        {
            current.generation = current.generation.wrapping_add(1);
        }
        current.config = config;
    }
}

#[derive(Debug)]
pub(crate) struct PreparedStreamInput {
    pub values: Result<PreparedMessages, String>,
    pub module: NodeId,
    pub callbacks: Vec<NativeCallbackPayload>,
    pub log_message: Option<String>,
    pub rejected_lines: u64,
    pub weight: usize,
}

pub(crate) fn prepare(
    parser: &mut StreamingParser,
    snapshot: &InputSnapshot,
    bytes: &[u8],
    transport: &str,
) -> Result<RetainedPayload<PreparedStreamInput>, String> {
    let config = &snapshot.config;
    let values = if config.processing_enabled {
        parser.push_bytes(bytes, &config.parse)
    } else {
        Ok(PreparedMessages::default())
    };
    let callbacks = prepare_stream_bytes_callbacks(bytes, None, true)?;
    let rejected_lines = parser.take_rejected_line_count();
    let log_message = config.log_incoming.then(|| {
        let prefix = &bytes[..bytes.len().min(1024)];
        let suffix = if bytes.len() > prefix.len() {
            format!(" ({} bytes total)", bytes.len())
        } else {
            String::new()
        };
        format!(
            "Received {transport} {}{}{}",
            format_bytes_for_log(prefix),
            suffix,
            if config.processing_enabled {
                ""
            } else {
                " (processing disabled)"
            }
        )
    });
    let own_weight = size_of::<PreparedStreamInput>()
        + callbacks.capacity() * size_of::<NativeCallbackPayload>()
        + log_message.as_ref().map_or(0, String::capacity)
        + values.as_ref().err().map_or(0, String::capacity);
    let weight = own_weight
        + values.as_ref().map_or(0, PreparedMessages::retained_bytes)
        + callbacks
            .iter()
            .map(NativeCallbackPayload::retained_bytes)
            .sum::<usize>();
    retain_payload(
        PreparedStreamInput {
            values,
            module: config.module,
            callbacks,
            log_message,
            rejected_lines,
            weight,
        },
        own_weight,
    )
}
