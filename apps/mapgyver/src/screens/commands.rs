use super::{protocol::ScreenEdit, scripting};
use golden_core::{
    events::{CustomEvent, Event, EventKind},
    node::{Node, NodeId, NodeScriptDescriptor, UserCreatableItem},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};
use std::sync::Arc;

pub const SCREEN_COMMAND_TYPE: &str = "mapgyver_screen_command";

fn default_request() -> String {
    serde_json::to_string(&ScreenEdit::InspectManager).expect("static screen operation")
}

pub(super) fn catalog() -> Vec<UserCreatableItem> {
    vec![
        UserCreatableItem::new(SCREEN_COMMAND_TYPE, SCREEN_COMMAND_TYPE, "Screen Operation"),
        UserCreatableItem::new(SCREEN_COMMAND_TYPE, SCREEN_COMMAND_TYPE, "Create Screen").with_initial_param(
            "request",
            ParamValue::Str(serde_json::to_string(&ScreenEdit::CreateScreen { label: None }).expect("static preset")),
        ),
    ]
}

#[golden_core::node("mapgyver_screen_command", label = "Screen Operation")]
#[children(
    request_id: String = "screen-command".to_owned() (label = "Request Id", description = "Correlation key returned with the committed result.");
    request: String = default_request() (label = "Request", description = "ScreenEdit operation and arguments as JSON.", widget = "textarea");
    execute: ParamValue = ParamValue::Trigger() (label = "Execute");
)]
pub struct ScreenCommandNode {
    owner: Arc<()>,
}

impl ScreenCommandNode {
    pub fn create() -> Self {
        Self::new(Arc::new(()))
    }
    fn execute_request(&self, ctx: &mut ProcessCtx) -> Result<(), String> {
        if !self.node_data().effective_enabled {
            return Err("screen command is disabled".into());
        }
        scripting::enqueue(
            ctx,
            self.id(),
            &self.owner,
            self.request_id.get_ref(),
            scripting::decode(self.request.get_ref())?,
        )
    }
}

#[golden_core::node("mapgyver_screen_command", from_struct)]
impl Node for ScreenCommandNode {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == SCREEN_COMMAND_TYPE).then(Self::create)
    }
    fn user_item_kind(&self) -> &str {
        SCREEN_COMMAND_TYPE
    }
    fn needs_update(&self) -> bool {
        false
    }
    fn execution_rule(&self) -> golden_core::engine::NodeExecutionRule {
        golden_core::engine::NodeExecutionRule::passive()
    }
    fn inbox_requires_tree_snapshot(&self, _: &golden_core::events::EventFrame) -> bool {
        false
    }
    fn destroy(&mut self, _: &mut ProcessCtx) {
        self.owner = Arc::new(());
    }
    fn on_effective_enabled_changed(&mut self, _: &mut ProcessCtx, enabled: bool) {
        if !enabled {
            self.owner = Arc::new(());
        }
    }
    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        u32::from(matches!(event.kind, EventKind::ParamChanged { .. }))
    }
    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _: ParamValue) {
        if self.execute.is_bound()
            && self.execute.id() == param
            && let Err(error) = self.execute_request(ctx)
        {
            golden_core::logerror!(origin = self.id(); format!("Screen command rejected: {error}"));
        }
    }
    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        let mut descriptor = NodeScriptDescriptor::default();
        descriptor.add_method("execute");
        descriptor
    }
    fn engine_call_custom_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if method != "execute" {
            return Ok(false);
        }
        if !args.is_empty() {
            return Err("execute expects no arguments".into());
        }
        self.execute_request(ctx)?;
        Ok(true)
    }
    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        scripting::forward_result(ctx, self.id(), event);
    }
}
