//! Built-in "Generic" output commands that are not bound to a module.
//!
//! Generic commands reuse the module-command framework for their trigger
//! plumbing (so the Outputs manager fires them exactly like a module command),
//! but they perform their action directly instead of emitting a module command
//! request.

use golden_core::{
    events::{Event, EventKind},
    node,
    node::{Node, NodeId},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module_command;

/// User-item kind for built-in, module-independent output commands.
pub(crate) const GENERIC_COMMAND_ITEM_KIND: &str = "generic_command";

pub(crate) const GENERIC_LOG_COMMAND_NODE_TYPE: &str = "generic_log_command";

/// Generic command that writes a message to the log when triggered.
#[node("generic_log_command", label = "Log")]
#[children(
    message: String = String::new() (
        label = "Message",
        description = "Text written to the log when this command is triggered."
    );
)]
pub struct GenericLogCommand {
    base: crate::app::ModuleCommandBase,
}

impl GenericLogCommand {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleCommandBase::new())
    }
}

#[golden_core::item("generic_command", node = "generic_log_command", via = base, from_struct)]
impl Node for GenericLogCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == GENERIC_LOG_COMMAND_NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        match event.kind {
            EventKind::ParamChanged { .. } => u32::MAX,
            _ => 0,
        }
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        if !module_command::module_command_triggered(snapshot, self.id(), param) {
            return;
        }
        self.run(ctx);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        if module_command::is_command_execute_request(&event, self.id()) {
            self.run(ctx);
        }
    }
}

impl GenericLogCommand {
    fn run(&self, ctx: &mut ProcessCtx) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        let message = command_string_param(snapshot, self.id(), "message").unwrap_or_default();
        golden_core::log!(origin = self.id(); format!("{message}"));
    }
}

fn command_string_param(snapshot: &ProcessTreeSnapshot, command_id: NodeId, path: &str) -> Option<String> {
    module_command::resolve_module_command_child(snapshot, command_id, path).and_then(|param_id| {
        snapshot
            .node(param_id)
            .and_then(|node| node.param_value.as_ref())
            .and_then(ParamValue::as_str)
    })
}
