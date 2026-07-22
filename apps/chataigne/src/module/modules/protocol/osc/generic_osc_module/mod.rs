use golden_core::{
    engine::NodeExecutionRule,
    events::{CustomEvent, Event},
    node::{Node, NodeCreationContext, NodeId, NodeMetaPatch, NodeScriptDescriptor},
    parameter::{ParameterEventBehaviour, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::{
    module::common::received_values::{
        apply_received_value_batch, ReceivedValueBatchMessage, ReceivedValueBatchOptions,
        ReceivedValuePayload,
    },
    OscDecodedMessage, OscValuePayload,
};

#[golden_core::node("osc_module", label = "OSC")]
pub struct GenericOscModule {
    base: crate::app::OscModuleBase,
}

impl GenericOscModule {
    pub fn create() -> Self {
        Self::new(crate::app::OscModuleBase::create())
    }

    #[cfg(test)]
    pub(crate) fn disable_transport_for_test(&mut self) {
        self.base.stop_transport();
        self.base.set_transport_dirty(false);
    }

    #[cfg(test)]
    pub(crate) fn enqueue_incoming_message_for_test(&mut self, message: OscDecodedMessage) {
        self.base.enqueue_incoming_message(message);
    }

    #[cfg(test)]
    pub(crate) fn auto_add_enabled_for_test(&self) -> bool {
        self.base.auto_add_enabled()
    }

    #[cfg(test)]
    pub(crate) fn has_pending_incoming_messages_for_test(&self) -> bool {
        self.base.has_pending_incoming_messages()
    }

    fn process_pending_incoming(
        base: &mut crate::app::OscModuleBase,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
    ) {
        let messages = base.take_pending_incoming_messages();
        if messages.is_empty() {
            return;
        }

        let Some(values_id) = base.values_id() else {
            for message in &messages {
                base.emit_osc_message_received_callback(ctx, message);
            }
            return;
        };

        let received_values = messages
            .iter()
            .filter_map(received_value_for_message)
            .collect::<Vec<_>>();

        let result = apply_received_value_batch(
            ctx,
            snapshot,
            values_id,
            received_values.iter().map(|received| ReceivedValueBatchMessage {
                path_segments: received.path_segments.as_slice(),
                payload: &received.payload,
                source_description: received.source_description.as_str(),
            }),
            ReceivedValueBatchOptions {
                auto_add: base.auto_add_enabled(),
                event_behaviour: ParameterEventBehaviour::Coalesce,
            },
        );
        base.mark_internal_param_changes(result.updated_params);

        for message in &messages {
            base.emit_osc_message_received_callback(ctx, message);
        }
    }
}

struct OscReceivedValue {
    path_segments: Vec<String>,
    payload: ReceivedValuePayload,
    source_description: String,
}

fn received_value_for_message(message: &OscDecodedMessage) -> Option<OscReceivedValue> {
    let payload = match &message.payload {
        OscValuePayload::Single(value) => ReceivedValuePayload::Single(value.clone()),
        OscValuePayload::Multi(values) => ReceivedValuePayload::Multi(values.clone()),
        OscValuePayload::Arguments(_) => return None,
    };
    Some(OscReceivedValue {
        path_segments: address_segments(message.address.as_str()),
        payload,
        source_description: format!("OSC address '{}'", message.address),
    })
}

fn address_segments(address: &str) -> Vec<String> {
    address
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect()
}

#[golden_core::item(
    "module",
    node = "osc_module",
    via = base,
    from_struct,
    menu_path = ["Network"]
)]
impl Node for GenericOscModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, context: NodeCreationContext) {
        self.base.on_node_ready(ctx, context);
    }

    fn update(&mut self, ctx: &mut ProcessCtx) {
        self.base.update(ctx);

        if !self.base.has_pending_incoming_messages() {
            return;
        }

        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();

        Self::process_pending_incoming(&mut self.base, ctx, snapshot);
    }

    fn destroy(&mut self, ctx: &mut ProcessCtx) {
        self.base.destroy(ctx);
    }

    fn update_requires_tree_snapshot(&self) -> bool {
        self.base.update_requires_tree_snapshot()
    }

    fn execution_rule(&self) -> NodeExecutionRule {
        self.base.execution_rule()
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        self.base.child_event_interest_depth(event)
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        self.base.script_descriptor_for_node(self.node_data(), self.get_type())
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        self.base.on_param_change(ctx, param, old_value);
    }

    fn on_meta_changed(&mut self, ctx: &mut ProcessCtx, node: NodeId, patch: NodeMetaPatch) {
        self.base.on_meta_changed(ctx, node, patch);
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.base.on_effective_enabled_changed(ctx, enabled);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.base.on_custom_event(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

#[cfg(test)]
mod tests;
