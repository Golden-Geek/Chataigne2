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
    node::{Node, NodeCreationContext, NodeId},
    parameter::ParamValue,
    process_ctx::{ProcessCtx, ProcessTreeSnapshot},
};

use crate::app::module_command::{self, ModuleCommandDeliveryPolicy, ModuleCommandInvocationId};

/// User-item kind for built-in, module-independent output commands.
pub(crate) const GENERIC_COMMAND_ITEM_KIND: &str = "generic_command";

pub(crate) const GENERIC_LOG_COMMAND_NODE_TYPE: &str = "generic_log_command";

pub(crate) const LOG_INVOCATION_CHANGE_MIN_TICKS: u64 = 30;
pub(crate) const LOG_INVOCATION_KEEPALIVE_TICKS: u64 = 200;
pub(crate) const LOG_INVOCATION_STALE_TICKS: u64 = 12_000;
pub(crate) const LOG_INVOCATION_RECENCY_TOUCH_TICKS: u64 = 256;
pub(crate) const MAX_LOG_INVOCATIONS: usize = 32_768;
const MAX_LOG_EMISSIONS_PER_TICK: usize = 1;
pub(crate) const MAX_LOG_PRUNE_STEPS_PER_EVENT: usize = 8;

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
        description = "Text written to the log when this command is triggered."
    );
)]
pub struct GenericLogCommand {
    #[state(default = String::new())]
    cached_message: String,
    #[state(default = None)]
    cached_message_param: Option<NodeId>,
    #[state(default = GenericLogRuntimeCache::default())]
    runtime_cache: GenericLogRuntimeCache,
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

    fn inbox_requires_tree_snapshot(&self, events: &EventFrame) -> bool {
        events.iter().any(|event| match &event.kind {
            EventKind::ParamChanged { .. } => true,
            EventKind::Custom(custom) => {
                self.cached_message_param.is_none() && module_command::is_command_execute_request(custom, self.id())
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
        let Some(execute) = module_command::command_execute_request(&event, self.id()) else {
            return;
        };
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
}

impl GenericLogCommand {
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
