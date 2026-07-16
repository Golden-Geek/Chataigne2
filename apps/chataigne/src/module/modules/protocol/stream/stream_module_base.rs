use golden_core::{
    node,
    node::{Node, NodeId},
    parameter::Enum,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module::{
    common::streaming::{
        module_helpers::{streaming_parse_config, StreamingIncomingQueue},
        parser::{StreamingIncomingMessage, StreamingParseConfig, StreamingParser},
    },
    ModuleDataCapabilities,
};

const STREAM_BACKPRESSURE_WARNING_ID: &str = "stream_backpressure";

#[node("streaming_module_base", label = "Streaming Module")]
#[children(
    folder(parameters) {
        folder(processing, label = "Processing", can_be_disabled = true) {
            auto_add: bool = true (
                label = "Auto Add",
                description = "Automatically create missing value nodes from incoming stream data."
            );
            parse_mode: Enum = "line" (
                label = "Parse Mode",
                description = "How incoming bytes are converted into values.",
                enum_options = ["line (default)", "raw", "json"]
            );
            name_separator: Enum = "space" (
                label = "Name Separator",
                description = "Separator between an incoming value name and its payload. Disable to receive unnamed values under 'received'.",
                enum_options = streaming_separator_enum_options(),
                can_be_disabled = true,
            );
            value_separator: Enum = "colon" (
                label = "Value Separator",
                description = "Separator used to split one payload into multiple values. Disable to keep the payload as one value.",
                enum_options = streaming_separator_enum_options(),
                can_be_disabled = true,
            );
            hierarchy_separator: Enum = "dot" (
                label = "Hierarchy Separator",
                description = "Separator used to split incoming value names into nested folders. Slash path segments are always supported.",
                enum_options = streaming_separator_enum_options(),
                can_be_disabled = true,
            );
        }
    }
)]
pub struct StreamingModuleBase {
    base: crate::app::ModuleBase,
    parser: StreamingParser,
    incoming: StreamingIncomingQueue,
    cached_parse_config: StreamingParseConfig,
    cached_processing_enabled: bool,
}

impl StreamingModuleBase {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            StreamingParser::default(),
            StreamingIncomingQueue::new(),
            StreamingParseConfig::default(),
            true,
        )
    }

    pub(crate) fn parameters_id(&self) -> Option<NodeId> {
        self.base.parameters_id()
    }

    pub(crate) fn connection_id(&self) -> Option<NodeId> {
        self.base.connection_id()
    }

    pub(crate) fn parameter_child_node_id(&self, snapshot: &ProcessTreeSnapshot, child_decl_id: &str) -> Option<NodeId> {
        let parameters_id = self.parameters_id()?;
        snapshot.find_child_by_decl_id(parameters_id, child_decl_id)
    }

    pub(crate) fn parameter_child_enabled(&self, snapshot: &ProcessTreeSnapshot, child_decl_id: &str) -> Option<bool> {
        let child_id = self.parameter_child_node_id(snapshot, child_decl_id)?;
        snapshot.node(child_id).map(|node| node.enabled)
    }

    pub(crate) fn connection_child_node_id(&self, snapshot: &ProcessTreeSnapshot, child_decl_id: &str) -> Option<NodeId> {
        let connection_id = self.connection_id()?;
        snapshot.find_child_by_decl_id(connection_id, child_decl_id)
    }

    pub(crate) fn connection_child_enabled(&self, snapshot: &ProcessTreeSnapshot, child_decl_id: &str) -> Option<bool> {
        let child_id = self.connection_child_node_id(snapshot, child_decl_id)?;
        snapshot.node(child_id).map(|node| node.enabled)
    }

    pub(crate) fn processing_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        self.parameter_child_enabled(snapshot, "processing")
    }

    /// Returns the cached processing-enabled state, updated by `refresh_config_cache`.
    pub(crate) fn processing_enabled_cached(&self) -> bool {
        self.cached_processing_enabled
    }

    /// Refreshes `cached_parse_config` and `cached_processing_enabled` from the snapshot.
    /// Call this whenever the snapshot is available (e.g. when `transport_dirty` or
    /// `has_pending_messages`) so the hot drain path can run without a snapshot.
    pub(crate) fn refresh_config_cache(&mut self, snapshot: &ProcessTreeSnapshot) {
        self.cached_parse_config = self.current_parse_config(snapshot);
        self.cached_processing_enabled = self.processing_enabled(snapshot).unwrap_or(true);
    }

    pub(crate) fn has_pending_messages(&self) -> bool {
        self.incoming.has_pending_messages()
    }

    pub(crate) fn take_ignored_param_change(&mut self, param: NodeId) -> bool {
        self.incoming.take_ignored_param_change(param)
    }

    pub(crate) fn parse_bytes_cached(&mut self, bytes: &[u8]) -> Result<Vec<StreamingIncomingMessage>, String> {
        self.parser.push_bytes(bytes, &self.cached_parse_config)
    }

    pub(crate) fn push_messages(&mut self, messages: Vec<StreamingIncomingMessage>) {
        self.incoming.push_messages(messages);
    }

    pub(crate) fn process_pending(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) -> bool {
        let dropped = self.incoming.take_dropped_message_count();
        if dropped > 0 {
            ctx.set_node_warning_with(
                self.id(),
                Some(STREAM_BACKPRESSURE_WARNING_ID),
                format!("Dropped {dropped} incoming stream messages because the bounded queue was full."),
                None,
            );
        } else {
            ctx.clear_node_warning(self.id(), Some(STREAM_BACKPRESSURE_WARNING_ID));
        }
        self.incoming
            .process_pending(ctx, snapshot, self.base.values_id(), self.auto_add.get())
    }

    pub(crate) fn log_incoming_enabled(&self) -> bool {
        self.base.log_incoming_enabled()
    }

    pub(crate) fn log_outgoing_enabled(&self) -> bool {
        self.base.log_outgoing_enabled()
    }

    pub(crate) fn set_connected(&mut self, ctx: &mut ProcessCtx, connected: bool) {
        self.base.set_connected(ctx, connected);
    }

    pub(crate) fn set_data_capabilities(&mut self, ctx: &mut ProcessCtx, capabilities: ModuleDataCapabilities) {
        self.base.set_data_capabilities(ctx, capabilities);
    }

    pub(crate) fn emit_incoming_traffic(&self, ctx: &mut ProcessCtx) {
        self.base.emit_incoming_traffic(ctx);
    }

    pub(crate) fn emit_outgoing_traffic(&self, ctx: &mut ProcessCtx) {
        self.base.emit_outgoing_traffic(ctx);
    }

    pub(crate) fn emit_script_param_callback(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        param: NodeId,
        old_value: &golden_core::parameter::ParamValue,
    ) {
        self.base
            .emit_script_param_callback(ctx, snapshot, param, old_value);
    }

    fn current_parse_config(&self, snapshot: &ProcessTreeSnapshot) -> StreamingParseConfig {
        streaming_parse_config(
            self.parse_mode.get_ref().as_str(),
            self.enabled_separator_variant(
                snapshot,
                self.name_separator.id(),
                self.name_separator.get_ref().as_str(),
            ),
            self.enabled_separator_variant(
                snapshot,
                self.value_separator.id(),
                self.value_separator.get_ref().as_str(),
            ),
            self.enabled_separator_variant(
                snapshot,
                self.hierarchy_separator.id(),
                self.hierarchy_separator.get_ref().as_str(),
            ),
        )
    }

    fn enabled_separator_variant<'a>(
        &self,
        snapshot: &ProcessTreeSnapshot,
        node: NodeId,
        variant: &'a str,
    ) -> Option<&'a str> {
        match snapshot.node(node) {
            Some(node) if !node.enabled => None,
            _ => Some(variant),
        }
    }
}

#[node("streaming_module_base", via = base, from_struct)]
impl Node for StreamingModuleBase {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base
            .configure_command_tester(ctx, crate::app::module::common::streaming::commands::STREAMING_COMMAND_NODE_TYPES);
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
}

fn streaming_separator_enum_options() -> Vec<golden_core::parameter::ParameterEnumOption> {
    [
        ("comma", "Comma (,)"),
        ("space", "Space"),
        ("colon", "Colon (:)"),
        ("semicolon", "Semicolon (;)"),
        ("dot", "Dot (.)"),
        ("tab", "Tab (\\t)"),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(index, (variant_id, label))| golden_core::parameter::ParameterEnumOption {
            variant_id: variant_id.to_string(),
            value: golden_core::parameter::ParamValue::Enum(variant_id.to_string()),
            label: label.to_string(),
            tags: Vec::new(),
            ordering: Some(index as i32),
        },
    )
    .collect()
}

#[cfg(test)]
mod stream_module_base_tests;
