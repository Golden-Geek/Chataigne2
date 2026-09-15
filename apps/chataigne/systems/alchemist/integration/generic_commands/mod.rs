//! Built-in "Generic" output commands that are not bound to a module.
//!
//! Generic commands reuse the module-command framework for their trigger
//! plumbing (so the Outputs manager fires them exactly like a module command),
//! but they perform their action directly instead of emitting a module command
//! request.

use std::collections::{HashMap, VecDeque};

use golden_core::{
    events::{Event, EventFrame, EventKind},
    node,
    node::{Node, NodeCreationContext, NodeHandle, NodeId},
    parameter::{
        Parameter, ParameterChangeCheck, ParameterEventBehaviour, ParamValue,
        ReferenceTargetKind,
    },
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module_command::{self, ModuleCommandDeliveryPolicy, ModuleCommandInvocationId};

/// User-item kind for built-in, module-independent output commands.
pub(crate) const GENERIC_COMMAND_ITEM_KIND: &str = "generic_command";

pub(crate) const GENERIC_LOG_COMMAND_NODE_TYPE: &str = "generic_log_command";
pub(crate) const GENERIC_INVOKE_COMMAND_NODE_TYPE: &str = "generic_invoke_command";
pub(crate) const GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE: &str = "generic_set_parameter_command";
pub(crate) const GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE: &str = "generic_trigger_parameter_command";

const GENERIC_COMMAND_OPERATION_WARNING_ID: &str = "generic_command_operation";

pub(crate) const LOG_INVOCATION_CHANGE_MIN_TICKS: u64 = 30;
pub(crate) const LOG_INVOCATION_KEEPALIVE_TICKS: u64 = 200;
pub(crate) const LOG_INVOCATION_STALE_TICKS: u64 = 12_000;
pub(crate) const LOG_INVOCATION_RECENCY_TOUCH_TICKS: u64 = 256;
pub(crate) const MAX_LOG_INVOCATIONS: usize = 32_768;
const MAX_LOG_EMISSIONS_PER_TICK: usize = 1;
pub(crate) const MAX_LOG_PRUNE_STEPS_PER_EVENT: usize = 8;

#[node("generic_command_base", label = "Command")]
#[children(
    trigger: ParamValue = ParamValue::Trigger() (
        label = "Trigger",
        description = "Fire this trigger to run the command.",
        show_in_inspector_content = false
    );
)]
pub struct GenericCommandBase {}

#[node("generic_command_base", from_struct)]
impl Node for GenericCommandBase {
    fn init(&mut self, _ctx: &mut ProcessCtx) {
        module_command::enable_module_command_authoring(self.node_data_mut());
    }

    fn user_item_kind(&self) -> &str {
        GENERIC_COMMAND_ITEM_KIND
    }
}

pub(crate) fn generic_command_supports_batch(node_type: &str) -> bool {
    matches!(
        node_type,
        GENERIC_LOG_COMMAND_NODE_TYPE
            | GENERIC_INVOKE_COMMAND_NODE_TYPE
            | GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE
            | GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE
    )
}

#[node("generic_set_parameter_command", label = "Set Parameter")]
#[children(
    target: golden_core::node::NodeReference = golden_core::node::NodeReference::default() (
        label = "Target Parameter",
        description = "Parameter whose value will be updated.",
        reference_target_kind = ReferenceTargetKind::ParameterOnly
    );
    value: ParamValue = ParamValue::Bool(false) (
        label = "Value",
        description = "Value converted and written through the target parameter's constraints.",
        tags = vec![module_command::COMMAND_PRIMARY_VALUE_TAG.to_owned()]
    );
)]
pub struct GenericSetParameterCommand {
    #[state(default = None)]
    operation_warning: Option<String>,
    base: GenericCommandBase,
}

impl GenericSetParameterCommand {
    pub fn create() -> Self {
        Self::new(GenericCommandBase::new())
    }

    fn execute(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        overrides: &[module_command::ModuleCommandParamOverride],
    ) -> Result<(), String> {
        let target = command_parameter_value(snapshot, self.target.id(), overrides)
            .ok_or_else(|| "Set Parameter requires a target parameter".to_string())?;
        let value = command_parameter_value(snapshot, self.value.id(), overrides)
            .ok_or_else(|| "Set Parameter requires a value".to_string())?;
        set_parameter_value(ctx, snapshot, &target, value)
    }

    fn run_current(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            self.update_operation_warning(ctx, Some("Set Parameter requires a tree snapshot".to_string()));
            return;
        };
        let result = self.execute(ctx, snapshot.as_ref(), &[]);
        self.update_operation_warning(ctx, result.err());
    }

    fn run_event(&mut self, ctx: &mut ProcessCtx, event: &golden_core::events::CustomEvent) {
        let Some(executions) = command_executions(event, self.id()) else {
            return;
        };
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            self.update_operation_warning(ctx, Some("Set Parameter requires a tree snapshot".to_string()));
            return;
        };
        let mut first_error = None;
        for execution in &executions {
            if let Err(error) = self.execute(ctx, snapshot.as_ref(), &execution.param_overrides) {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        self.update_operation_warning(ctx, first_error);
    }

    fn update_operation_warning(&mut self, ctx: &mut ProcessCtx, error: Option<String>) {
        update_command_warning(ctx, self.id(), &mut self.operation_warning, error);
    }

    fn sync_value_type_to_target(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            return;
        };
        let Some(target) = resolve_parameter_target(
            snapshot.as_ref(),
            &ParamValue::Reference(self.target.get_ref().clone()),
        ) else {
            return;
        };
        let Some(value) = snapshot.node(target).and_then(|node| node.param_value.clone()) else {
            return;
        };
        let Some(current) = snapshot.node(self.value.id()) else {
            return;
        };
        let mut replacement = Parameter::new(
            current.label.as_str(),
            value,
            ParameterChangeCheck::ValueChange,
        );
        replacement.node_data_mut().meta.uuid = current.uuid;
        replacement.node_data_mut().meta.tags = current.tags.clone();
        replacement.node_data_mut().meta.presentation = current.presentation.clone();
        ctx.replace_node(self.value.id(), replacement);
    }
}

#[golden_core::item(
    "generic_command",
    node = "generic_set_parameter_command",
    via = base,
    from_struct
)]
impl Node for GenericSetParameterCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == GENERIC_SET_PARAMETER_COMMAND_NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        command_inbox_requires_tree_snapshot(events, self.id())
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if param == self.target.id() {
            self.sync_value_type_to_target(ctx);
            return;
        }
        if ctx
            .tree_snapshot()
            .is_some_and(|snapshot| module_command::module_command_triggered(snapshot, self.id(), param))
        {
            self.run_current(ctx);
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        self.run_event(ctx, &event);
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        self.sync_value_type_to_target(ctx);
    }
}

#[node("generic_trigger_parameter_command", label = "Trigger Parameter")]
#[children(
    target: golden_core::node::NodeReference = golden_core::node::NodeReference::default() (
        label = "Target Parameter",
        description = "Trigger parameter that will receive a new trigger edge.",
        reference_target_kind = ReferenceTargetKind::ParameterOnly
    );
)]
pub struct GenericTriggerParameterCommand {
    #[state(default = None)]
    operation_warning: Option<String>,
    base: GenericCommandBase,
}

impl GenericTriggerParameterCommand {
    pub fn create() -> Self {
        Self::new(GenericCommandBase::new())
    }

    fn execute(
        &self,
        ctx: &mut ProcessCtx,
        snapshot: &ProcessTreeSnapshot,
        overrides: &[module_command::ModuleCommandParamOverride],
    ) -> Result<(), String> {
        let target = command_parameter_value(snapshot, self.target.id(), overrides)
            .ok_or_else(|| "Trigger Parameter requires a target parameter".to_string())?;
        trigger_parameter(ctx, snapshot, &target)
    }

    fn execute_with_referenced_param(
        &self,
        ctx: &mut ProcessCtx,
        overrides: &[module_command::ModuleCommandParamOverride],
    ) -> Result<(), String> {
        let reference = match overrides.iter().find(|override_value| override_value.param_id == self.target.id()) {
            Some(override_value) => match &override_value.value {
                ParamValue::Reference(reference) => reference,
                _ => return Err("Trigger Parameter requires a valid target parameter".to_string()),
            },
            None => self.target.get_ref(),
        };
        let Some((target, value)) = ctx.referenced_param(reference) else {
            return Err("Trigger Parameter requires a valid target parameter".to_string());
        };
        if !matches!(value, ParamValue::Trigger()) {
            return Err("Trigger Parameter requires a trigger target".to_string());
        }
        ctx.set_param_with_behaviour(target, ParamValue::Trigger(), ParameterEventBehaviour::Append);
        Ok(())
    }

    fn run_current(&mut self, ctx: &mut ProcessCtx) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            self.update_operation_warning(ctx, Some("Trigger Parameter requires a tree snapshot".to_string()));
            return;
        };
        let result = self.execute(ctx, snapshot.as_ref(), &[]);
        self.update_operation_warning(ctx, result.err());
    }

    fn run_event(&mut self, ctx: &mut ProcessCtx, event: &golden_core::events::CustomEvent) {
        let Some(executions) = command_executions(event, self.id()) else {
            return;
        };
        let snapshot = ctx.tree_snapshot_arc();
        let mut first_error = None;
        for execution in &executions {
            let result = match snapshot.as_ref() {
                Some(snapshot) => self.execute(ctx, snapshot.as_ref(), &execution.param_overrides),
                None => self.execute_with_referenced_param(ctx, &execution.param_overrides),
            };
            if let Err(error) = result {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        self.update_operation_warning(ctx, first_error);
    }

    fn update_operation_warning(&mut self, ctx: &mut ProcessCtx, error: Option<String>) {
        update_command_warning(ctx, self.id(), &mut self.operation_warning, error);
    }
}

#[golden_core::item(
    "generic_command",
    node = "generic_trigger_parameter_command",
    via = base,
    from_struct
)]
impl Node for GenericTriggerParameterCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == GENERIC_TRIGGER_PARAMETER_COMMAND_NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        events.iter().any(|event| matches!(event.kind, EventKind::ParamChanged { .. }))
    }

    fn visit_inbox_parameter_references(&self, events: &EventFrame, visit: &mut dyn FnMut(&node::NodeReference)) {
        let mut needs_default = false;
        for event in events {
            let EventKind::Custom(custom) = &event.kind else {
                continue;
            };
            let Some(executions) = command_executions(custom, self.id()) else {
                continue;
            };
            for execution in &executions {
                match execution
                    .param_overrides
                    .iter()
                    .find(|override_value| override_value.param_id == self.target.id())
                {
                    Some(override_value) => {
                        if let ParamValue::Reference(reference) = &override_value.value {
                            visit(reference);
                        }
                    }
                    None => needs_default = true,
                }
            }
        }
        if needs_default {
            visit(self.target.get_ref());
        }
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if ctx
            .tree_snapshot()
            .is_some_and(|snapshot| module_command::module_command_triggered(snapshot, self.id(), param))
        {
            self.run_current(ctx);
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        self.run_event(ctx, &event);
    }
}

fn command_inbox_requires_tree_snapshot(events: &EventFrame, command: NodeId) -> bool {
    events.iter().any(|event| match &event.kind {
        EventKind::ParamChanged { .. } => true,
        EventKind::Custom(custom) => {
            module_command::is_command_execute_request(custom, command)
                || module_command::is_command_execute_batch_request(custom, command)
        }
        _ => false,
    })
}

fn command_executions(
    event: &golden_core::events::CustomEvent,
    command: NodeId,
) -> Option<Vec<module_command::ModuleCommandExecuteEvent>> {
    module_command::command_execute_request(event, command)
        .map(|execution| vec![execution])
        .or_else(|| module_command::command_execute_batch_requests(event, command))
}

fn command_parameter_value(
    snapshot: &ProcessTreeSnapshot,
    parameter: NodeId,
    overrides: &[module_command::ModuleCommandParamOverride],
) -> Option<ParamValue> {
    overrides
        .iter()
        .find(|override_value| override_value.param_id == parameter)
        .map(|override_value| override_value.value.clone())
        .or_else(|| snapshot.node(parameter).and_then(|node| node.param_value.clone()))
}

fn resolve_parameter_target(snapshot: &ProcessTreeSnapshot, value: &ParamValue) -> Option<NodeId> {
    let ParamValue::Reference(reference) = value else {
        return None;
    };
    reference
        .cached_id()
        .filter(|target| {
            snapshot
                .node(*target)
                .is_some_and(|node| node.uuid == reference.uuid() && node.param_value.is_some())
        })
        .or_else(|| {
            snapshot
                .node_id_by_uuid(reference.uuid())
                .filter(|target| snapshot.node(*target).is_some_and(|node| node.param_value.is_some()))
        })
}

fn set_parameter_value(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    target: &ParamValue,
    value: ParamValue,
) -> Result<(), String> {
    let target = resolve_parameter_target(snapshot, target)
        .ok_or_else(|| "Set Parameter requires a valid target parameter".to_string())?;
    if snapshot
        .node(target)
        .is_some_and(|node| matches!(node.param_value, Some(ParamValue::Trigger())))
    {
        return Err("Set Parameter cannot target a trigger; use Trigger Parameter".to_string());
    }
    ctx.set_param(target, value);
    Ok(())
}

fn trigger_parameter(
    ctx: &mut ProcessCtx,
    snapshot: &ProcessTreeSnapshot,
    target: &ParamValue,
) -> Result<(), String> {
    let target = resolve_parameter_target(snapshot, target)
        .ok_or_else(|| "Trigger Parameter requires a valid target parameter".to_string())?;
    if snapshot
        .node(target)
        .is_none_or(|node| !matches!(node.param_value, Some(ParamValue::Trigger())))
    {
        return Err("Trigger Parameter requires a trigger target".to_string());
    }
    ctx.set_param_with_behaviour(
        target,
        ParamValue::Trigger(),
        ParameterEventBehaviour::Append,
    );
    Ok(())
}

#[node("generic_invoke_command", label = "Invoke Existing Command")]
#[children(
    target: golden_core::node::NodeReference = golden_core::node::NodeReference::default() (
        label = "Command",
        description = "Existing command or output group invoked through this explicit advanced adapter.",
        reference_target_kind = ReferenceTargetKind::AnyNode
    );
    unresolved_legacy_target: String = String::new() (
        label = "Unresolved Legacy Target",
        description = "Original target retained by migration when it could not be represented as a node reference. Select Command above to resolve it.",
        read_only = true
    );
)]
pub struct GenericInvokeCommand {
    #[state(default = None)]
    operation_warning: Option<String>,
    base: GenericCommandBase,
}

impl GenericInvokeCommand {
    pub fn create() -> Self {
        Self::new(GenericCommandBase::new())
    }

    fn forward(
        &mut self,
        ctx: &mut ProcessCtx,
        executions: Vec<module_command::ModuleCommandExecuteEvent>,
    ) {
        let Some(snapshot) = ctx.tree_snapshot_arc() else {
            self.update_operation_warning(
                ctx,
                Some("Invoke Existing Command requires a tree snapshot".to_owned()),
            );
            return;
        };
        let target = match generic_invoke_target(snapshot.as_ref(), self.id()) {
            Ok(target) => target,
            Err(error) => {
                self.update_operation_warning(ctx, Some(error));
                return;
            }
        };
        let mut forwarded = executions
            .into_iter()
            .map(|mut execution| {
                execution.command_id = target;
                execution
            })
            .collect::<Vec<_>>();
        let result = if forwarded.len() == 1 {
            let execution = forwarded.pop().expect("one forwarded execution");
            module_command::emit_command_execute_with_invocation(
                ctx,
                target,
                execution.param_overrides,
                execution.invocation_id,
                execution.delivery_policy,
            )
            .map(|_| ())
        } else if forwarded.is_empty() {
            Ok(())
        } else {
            module_command::emit_command_execute_batch(ctx, target, forwarded).map(|_| ())
        };
        self.update_operation_warning(ctx, result.err());
    }

    fn update_operation_warning(&mut self, ctx: &mut ProcessCtx, error: Option<String>) {
        update_command_warning(ctx, self.id(), &mut self.operation_warning, error);
    }
}

#[golden_core::item(
    "generic_command",
    node = "generic_invoke_command",
    via = base,
    from_struct
)]
impl Node for GenericInvokeCommand {
    fn project_create(node_type: &str) -> Option<Self> {
        (node_type == GENERIC_INVOKE_COMMAND_NODE_TYPE).then(Self::create)
    }

    fn child_event_interest_depth(&self, event: &Event) -> u32 {
        matches!(event.kind, EventKind::ParamChanged { .. })
            .then_some(u32::MAX)
            .unwrap_or(0)
    }

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        command_inbox_requires_tree_snapshot(events, self.id())
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        if ctx
            .tree_snapshot()
            .is_some_and(|snapshot| module_command::module_command_triggered(snapshot, self.id(), param))
        {
            self.forward(
                ctx,
                vec![module_command::ModuleCommandExecuteEvent {
                    command_id: self.id(),
                    param_overrides: Vec::new(),
                    invocation_id: None,
                    delivery_policy: ModuleCommandDeliveryPolicy::Standard,
                }],
            );
        }
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        if let Some(executions) = command_executions(&event, self.id()) {
            self.forward(ctx, executions);
        }
    }
}

pub(crate) fn generic_invoke_target(
    snapshot: &ProcessTreeSnapshot,
    command: NodeId,
) -> Result<NodeId, String> {
    let target_param = module_command::resolve_module_command_child(snapshot, command, "target")
        .ok_or_else(|| "Invoke Existing Command has no target control".to_owned())?;
    let reference = snapshot
        .node(target_param)
        .and_then(|target| target.param_value.as_ref())
        .and_then(|value| match value {
            ParamValue::Reference(reference) if !reference.is_empty() => Some(reference),
            _ => None,
        })
        .ok_or_else(|| "Invoke Existing Command requires a target".to_owned())?;
    let target = reference
        .cached_id()
        .filter(|target| snapshot.node(*target).is_some())
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))
        .ok_or_else(|| "Invoke Existing Command target is unavailable".to_owned())?;
    if invoke_chain_reaches(snapshot, target, command, 0) {
        return Err("Invoke Existing Command cannot target itself or form a command cycle".to_owned());
    }
    if !crate::app::systems_alchemist_managed_nodes::is_output_node(snapshot, target)
        && !crate::app::systems_alchemist_managed_nodes::is_output_container(snapshot, target)
    {
        return Err("Invoke Existing Command target is not a command or output group".to_owned());
    }
    Ok(target)
}

fn invoke_chain_reaches(
    snapshot: &ProcessTreeSnapshot,
    candidate: NodeId,
    origin: NodeId,
    depth: usize,
) -> bool {
    if candidate == origin || depth >= 64 {
        return true;
    }
    if snapshot
        .node(candidate)
        .is_none_or(|node| node.node_type != GENERIC_INVOKE_COMMAND_NODE_TYPE)
    {
        return false;
    }
    let Some(target_param) =
        module_command::resolve_module_command_child(snapshot, candidate, "target")
    else {
        return false;
    };
    let Some(ParamValue::Reference(reference)) = snapshot
        .node(target_param)
        .and_then(|target| target.param_value.as_ref())
    else {
        return false;
    };
    reference
        .cached_id()
        .filter(|target| snapshot.node(*target).is_some())
        .or_else(|| snapshot.node_id_by_uuid(reference.uuid()))
        .is_some_and(|target| invoke_chain_reaches(snapshot, target, origin, depth + 1))
}

fn update_command_warning(
    ctx: &mut ProcessCtx,
    command: NodeId,
    current: &mut Option<String>,
    next: Option<String>,
) {
    if *current == next {
        return;
    }
    match next.as_deref() {
        Some(message) => {
            NodeHandle::new(command).set_warning_with(
                ctx,
                Some(GENERIC_COMMAND_OPERATION_WARNING_ID),
                message,
                None,
            );
            golden_core::logerror!(origin = command; message);
        }
        None => NodeHandle::new(command).clear_warning(ctx, Some(GENERIC_COMMAND_OPERATION_WARNING_ID)),
    }
    *current = next;
}

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
struct GenericLogRuntimeCache {
    records: HashMap<ModuleCommandInvocationId, GenericLogInvocationRecord>,
    recency: VecDeque<(ModuleCommandInvocationId, u64)>,
    next_generation: u64,
    budget_tick: Option<u64>,
    emissions_this_tick: usize,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct GenericLogInvocationRecord {
    message: String,
    emitted_tick: u64,
    last_seen_tick: u64,
    recency_tick: u64,
    generation: u64,
}

impl GenericLogRuntimeCache {
    fn should_emit(&mut self, invocation_id: ModuleCommandInvocationId, message: &str, tick: u64) -> bool {
        if self.budget_tick != Some(tick) {
            self.budget_tick = Some(tick);
            self.emissions_this_tick = 0;
        }
        self.prune_stale(tick);

        let mut touch_recency = false;
        if let Some(previous) = self.records.get_mut(&invocation_id) {
            previous.last_seen_tick = tick;
            touch_recency = tick.saturating_sub(previous.recency_tick) >= LOG_INVOCATION_RECENCY_TOUCH_TICKS;
            let minimum_ticks = if previous.message == message {
                LOG_INVOCATION_KEEPALIVE_TICKS
            } else {
                LOG_INVOCATION_CHANGE_MIN_TICKS
            };
            if tick.saturating_sub(previous.emitted_tick) < minimum_ticks {
                if touch_recency {
                    self.touch(invocation_id, tick);
                }
                return false;
            }
        }

        if self.emissions_this_tick >= MAX_LOG_EMISSIONS_PER_TICK {
            if touch_recency {
                self.touch(invocation_id, tick);
            }
            return false;
        }

        self.emissions_this_tick += 1;
        self.record_emission(invocation_id, message, tick);
        true
    }

    fn record_emission(&mut self, invocation_id: ModuleCommandInvocationId, message: &str, tick: u64) {
        if !self.records.contains_key(&invocation_id) {
            self.make_room();
        }
        let generation = self.allocate_generation();
        let record = self
            .records
            .entry(invocation_id)
            .or_insert_with(|| GenericLogInvocationRecord {
                message: String::new(),
                emitted_tick: tick,
                last_seen_tick: tick,
                recency_tick: tick,
                generation,
            });
        record.message.clear();
        record.message.push_str(message);
        record.emitted_tick = tick;
        record.last_seen_tick = tick;
        record.recency_tick = tick;
        record.generation = generation;
        self.recency.push_back((invocation_id, generation));
    }

    fn touch(&mut self, invocation_id: ModuleCommandInvocationId, tick: u64) {
        let generation = self.allocate_generation();
        let Some(record) = self.records.get_mut(&invocation_id) else {
            return;
        };
        record.recency_tick = tick;
        record.generation = generation;
        self.recency.push_back((invocation_id, generation));
    }

    fn allocate_generation(&mut self) -> u64 {
        let generation = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .expect("generic log invocation generation exhausted");
        generation
    }

    fn prune_stale(&mut self, tick: u64) {
        for _ in 0..MAX_LOG_PRUNE_STEPS_PER_EVENT {
            let Some((invocation_id, generation)) = self.recency.front().copied() else {
                break;
            };
            let Some(record) = self.records.get(&invocation_id) else {
                self.recency.pop_front();
                continue;
            };
            if record.generation != generation {
                self.recency.pop_front();
                continue;
            }
            if tick.saturating_sub(record.last_seen_tick) < LOG_INVOCATION_STALE_TICKS {
                break;
            }
            self.recency.pop_front();
            self.records.remove(&invocation_id);
        }
    }

    fn make_room(&mut self) {
        while self.records.len() >= MAX_LOG_INVOCATIONS {
            let Some((invocation_id, generation)) = self.recency.pop_front() else {
                self.records.clear();
                return;
            };
            if self
                .records
                .get(&invocation_id)
                .is_some_and(|record| record.generation == generation)
            {
                self.records.remove(&invocation_id);
            }
        }
    }
}

/// Generic command that writes a message to the log when triggered.
#[node("generic_log_command", label = "Log")]
#[children(
    message: String = String::new() (
        label = "Message",
        description = "Text written to the log when this command is triggered.",
        tags = vec![module_command::COMMAND_PRIMARY_VALUE_TAG.to_owned()]
    );
)]
pub struct GenericLogCommand {
    #[state(default = String::new())]
    cached_message: String,
    #[state(default = None)]
    cached_message_param: Option<NodeId>,
    #[state(default = GenericLogRuntimeCache::default())]
    runtime_cache: GenericLogRuntimeCache,
    base: GenericCommandBase,
}

impl GenericLogCommand {
    pub fn create() -> Self {
        Self::new(GenericCommandBase::new())
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

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        events.iter().any(|event| match &event.kind {
            EventKind::ParamChanged { .. } => true,
            EventKind::Custom(custom) => {
                self.cached_message_param.is_none()
                    && (module_command::is_command_execute_request(custom, self.id())
                        || module_command::is_command_execute_batch_request(custom, self.id()))
            }
            _ => false,
        })
    }

    fn on_node_ready(&mut self, ctx: &mut ProcessCtx, _context: NodeCreationContext) {
        if let Some(snapshot) = ctx.tree_snapshot() {
            self.refresh_cached_message(snapshot);
        }
    }

    fn on_param_change(&mut self, ctx: &mut ProcessCtx, param: NodeId, _old_value: ParamValue) {
        let Some(snapshot_arc) = ctx.tree_snapshot_arc() else {
            return;
        };
        let snapshot = snapshot_arc.as_ref();
        if module_command::resolve_module_command_child(snapshot, self.id(), "message") == Some(param) {
            self.refresh_cached_message(snapshot);
        }
        if !module_command::module_command_triggered(snapshot, self.id(), param) {
            return;
        }
        self.run();
    }

    fn on_custom_event(&mut self, ctx: &mut ProcessCtx, event: golden_core::events::CustomEvent) {
        if let Some(execute) = module_command::command_execute_request(&event, self.id()) {
            self.run_command_execute(ctx, &execute);
            return;
        }
        let Some(executions) = module_command::command_execute_batch_requests(&event, self.id()) else {
            return;
        };
        for execute in &executions {
            self.run_command_execute(ctx, execute);
        }
    }
}

impl GenericLogCommand {
    fn run_command_execute(&mut self, ctx: &mut ProcessCtx, execute: &module_command::ModuleCommandExecuteEvent) {
        if let Some(message) = self
            .cached_message_param
            .and_then(|param_id| command_string_param_override(&execute.param_overrides, param_id))
        {
            self.run_execute(
                ctx.time.tick,
                execute.invocation_id,
                execute.delivery_policy,
                message.as_str(),
            );
            return;
        }
        if self.cached_message.is_empty() {
            if let Some(snapshot) = ctx.tree_snapshot() {
                self.refresh_cached_message(snapshot);
            }
        }
        let message = self.cached_message.clone();
        self.run_execute(
            ctx.time.tick,
            execute.invocation_id,
            execute.delivery_policy,
            message.as_str(),
        );
    }

    fn run(&self) {
        self.run_message(self.cached_message.as_str());
    }

    fn run_message(&self, message: &str) {
        golden_core::log!(origin = self.id(); format!("{message}"));
    }

    fn run_execute(
        &mut self,
        tick: u64,
        invocation_id: Option<ModuleCommandInvocationId>,
        delivery_policy: ModuleCommandDeliveryPolicy,
        message: &str,
    ) {
        if delivery_policy == ModuleCommandDeliveryPolicy::ChangeAwareLogAdmitted {
            self.run_message(message);
            return;
        }
        if invocation_id.is_some_and(|invocation_id| !self.runtime_cache.should_emit(invocation_id, message, tick)) {
            return;
        }
        self.run_message(message);
    }

    fn refresh_cached_message(&mut self, snapshot: &ProcessTreeSnapshot) {
        self.cached_message_param = module_command::resolve_module_command_child(snapshot, self.id(), "message");
        self.cached_message = self
            .cached_message_param
            .and_then(|param_id| {
                snapshot
                    .node(param_id)
                    .and_then(|node| node.param_value.as_ref())
                    .and_then(ParamValue::as_str)
            })
            .unwrap_or_default();
    }
}

fn command_string_param_override(
    param_overrides: &module_command::ModuleCommandParamOverrides,
    param_id: NodeId,
) -> Option<String> {
    param_overrides
        .iter()
        .find(|entry| entry.param_id == param_id)
        .and_then(|entry| entry.value.as_str())
}

#[cfg(test)]
mod tests;
