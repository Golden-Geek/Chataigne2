use std::sync::Arc;

use golden_core::{
    events::{CustomEvent, Event, EventKind},
    node::{Node, NodeId, NodeScriptDescriptor, UserCreatableItem},
    parameter::ParamValue,
    process_ctx::ProcessCtx,
};

use super::{
    protocol::{MediaEdit, MediaKind},
    scripting,
};

pub const MEDIA_COMMAND_TYPE: &str = "mapgyver_media_command";

fn default_request() -> String {
    serde_json::to_string(&MediaEdit::InspectManager).expect("static media operation")
}

pub(super) fn catalog() -> Vec<UserCreatableItem> {
    let mut items = vec![UserCreatableItem::new(
        MEDIA_COMMAND_TYPE,
        MEDIA_COMMAND_TYPE,
        "Media Operation",
    )];
    for (label, media_kind) in [
        ("Create Solid Color", MediaKind::SolidColor),
        ("Create Picture", MediaKind::Picture),
        ("Create Shared Texture", MediaKind::SharedTexture),
        ("Create Composition", MediaKind::Composition),
    ] {
        let request = serde_json::to_string(&MediaEdit::CreateMedia {
            media_kind,
            label: None,
        })
        .expect("static command preset");
        items.push(
            UserCreatableItem::new(MEDIA_COMMAND_TYPE, MEDIA_COMMAND_TYPE, label)
                .with_initial_param("request", ParamValue::Str(request)),
        );
    }
    items
}

#[golden_core::node("mapgyver_media_command", label = "Media Operation")]
#[children(
    request_id: String = "media-command".to_owned() (
        label = "Request Id", description = "Correlation key returned with the committed result."
    );
    request: String = default_request() (
        label = "Request", description = "Media operation and arguments as JSON; presets supply creation requests.", widget = "textarea"
    );
    execute: ParamValue = ParamValue::Trigger() (label = "Execute");
)]
pub struct MediaCommandNode {
    owner: Arc<()>,
}

impl MediaCommandNode {
    pub fn create() -> Self {
        Self::new(Arc::new(()))
    }

    fn execute_request(&self, ctx: &mut ProcessCtx) -> Result<(), String> {
        if !self.node_data().effective_enabled {
            return Err("media command is disabled".into());
        }
        let edit = scripting::decode(self.request.get_ref())?;
        scripting::enqueue(ctx, self.id(), &self.owner, self.request_id.get_ref(), edit)
    }
}

#[golden_core::node("mapgyver_media_command", from_struct)]
impl Node for MediaCommandNode {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == MEDIA_COMMAND_TYPE).then(Self::create)
    }

    fn user_item_kind(&self) -> &str {
        MEDIA_COMMAND_TYPE
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
            golden_core::logerror!(origin = self.id(); format!("Media command rejected: {error}"));
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
