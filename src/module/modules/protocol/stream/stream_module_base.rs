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

#[node("streaming_module_base", label = "Streaming Module")]
#[children(
    folder(parameters, label = "Parameters", reuse = true) {
        folder(receiver, label = "Receiver", reuse = true, can_be_disabled = true) {
            auto_add: bool = true (
                label = "Auto Add",
                description = "Automatically create missing value nodes from incoming stream data."
            );
            parse_mode: Enum = "line" (
                label = "Parse Mode",
                description = "How incoming bytes are converted into values.",
                enum_options = ["line (default)", "raw"]
            );
            line_delimiter: String = "\\n".to_string() (
                label = "Line Delimiter",
                description = "Delimiter that terminates one incoming line. Escape sequences such as \\n, \\r, \\t, and \\xNN are supported."
            );
            value_separator: String = ",".to_string() (
                label = "Value Separator",
                description = "Separator used to split one line into values. Leave empty to split on whitespace."
            );
            first_element: Enum = "name" (
                label = "First Element",
                description = "Whether the first line element is a parameter name or a value.",
                enum_options = ["name (default)", "value"]
            );
            hierarchy_from_name: bool = true (
                label = "Name Hierarchy",
                description = "Split parameter names such as my.received.value into nested value folders."
            );
            hierarchy_delimiter: String = ".".to_string() (
                label = "Hierarchy Delimiter",
                description = "Delimiter used to split incoming parameter names into nested folders."
            );
        }
    }
)]
pub struct StreamingModuleBase {
    base: crate::app::ModuleBase,
    parser: StreamingParser,
    incoming: StreamingIncomingQueue,
}

impl StreamingModuleBase {
    pub fn create() -> Self {
        Self::new(
            crate::app::ModuleBase::new(),
            StreamingParser::default(),
            StreamingIncomingQueue::new(),
        )
    }

    pub(crate) fn parameters_id(&self) -> Option<NodeId> {
        self.base.parameters_id()
    }

    pub(crate) fn parameter_child_node_id(
        &self,
        snapshot: &ProcessTreeSnapshot,
        child_name: &str,
    ) -> Option<NodeId> {
        let parameters_id = self.parameters_id()?;
        snapshot.find_child(parameters_id, child_name)
    }

    pub(crate) fn parameter_child_enabled(
        &self,
        snapshot: &ProcessTreeSnapshot,
        child_name: &str,
    ) -> Option<bool> {
        let child_id = self.parameter_child_node_id(snapshot, child_name)?;
        snapshot.node(child_id).map(|node| node.enabled)
    }

    pub(crate) fn receiver_node_id(&self, snapshot: &ProcessTreeSnapshot) -> Option<NodeId> {
        self.parameter_child_node_id(snapshot, "receiver")
    }

    pub(crate) fn receiver_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        self.parameter_child_enabled(snapshot, "receiver")
    }

    pub(crate) fn sender_enabled(&self, snapshot: &ProcessTreeSnapshot) -> Option<bool> {
        self.parameter_child_enabled(snapshot, "sender")
    }

    pub(crate) fn has_pending_messages(&self) -> bool {
        self.incoming.has_pending_messages()
    }

    pub(crate) fn take_ignored_param_change(&mut self, param: NodeId) -> bool {
        self.incoming.take_ignored_param_change(param)
    }

    pub(crate) fn parse_bytes(&mut self, bytes: &[u8]) -> Result<Vec<StreamingIncomingMessage>, String> {
        let parse_config = self.current_parse_config();
        self.parser.push_bytes(bytes, &parse_config)
    }

    pub(crate) fn push_messages(&mut self, messages: Vec<StreamingIncomingMessage>) {
        self.incoming.push_messages(messages);
    }

    pub(crate) fn process_pending(&mut self, ctx: &mut ProcessCtx, snapshot: &ProcessTreeSnapshot) -> bool {
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

    pub(crate) fn set_data_capabilities(
        &mut self,
        ctx: &mut ProcessCtx,
        capabilities: ModuleDataCapabilities,
    ) {
        self.base.set_data_capabilities(ctx, capabilities);
    }

    pub(crate) fn emit_incoming_traffic(&self, ctx: &mut ProcessCtx) {
        self.base.emit_incoming_traffic(ctx);
    }

    pub(crate) fn emit_outgoing_traffic(&self, ctx: &mut ProcessCtx) {
        self.base.emit_outgoing_traffic(ctx);
    }

    fn current_parse_config(&self) -> StreamingParseConfig {
        streaming_parse_config(
            self.parse_mode.get_ref().as_str(),
            self.line_delimiter.get_ref().as_str(),
            self.value_separator.get_ref().as_str(),
            self.first_element.get_ref().as_str(),
            self.hierarchy_from_name.get(),
            self.hierarchy_delimiter.get_ref().as_str(),
        )
    }
}

#[node("streaming_module_base", via = base, from_struct)]
impl Node for StreamingModuleBase {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }
}

#[cfg(test)]
mod tests {
    use golden_core::node::{Folder, Node, NodeId};

    const COMMON_RECEIVER_PARAM_DECL_IDS: &[&str] = &[
        "auto_add",
        "parse_mode",
        "line_delimiter",
        "value_separator",
        "first_element",
        "hierarchy_from_name",
        "hierarchy_delimiter",
    ];

    #[test]
    fn concrete_stream_modules_materialize_common_receiver_params_once() {
        assert_common_receiver_params_once(crate::app::SerialModule::create().into(), "Serial");
        assert_common_receiver_params_once(crate::app::TcpModule::create().into(), "TCP");
        assert_common_receiver_params_once(crate::app::UdpModule::create().into(), "UDP");
    }

    fn assert_common_receiver_params_once(module: crate::app::AppNode, label: &str) {
        let (engine, module_id) = create_engine_with_module(module);
        let parameters_id = find_child_by_key(&engine, module_id, "parameters")
            .unwrap_or_else(|| panic!("{label} module should have a parameters folder"));
        let receiver_id = find_child_by_key(&engine, parameters_id, "receiver")
            .unwrap_or_else(|| panic!("{label} module should have a receiver folder"));

        for decl_id in COMMON_RECEIVER_PARAM_DECL_IDS {
            let count = count_direct_children_by_key(&engine, receiver_id, decl_id);
            assert_eq!(
                count, 1,
                "{label} receiver should materialize exactly one '{decl_id}' parameter"
            );
        }
    }

    fn create_engine_with_module(module: crate::app::AppNode) -> (crate::app::AppEngine, NodeId) {
        let root: crate::app::AppNode = Folder::new("root").into();
        let mut engine = crate::app::AppEngine::new(root);
        engine.add_node(module, None);
        engine.apply_edits().expect("stream module should attach");
        for _ in 0..4 {
            engine.apply_edits().expect("stream module defaults should materialize");
        }

        let module_id = engine
            .nodes
            .get(engine.root)
            .and_then(|root| root.node_data().first_child)
            .expect("stream module should be attached under root");

        (engine, module_id)
    }

    fn find_child_by_key(
        engine: &crate::app::AppEngine,
        parent: NodeId,
        key: &str,
    ) -> Option<NodeId> {
        let mut current = engine.nodes.get(parent)?.node_data().first_child;
        while let Some(child_id) = current {
            let child = engine.nodes.get(child_id)?;
            if node_key_matches(child.node_data(), key) {
                return Some(child_id);
            }
            current = child.node_data().next_sibling;
        }
        None
    }

    fn count_direct_children_by_key(
        engine: &crate::app::AppEngine,
        parent: NodeId,
        key: &str,
    ) -> usize {
        let mut count = 0;
        let mut current = engine
            .nodes
            .get(parent)
            .and_then(|node| node.node_data().first_child);
        while let Some(child_id) = current {
            let child = engine
                .nodes
                .get(child_id)
                .expect("child id from node links should exist");
            if node_key_matches(child.node_data(), key) {
                count += 1;
            }
            current = child.node_data().next_sibling;
        }
        count
    }

    fn node_key_matches(node: &golden_core::node::NodeData, key: &str) -> bool {
        node.meta.decl_id.0 == key
            || node.meta.decl_id.0.rsplit('/').next() == Some(key)
            || node.meta.short_name == key
            || node.meta.label == key
    }
}
