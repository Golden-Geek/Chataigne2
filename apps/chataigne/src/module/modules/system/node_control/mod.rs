use golden_core::{
    events::{CustomEvent, Event},
    node,
    node::{Node, NodeHandle, NodeId, NodeScriptDescriptor},
    parameter::{ParameterEventBehaviour, ParamValue},
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

pub(crate) use crate::app::module_modules_system_node_control_commands::{
    NodeControlRequest, NODE_COMMAND_TYPES,
};

const NODE_CONTROL_WARNING_ID: &str = "node_control_operation";
const NODE_SCRIPT_METHODS: &[&str] = &["setValue", "trigger"];
const NODE_VALUE_SET_CALLBACK: &str = "nodeValueSet";
const NODE_TRIGGERED_CALLBACK: &str = "nodeTriggered";

#[node("node_module", label = "Node")]
#[children(
    folder(values) {
        last_target: String = String::new() (
            label = "Last Target",
            description = "Path of the last parameter changed through this module.",
            read_only = true
        );
        operation_count: i32 = 0 (
            label = "Operation Count",
            description = "Number of successful set or trigger operations.",
            read_only = true
        );
    }
)]
pub struct NodeModule {
    base: crate::app::ModuleBase,
}

impl NodeModule {
    pub fn create() -> Self {
        Self::new(crate::app::ModuleBase::create_with_command_types(
            NODE_COMMAND_TYPES,
        ))
    }

    fn apply_control(
        &mut self,
        ctx: &mut ProcessCtx,
        request: NodeControlRequest,
    ) -> Result<(), String> {
        let snapshot = ctx
            .tree_snapshot_arc()
            .ok_or_else(|| "Node operation requires a tree snapshot".to_string())?;
        let target = match &request {
            NodeControlRequest::SetValue { target, .. }
            | NodeControlRequest::Trigger { target } => *target,
        };
        let target_node = snapshot
            .node(target)
            .ok_or_else(|| format!("Node target {:?} no longer exists", target))?;
        if target_node.param_value.is_none() {
            return Err(format!(
                "Node target '{}' is not a parameter",
                target_node.label
            ));
        }
        let target_path = node_path(snapshot.as_ref(), target);

        match request {
            NodeControlRequest::SetValue { value, .. } => {
                ctx.set_param(target, value.clone());
                crate::app::module::script_api::emit_script_callback(
                    ctx,
                    self.id(),
                    NODE_VALUE_SET_CALLBACK,
                    vec![
                        crate::app::module::script_api::node_arg(target),
                        value.to_script_json(),
                    ],
                );
            }
            NodeControlRequest::Trigger { .. } => {
                if !matches!(target_node.param_value, Some(ParamValue::Trigger())) {
                    return Err(format!(
                        "Node target '{}' is not a trigger parameter",
                        target_node.label
                    ));
                }
                ctx.set_param_with_behaviour(
                    target,
                    ParamValue::Trigger(),
                    ParameterEventBehaviour::Append,
                );
                crate::app::module::script_api::emit_script_callback(
                    ctx,
                    self.id(),
                    NODE_TRIGGERED_CALLBACK,
                    vec![crate::app::module::script_api::node_arg(target)],
                );
            }
        }

        self.last_target.set(ctx, target_path);
        self.operation_count
            .set(ctx, self.operation_count.get().saturating_add(1));
        self.clear_operation_warning(ctx);
        self.base.emit_outgoing_traffic(ctx);
        Ok(())
    }

    fn on_custom_event_inner(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        let Some(request) = crate::app::module_command::decode_module_command_request(&event) else {
            return;
        };
        if request.module_id != self.id()
            || !NODE_COMMAND_TYPES.contains(&request.command_type.as_str())
        {
            return;
        }
        if let Err(error) = serde_json::from_value::<NodeControlRequest>(request.payload)
            .map_err(|error| format!("invalid Node command payload: {error}"))
            .and_then(|request| self.apply_control(ctx, request))
        {
            self.set_operation_warning(ctx, error.as_str());
            golden_core::logerror!(format!("Failed to execute Node command: {error}"));
        }
    }

    fn handle_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Option<Result<(), String>> {
        let request = match method {
            "setValue" => Some(script_set_value(ctx.tree_snapshot(), args)),
            "trigger" => Some(script_trigger(ctx.tree_snapshot(), args)),
            _ => None,
        }?;
        Some(request.and_then(|request| self.apply_control(ctx, request)))
    }

    fn set_operation_warning(&self, ctx: &mut ProcessCtx, message: &str) {
        NodeHandle::new(self.id()).set_warning_with(
            ctx,
            Some(NODE_CONTROL_WARNING_ID),
            message,
            None,
        );
    }

    fn clear_operation_warning(&self, ctx: &mut ProcessCtx) {
        NodeHandle::new(self.id()).clear_warning(ctx, Some(NODE_CONTROL_WARNING_ID));
    }
}

#[golden_core::item(
    "module",
    node = "node_module",
    via = base,
    from_struct,
    menu_path = ["System"]
)]
impl Node for NodeModule {
    fn init(&mut self, ctx: &mut ProcessCtx) {
        self.base.init(ctx);
        self.base.configure_command_tester(ctx, NODE_COMMAND_TYPES);
        self.base.set_connected(ctx, true);
        self.base.set_data_capabilities(
            ctx,
            crate::app::module::ModuleDataCapabilities::new(false, true),
        );
        crate::app::module::enable_module_authoring(self.node_data_mut());
    }

    fn child_event_interest_depth(&self, _event: &Event) -> u32 {
        u32::MAX
    }

    fn engine_script_descriptor(&self) -> NodeScriptDescriptor {
        crate::app::module::script_api::descriptor_for_node(
            self.node_data(),
            self.get_type(),
            NODE_SCRIPT_METHODS,
        )
    }

    fn engine_call_script_method(
        &mut self,
        ctx: &mut ProcessCtx,
        method: &str,
        args: &[ParamValue],
    ) -> Result<bool, String> {
        if let Some(result) = self.handle_script_method(ctx, method, args) {
            result?;
            return Ok(true);
        }
        self.base.engine_call_script_method(ctx, method, args)
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, old_value: ParamValue) {
        if let Some(snapshot) = ctx.tree_snapshot_arc() {
            self.base
                .emit_script_param_callback(ctx, snapshot.as_ref(), param, &old_value);
        }
    }

    fn on_effective_enabled_changed(&mut self, ctx: &mut ProcessCtx, enabled: bool) {
        self.base.set_connected(ctx, enabled);
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: CustomEvent) {
        self.on_custom_event_inner(ctx, event);
    }

    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == Self::NODE_TYPE).then(Self::create)
    }
}

pub(crate) fn script_set_value(
    snapshot: Option<&ProcessTreeSnapshot>,
    args: &[ParamValue],
) -> Result<NodeControlRequest, String> {
    if args.len() != 2 {
        return Err("setValue expects a node reference and a value".to_string());
    }
    let snapshot = snapshot.ok_or_else(|| "setValue requires a tree snapshot".to_string())?;
    let target = resolve_script_reference(snapshot, &args[0])
        .ok_or_else(|| "setValue expects a valid node reference".to_string())?;
    Ok(NodeControlRequest::SetValue {
        target,
        value: args[1].clone(),
    })
}

pub(crate) fn script_trigger(
    snapshot: Option<&ProcessTreeSnapshot>,
    args: &[ParamValue],
) -> Result<NodeControlRequest, String> {
    if args.len() != 1 {
        return Err("trigger expects one node reference".to_string());
    }
    let snapshot = snapshot.ok_or_else(|| "trigger requires a tree snapshot".to_string())?;
    resolve_script_reference(snapshot, &args[0])
        .map(|target| NodeControlRequest::Trigger { target })
        .ok_or_else(|| "trigger expects a valid node reference".to_string())
}

fn resolve_script_reference(
    snapshot: &ProcessTreeSnapshot,
    value: &ParamValue,
) -> Option<NodeId> {
    let ParamValue::Reference(reference) = value else {
        return None;
    };
    reference
        .cached_id()
        .filter(|target| {
            snapshot
                .node(*target)
                .is_some_and(|node| node.uuid == reference.uuid())
        })
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))
}

fn node_path(snapshot: &ProcessTreeSnapshot, target: NodeId) -> String {
    let mut parts = Vec::new();
    let mut current = Some(target);
    while let Some(node_id) = current {
        let Some(node) = snapshot.node(node_id) else {
            break;
        };
        parts.push(node.label.clone());
        current = node.parent;
    }
    parts.reverse();
    parts.join("/")
}

#[cfg(test)]
mod node_control_tests;
